use std::{
    collections::VecDeque,
    convert::Infallible,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    backend::{Backend, ClearType, TestBackend, WindowSize},
    buffer::{Buffer, Cell},
    layout::{Position, Size},
};
use serde_json::{Value, json};
use shodan::application::{TerminalMode, build_application, run_application};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
    task::JoinHandle,
    time::timeout,
};
use tokio_stream::{
    StreamExt,
    wrappers::{ReceiverStream, UnboundedReceiverStream},
};

pub const ASTEROIDS_PLAN_PROMPT: &str = "Write a proof-of-concept plan for a monochrome, 2D, low-poly Asteroids game in Bevy. Under ## Requirements, describe the game’s behavior without implementation details. Under ## User scenarios, write scenarios derived from those requirements. Name each scenario using ### 1. User ...";

pub struct CapturedRequest {
    pub body: Value,
    response_chunks: mpsc::Sender<String>,
}

#[derive(Default)]
pub struct RecordedLlama {
    available_models: Option<String>,
    model_loads: Vec<(&'static str, String, String)>,
    model_streams: Vec<(&'static str, VecDeque<String>)>,
}

impl RecordedLlama {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mock_available_models(mut self, available_models: &str) -> Self {
        self.available_models = Some(available_models.to_owned());
        self
    }

    pub fn mock_model_load(
        mut self,
        model: &'static str,
        loading_models: &str,
        loaded_models: &str,
    ) -> Self {
        self.model_loads
            .push((model, loading_models.to_owned(), loaded_models.to_owned()));
        self
    }

    pub fn mock_stream(mut self, model: &'static str, stream_path: &str) -> Self {
        match self
            .model_streams
            .iter_mut()
            .find(|(streamed_model, _)| *streamed_model == model)
        {
            Some((_, streams)) => streams.push_back(stream_path.to_owned()),
            None => self
                .model_streams
                .push((model, VecDeque::from([stream_path.to_owned()]))),
        }
        self
    }

    pub async fn start(self) -> (String, mpsc::Receiver<CapturedRequest>) {
        let available_models = self
            .available_models
            .expect("the mock server replays available models");
        let (requests, receiver) = mpsc::channel(8);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the mock server binds to a local port");
        let address = listener
            .local_addr()
            .expect("the mock server has a local address");
        let router = Router::new()
            .route("/v1/models", get(list_models))
            .route("/models/load", post(load_model))
            .route("/v1/chat/completions", post(stream_response))
            .with_state(LlamaState {
                requests,
                available_models: Arc::new(Mutex::new(AvailableModels::Serving(available_models))),
                model_loads: self.model_loads,
                model_streams: Arc::new(Mutex::new(self.model_streams)),
            });
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("the mock server serves requests");
        });
        (format!("http://{address}"), receiver)
    }
}

enum AvailableModels {
    Serving(String),
    Loading {
        loading: String,
        loaded: String,
        loading_served: bool,
    },
}

type RecordedStreams = Vec<(&'static str, VecDeque<String>)>;

fn read_fixture(fixture_path: &str) -> String {
    std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|error| panic!("the mock server cannot read {fixture_path}: {error}"))
}

fn current_models(models: &mut AvailableModels) -> String {
    match models {
        AvailableModels::Serving(fixture_path) => read_fixture(fixture_path),
        AvailableModels::Loading {
            loading,
            loaded,
            loading_served,
        } => {
            if *loading_served {
                read_fixture(loaded)
            } else {
                *loading_served = true;
                read_fixture(loading)
            }
        }
    }
}

fn assert_loaded_model(models_fixture: &str, requested_model: &str) {
    let models: Value = serde_json::from_str(models_fixture).expect("the models fixture is JSON");
    let loaded = models["data"]
        .as_array()
        .expect("the models fixture lists models")
        .iter()
        .any(|model| model["id"] == requested_model && model["status"]["value"] == "loaded");
    assert!(
        loaded,
        "the application chats with an unloaded model: {requested_model}"
    );
}

#[derive(Clone)]
struct LlamaState {
    requests: mpsc::Sender<CapturedRequest>,
    available_models: Arc<Mutex<AvailableModels>>,
    model_loads: Vec<(&'static str, String, String)>,
    model_streams: Arc<Mutex<RecordedStreams>>,
}

async fn list_models(State(server): State<LlamaState>) -> Response {
    let models_fixture = current_models(
        &mut server
            .available_models
            .lock()
            .expect("the mock server state is not poisoned"),
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .body(Body::from(models_fixture))
        .expect("the models fixture response is valid")
}

async fn load_model(
    State(server): State<LlamaState>,
    Json(body): Json<Value>,
) -> ([(header::HeaderName, &'static str); 1], &'static str) {
    let requested_model = body
        .get("model")
        .and_then(Value::as_str)
        .expect("the load request names a model");
    assert_eq!(body, json!({"model": requested_model}));
    let (_, loading_models, loaded_models) = server
        .model_loads
        .iter()
        .find(|(model, _, _)| *model == requested_model)
        .unwrap_or_else(|| panic!("the application loads an unmocked model: {requested_model}"));
    *server
        .available_models
        .lock()
        .expect("the mock server state is not poisoned") = AvailableModels::Loading {
        loading: loading_models.clone(),
        loaded: loaded_models.clone(),
        loading_served: false,
    };
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("fixtures/llama-cpp/load-model/success.json"),
    )
}

async fn stream_response(State(server): State<LlamaState>, Json(body): Json<Value>) -> Response {
    let requested_model = body.get("model").and_then(Value::as_str).map(str::to_owned);
    if requested_model.is_none() || requested_model.as_deref() == Some("") {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(include_str!(
                "fixtures/llama-cpp/chat/missing-model-error.json"
            )))
            .expect("the recorded error response is valid");
    }
    let requested_model = requested_model.expect("the chat request names a model");
    let models_fixture = current_models(
        &mut server
            .available_models
            .lock()
            .expect("the mock server state is not poisoned"),
    );
    assert_loaded_model(&models_fixture, &requested_model);
    let stream_fixture = server
        .model_streams
        .lock()
        .expect("the mock server state is not poisoned")
        .iter_mut()
        .find(|(streamed_model, _)| *streamed_model == requested_model)
        .and_then(|(_, streams)| streams.pop_front());
    let (response_chunks, receiver) = mpsc::channel::<String>(8);
    let request_body = body.clone();
    let captured_request = CapturedRequest {
        response_chunks: response_chunks.clone(),
        body,
    };
    server
        .requests
        .send(captured_request)
        .await
        .expect("the feature test receives model requests");
    if let Some(stream_path) = stream_fixture {
        tokio::spawn(async move {
            stream_recorded_events(&response_chunks, &stream_path, &request_body).await;
        });
    }
    let stream = ReceiverStream::new(receiver).map(Ok::<_, Infallible>);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
}

async fn stream_recorded_events(
    response_chunks: &mpsc::Sender<String>,
    stream_path: &str,
    request_body: &Value,
) {
    let response = record_or_read_stream(stream_path, request_body).await;
    let event_delay = Duration::from_millis(5);
    let mut events = response.split("\n\n").peekable();
    while let Some(event) = events.next() {
        let mut recorded_event = event.to_owned();
        if events.peek().is_some() {
            recorded_event.push_str("\n\n");
        }
        if response_chunks.send(recorded_event).await.is_err() {
            return;
        }
        tokio::time::sleep(event_delay).await;
    }
}

async fn record_or_read_stream(stream_path: &str, request_body: &Value) -> String {
    if std::env::var_os("SHODAN_RECORD_LLAMA").is_some() {
        let address = std::env::var("SHODAN_LLAMA_ADDRESS")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{address}/v1/chat/completions"))
            .json(request_body)
            .send()
            .await
            .expect("the running server accepts the request")
            .error_for_status()
            .expect("the running server accepts the conversation")
            .text()
            .await
            .expect("the running server streams a response");
        let fixture_path = std::path::Path::new(stream_path);
        std::fs::create_dir_all(
            fixture_path
                .parent()
                .expect("the fixture has a parent directory"),
        )
        .expect("the fixture directory can be created");
        std::fs::write(fixture_path, &response).expect("the response can be recorded");
        response
    } else {
        std::fs::read_to_string(stream_path).expect("the scenario has a recorded provider response")
    }
}

pub async fn receive_request(requests: &mut mpsc::Receiver<CapturedRequest>) -> CapturedRequest {
    let request_timeout = Duration::from_secs(5);
    timeout(request_timeout, requests.recv())
        .await
        .expect("the application sends a request to the model")
        .expect("the mock server remains available")
}

pub fn assert_received_messages(request: &CapturedRequest, messages: &[(&str, &str)]) {
    let messages = messages
        .iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect::<Vec<_>>();
    assert_eq!(request.body["messages"], json!(messages));
}

pub async fn send_recorded_response(request: &CapturedRequest) {
    send_response_bytes(
        request,
        include_str!("fixtures/llama-cpp/chat/lfm3-asteroids-plan.txt"),
    )
    .await;
}

fn split_recorded_response() -> (&'static str, &'static str) {
    let response = include_str!("fixtures/llama-cpp/chat/lfm3-asteroids-plan.txt");
    let content_start = response
        .find("\"content\":\"")
        .expect("the response has content");
    let content_event_end = response[content_start..]
        .find("\n\n")
        .expect("the content event ends")
        + content_start
        + 2;
    response.split_at(content_event_end)
}

pub async fn send_initial_recorded_response(request: &CapturedRequest) {
    send_response_bytes(request, split_recorded_response().0).await;
}

pub async fn send_remaining_recorded_response(request: &CapturedRequest) {
    send_response_bytes(request, split_recorded_response().1).await;
}

pub async fn wait_for_response_disconnect(request: &CapturedRequest) {
    let disconnect_timeout = Duration::from_secs(5);
    timeout(disconnect_timeout, request.response_chunks.closed())
        .await
        .expect("the application disconnects from the response");
}

pub async fn send_response_bytes(request: &CapturedRequest, response: &str) {
    request
        .response_chunks
        .send(response.to_owned())
        .await
        .expect("the application receives the recorded response");
}

pub async fn receive_request_or_error(
    requests: &mut mpsc::Receiver<CapturedRequest>,
    application: &mut TestApplication,
) -> CapturedRequest {
    tokio::select! {
        request = receive_request(requests) => request,
        screen = application.wait_for_screen(|screen| read_screen(screen).contains("Request failed:")) => {
            panic!("{}", read_screen(&screen));
        }
    }
}

struct SharedTerminal {
    backend: TestBackend,
    frames: watch::Sender<Buffer>,
}

impl Backend for SharedTerminal {
    type Error = Infallible;

    fn draw<'a, Content>(&mut self, content: Content) -> Result<(), Self::Error>
    where
        Content: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.backend.draw(content)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.backend.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.backend.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.backend.get_cursor_position()
    }

    fn set_cursor_position<Coordinate: Into<Position>>(
        &mut self,
        position: Coordinate,
    ) -> Result<(), Self::Error> {
        self.backend.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.backend.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.backend.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.backend.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.backend.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let backend = &mut self.backend;
        backend.flush()?;
        self.frames.send_replace(backend.buffer().clone());
        Ok(())
    }
}

pub struct TestApplication {
    events: mpsc::UnboundedSender<io::Result<Event>>,
    frames: watch::Receiver<Buffer>,
    task: JoinHandle<io::Result<()>>,
}

pub fn run_test_application(server_address: String, width: u16, height: u16) -> TestApplication {
    let backend = TestBackend::new(width, height);
    let (frames, receiver) = watch::channel(backend.buffer().clone());
    let backend = SharedTerminal { backend, frames };
    let (events, event_receiver) = mpsc::unbounded_channel();
    let (application, interface, updates) = build_application(
        server_address,
        backend,
        UnboundedReceiverStream::new(event_receiver),
        TerminalMode::Unmanaged,
    )
    .expect("the feature test builds the application");
    let task = tokio::spawn(run_application(application, interface, updates));
    TestApplication {
        events,
        frames: receiver,
        task,
    }
}

impl TestApplication {
    pub fn press_key(&self, code: KeyCode) {
        let key = KeyEvent::new(code, KeyModifiers::NONE);
        self.events
            .send(Ok(Event::Key(key)))
            .expect("the application receives terminal input");
    }

    pub fn press_control_key(&self, character: char) {
        let key = KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL);
        self.events
            .send(Ok(Event::Key(key)))
            .expect("the application receives terminal input");
    }

    pub fn scroll_mouse(&self, kind: MouseEventKind, column: u16, row: u16) {
        let mouse = MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        self.events
            .send(Ok(Event::Mouse(mouse)))
            .expect("the application receives terminal input");
    }

    pub fn enter_text(&self, text: &str) {
        for character in text.chars() {
            self.press_key(KeyCode::Char(character));
        }
    }

    pub fn send_message(&self, message: &str) {
        self.enter_text(message);
        self.press_key(KeyCode::Enter);
    }

    pub async fn wait_for_screen(&mut self, matches: impl Fn(&Buffer) -> bool) -> Buffer {
        let screen_timeout = Duration::from_secs(5);
        timeout(screen_timeout, async {
            loop {
                let screen = self.frames.borrow_and_update().clone();
                if matches(&screen) {
                    return screen;
                }
                self.frames
                    .changed()
                    .await
                    .expect("the application continues drawing");
            }
        })
        .await
        .expect("the application displays the expected screen")
    }
}

impl Drop for TestApplication {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn read_screen(buffer: &Buffer) -> String {
    buffer.content().iter().map(Cell::symbol).collect()
}

pub fn read_history(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width);
    let height = usize::from(buffer.area.height);
    buffer.content()[..height.saturating_sub(4) * width]
        .iter()
        .map(Cell::symbol)
        .collect()
}

pub fn read_bottom_line(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width);
    let height = usize::from(buffer.area.height);
    buffer.content()[(height - 1) * width..]
        .iter()
        .map(Cell::symbol)
        .collect()
}
