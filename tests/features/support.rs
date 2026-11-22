use std::{convert::Infallible, io, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderValue, header},
    response::Response,
    routing::post,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
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

pub struct CapturedRequest {
    pub body: Value,
    response_chunks: mpsc::Sender<String>,
}

#[derive(Clone)]
struct MockServer {
    requests: mpsc::Sender<CapturedRequest>,
}

async fn stream_response(State(server): State<MockServer>, Json(body): Json<Value>) -> Response {
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

pub async fn start_mock_server() -> (String, mpsc::Receiver<CapturedRequest>) {
    let (requests, receiver) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the mock server binds to a local port");
    let address = listener
        .local_addr()
        .expect("the mock server has a local address");
    let router = Router::new()
        .route("/v1/chat/completions", post(stream_response))
        .with_state(MockServer { requests });
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

pub async fn send_response_chunk(request: &CapturedRequest, content: &str) {
    let chunk = json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "test-model",
        "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": null}]
    });
    request
        .response_chunks
        .send(format!("data: {chunk}\n\n"))
        .await
        .expect("the application receives the streamed response");
}

pub async fn send_multiline_response_chunk(request: &CapturedRequest, content: &str) {
    let content = json!(content);
    request
        .response_chunks
        .send("data: {\"id\":\"chatcmpl-test\",\n".to_owned())
        .await
        .expect("the application receives the streamed response");
    request.response_chunks.send(format!("data: \"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"test-model\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":{content}}},\"finish_reason\":null}}]}}\n\n")).await.expect("the application receives the streamed response");
}

pub async fn finish_response(request: &CapturedRequest, usage: Option<Value>) {
    let chunk = json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "test-model",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });
    let usage_chunk = usage.map_or_else(String::new, |usage| {
        let chunk = json!({
            "id": "chatcmpl-test",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": "test-model",
            "choices": [],
            "usage": usage
        });
        format!("data: {chunk}\n\n")
    });
    request
        .response_chunks
        .send(format!("data: {chunk}\n\n{usage_chunk}data: [DONE]\n\n"))
        .await
        .expect("the application receives the end of the response");
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
    buffer.content()[..height.saturating_sub(2) * width]
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
