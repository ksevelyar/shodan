use std::{convert::Infallible, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderValue, header},
    response::Response,
    routing::post,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use shodan::interface::Interface;
use tokio::{net::TcpListener, sync::mpsc, time::timeout};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

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

pub fn send_message(interface: &mut Interface, message: &str) {
    for character in message.chars() {
        interface.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    interface.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

pub async fn wait_for_update(interface: &mut Interface) {
    let update_timeout = Duration::from_secs(5);
    timeout(update_timeout, interface.wait_for_update())
        .await
        .expect("the interface receives a model response");
}

pub fn render_interface(interface: &Interface, terminal: &mut Terminal<TestBackend>) -> String {
    terminal
        .draw(|frame| interface.draw(frame))
        .expect("the interface draws in the test terminal");
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}
