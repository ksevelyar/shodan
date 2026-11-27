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
    http::{HeaderValue, header},
    response::Response,
    routing::post,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    backend::{Backend, ClearType, TestBackend, WindowSize},
    buffer::{Buffer, Cell},
    layout::{Position, Size},
};
use serde_json::{Value, json};
use shodan::{application::run, interface::TerminalMode, settings::ApplicationSettings};
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

pub static LOGIN_PORT: LoginPort = LoginPort(tokio::sync::Mutex::const_new(()));

pub struct LoginPort(tokio::sync::Mutex<()>);

impl LoginPort {
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        let guard = self.0.lock().await;
        let port_timeout = Duration::from_secs(5);
        timeout(port_timeout, async {
            loop {
                if TcpListener::bind("127.0.0.1:1455").await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the login callback port becomes available");
        guard
    }
}

pub const ASTEROIDS_PLAN_PROMPT: &str = "Write a proof-of-concept plan for a monochrome, 2D, low-poly Asteroids game in Bevy. Under ## Requirements, describe the game’s behavior without implementation details. Under ## User scenarios, write scenarios derived from those requirements. Name each scenario using ### 1. User ...";

pub struct CapturedRequest {
    pub body: Value,
    response_chunks: mpsc::Sender<String>,
}

#[derive(Default)]
pub struct RecordedLlama {
    model_streams: Vec<(&'static str, VecDeque<String>)>,
}

impl RecordedLlama {
    pub fn new() -> Self {
        Self::default()
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
        self.start_at_path("/v1/chat/completions").await
    }

    pub async fn start_openai(self) -> (String, mpsc::Receiver<CapturedRequest>) {
        self.start_at_path("/v1/responses").await
    }

    async fn start_at_path(self, path: &str) -> (String, mpsc::Receiver<CapturedRequest>) {
        let (requests, receiver) = mpsc::channel(8);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the mock server binds to a local port");
        let address = listener
            .local_addr()
            .expect("the mock server has a local address");
        let router = Router::new()
            .route(path, post(stream_response))
            .route(
                "/api/accounts/oauth/token",
                post(refresh_openai_credentials),
            )
            .with_state(LlamaState {
                requests,
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

async fn refresh_openai_credentials(body: String) -> (axum::http::StatusCode, Json<Value>) {
    let (status, response) = if body.contains("client_id=invalid-client") {
        (
            axum::http::StatusCode::BAD_REQUEST,
            include_str!("fixtures/openai/refresh-error.json"),
        )
    } else if body.contains("grant_type=authorization_code") {
        (
            axum::http::StatusCode::OK,
            include_str!("fixtures/openai/login.json"),
        )
    } else {
        (
            axum::http::StatusCode::OK,
            include_str!("fixtures/openai/refresh.json"),
        )
    };
    (
        status,
        Json(serde_json::from_str(response).expect("the recorded token response is valid JSON")),
    )
}

type RecordedStreams = Vec<(&'static str, VecDeque<String>)>;

#[derive(Clone)]
struct LlamaState {
    requests: mpsc::Sender<CapturedRequest>,
    model_streams: Arc<Mutex<RecordedStreams>>,
}

async fn stream_response(State(server): State<LlamaState>, Json(body): Json<Value>) -> Response {
    let requested_model = body["model"]
        .as_str()
        .expect("the chat request names a model");
    let stream_fixture = server
        .model_streams
        .lock()
        .expect("the mock server state is not poisoned")
        .iter_mut()
        .find(|(streamed_model, _)| *streamed_model == requested_model)
        .and_then(|(_, streams)| streams.pop_front());
    let (response_chunks, receiver) = mpsc::channel::<String>(8);
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
            stream_recorded_events(&response_chunks, &stream_path).await;
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

async fn stream_recorded_events(response_chunks: &mpsc::Sender<String>, stream_path: &str) {
    let response = std::fs::read_to_string(stream_path)
        .expect("the scenario has a recorded provider response");
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
        include_str!("fixtures/llama/chat/lfm3-asteroids-plan.txt"),
    )
    .await;
}

fn split_recorded_response() -> (&'static str, &'static str) {
    let response = include_str!("fixtures/llama/chat/lfm3-asteroids-plan.txt");
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

pub struct TestHome {
    path: std::path::PathBuf,
}

impl TestHome {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).expect("the temporary home can be removed");
    }
}

pub fn create_home(configuration: Value) -> TestHome {
    let Value::Object(configuration) = configuration else {
        panic!("the provider configuration must be an object");
    };
    let path = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let directory = path.join(".shodan");
    std::fs::create_dir_all(&directory).expect("the configuration directory can be created");
    let file = std::fs::File::create(directory.join("settings.json"))
        .expect("the configuration file can be created");
    serde_json::to_writer(file, &configuration).expect("the configuration can be written");
    TestHome { path }
}

pub struct TestApplication {
    events: mpsc::UnboundedSender<io::Result<Event>>,
    frames: watch::Receiver<Buffer>,
    task: JoinHandle<Result<(), shodan::application::ApplicationError>>,
}

pub fn run_test_application(settings: ApplicationSettings) -> TestApplication {
    run_test_application_with_configured_clipboard(
        settings.with_clipboard_command(vec!["true".to_owned()]),
    )
}

pub fn run_test_application_with_configured_clipboard(
    settings: ApplicationSettings,
) -> TestApplication {
    run_test_application_with_size(settings, 120, 24)
}

pub fn run_test_application_with_size(
    settings: ApplicationSettings,
    width: u16,
    height: u16,
) -> TestApplication {
    let backend = TestBackend::new(width, height);
    let (frames, receiver) = watch::channel(backend.buffer().clone());
    let backend = SharedTerminal { backend, frames };
    let (events, event_receiver) = mpsc::unbounded_channel();
    let task = tokio::spawn(run(settings, || {
        (
            backend,
            UnboundedReceiverStream::new(event_receiver),
            TerminalMode::Unmanaged,
        )
    }));
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

    pub fn press_shift_enter(&self) {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        self.events
            .send(Ok(Event::Key(key)))
            .expect("the application receives terminal input");
    }

    pub fn paste_text(&self, text: &str) {
        self.events
            .send(Ok(Event::Paste(text.to_owned())))
            .expect("the application receives terminal input");
    }

    pub fn press_control_end(&self) {
        let key = KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL);
        self.events
            .send(Ok(Event::Key(key)))
            .expect("the application receives terminal input");
    }

    pub fn press_control_tab(&self) {
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL);
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
        .unwrap_or_else(|_| {
            panic!(
                "the application displays the expected screen; last screen:\n{}",
                read_screen(&self.frames.borrow())
            )
        })
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
    buffer.content()[width..height.saturating_sub(4) * width]
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
