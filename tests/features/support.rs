use std::{
    convert::Infallible,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
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

#[derive(Clone, Copy)]
pub enum RecordedModels {
    BothLoaded,
    NoneRunning,
    SecondLoaded,
    QwenLoaded,
}

#[derive(Clone)]
struct MockServer {
    requests: mpsc::Sender<CapturedRequest>,
    models: RecordedModels,
    load_requests: Arc<AtomicUsize>,
    model_requests: Arc<AtomicUsize>,
}

async fn list_models(
    State(server): State<MockServer>,
) -> ([(header::HeaderName, &'static str); 1], &'static str) {
    let response = match server.models {
        RecordedModels::BothLoaded => include_str!("fixtures/llama-cpp/models/both-loaded.json"),
        RecordedModels::QwenLoaded => include_str!("fixtures/llama-cpp/models/qwen-loaded.json"),
        RecordedModels::SecondLoaded => {
            include_str!("fixtures/llama-cpp/models/minicpm-loaded.json")
        }
        RecordedModels::NoneRunning if server.load_requests.load(Ordering::SeqCst) == 0 => {
            include_str!("fixtures/llama-cpp/models/both-unloaded.json")
        }
        RecordedModels::NoneRunning
            if server.model_requests.fetch_add(1, Ordering::SeqCst) == 0 =>
        {
            include_str!("fixtures/llama-cpp/models/lfm3-loading.json")
        }
        RecordedModels::NoneRunning => include_str!("fixtures/llama-cpp/models/lfm3-loaded.json"),
    };
    ([(header::CONTENT_TYPE, "application/json")], response)
}

async fn load_model(
    State(server): State<MockServer>,
    Json(body): Json<Value>,
) -> ([(header::HeaderName, &'static str); 1], &'static str) {
    assert_eq!(body, json!({"model": "LFM3-Omni-2B-Q4_K_M"}));
    server.load_requests.fetch_add(1, Ordering::SeqCst);
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("fixtures/llama-cpp/load-model/success.json"),
    )
}

async fn stream_response(State(server): State<MockServer>, Json(body): Json<Value>) -> Response {
    if matches!(server.models, RecordedModels::NoneRunning) {
        assert!(
            server.load_requests.load(Ordering::SeqCst) > 0,
            "the application loads the model before chatting"
        );
    }
    let expected_model = match server.models {
        RecordedModels::BothLoaded | RecordedModels::NoneRunning => "LFM3-Omni-2B-Q4_K_M",
        RecordedModels::SecondLoaded => "MiniCPM5-2B-Q4_K_M",
        RecordedModels::QwenLoaded => "Qwen3.8-27B-UD-IQ4_XS",
    };
    let requested_model = body.get("model").and_then(Value::as_str);
    if requested_model.is_none() || requested_model == Some("") {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(include_str!(
                "fixtures/llama-cpp/chat/missing-model-error.json"
            )))
            .expect("the recorded error response is valid");
    }
    assert_eq!(requested_model, Some(expected_model));
    let (response_chunks, receiver) = mpsc::channel::<String>(8);
    server
        .requests
        .send(CapturedRequest {
            body,
            response_chunks,
        })
        .await
        .expect("the feature test receives model requests");
    let stream = ReceiverStream::new(receiver).map(Ok::<_, Infallible>);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
}

pub async fn start_mock_server(
    models: RecordedModels,
) -> (String, mpsc::Receiver<CapturedRequest>) {
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
        .with_state(MockServer {
            requests,
            models,
            load_requests: Arc::new(AtomicUsize::new(0)),
            model_requests: Arc::new(AtomicUsize::new(0)),
        });
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("the mock server serves requests");
    });
    (format!("http://{address}"), receiver)
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
