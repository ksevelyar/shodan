use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use shodan::{application::Application, interface::Interface};

use super::support::{
    finish_response, receive_request, send_message, send_response_chunk, start_mock_server,
    wait_for_update,
};

fn read_bottom_line(interface: &Interface, terminal: &mut Terminal<TestBackend>) -> String {
    terminal
        .draw(|frame| interface.draw(frame))
        .expect("the interface draws in the test terminal");
    let buffer = terminal.backend().buffer();
    let width = usize::from(buffer.area.width);
    let height = usize::from(buffer.area.height);
    buffer.content()[(height - 1) * width..]
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[tokio::test]
async fn user_views_session_statistics() {
    let (server_address, mut requests) = start_mock_server().await;
    let application = Application::build(server_address);
    let mut interface = Interface::new(application);
    let mut terminal = Terminal::new(TestBackend::new(120, 12))
        .expect("the feature test creates a terminal viewport");

    send_message(&mut interface, "explain quantum mechanics");
    let request = receive_request(&mut requests).await;
    let response = include_str!("fixtures/feynman-response.txt");
    send_response_chunk(&request, response).await;
    wait_for_update(&mut interface).await;
    let usage = json!({
        "prompt_tokens": 52000,
        "completion_tokens": 12000,
        "total_tokens": 64000,
        "prompt_tokens_details": {"cached_tokens": 51844}
    });
    finish_response(&request, Some(usage)).await;
    wait_for_update(&mut interface).await;

    let status = read_bottom_line(&interface, &mut terminal);
    assert!(status.starts_with("↑156 ↓12k CH99.7%"));
    let provider_position = status
        .find("llama.cpp")
        .expect("the status panel displays the provider");
    let provider_column = status[..provider_position].chars().count();
    assert!(provider_column >= 60);
    assert!(status.contains("test-model"));
    assert!(status.contains('⏱'));

    interface.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(read_bottom_line(&interface, &mut terminal), status);
}
