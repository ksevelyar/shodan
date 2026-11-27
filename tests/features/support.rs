mod shared;

pub use shared::*;

use std::{collections::VecDeque, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::{Buffer, Cell};
use serde_json::json;
use shodan::settings::ApplicationSettings;
use tokio::{net::TcpListener, sync::mpsc, time::timeout};

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

impl RecordedProvider {
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

impl TestApplication {
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
}

pub fn read_history(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width);
    let height = usize::from(buffer.area.height);
    buffer.content()[width.saturating_mul(2)..height.saturating_sub(4) * width]
        .iter()
        .map(Cell::symbol)
        .collect()
}
