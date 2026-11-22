use crossterm::event::KeyCode;
use serde_json::json;

use super::support::{
    finish_response, read_bottom_line, read_history, read_screen, receive_request,
    run_test_application, send_response_chunk, start_mock_server,
};

#[tokio::test]
async fn user_views_session_statistics() {
    let (server_address, mut requests) = start_mock_server().await;
    let mut application = run_test_application(server_address, 120, 12);

    application.send_message("explain quantum mechanics");
    let request = receive_request(&mut requests).await;
    let response = include_str!("fixtures/feynman-response.txt");
    send_response_chunk(&request, response).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("comfortable picture"))
        .await;
    let usage = json!({
        "prompt_tokens": 52000,
        "completion_tokens": 12000,
        "total_tokens": 64000,
        "prompt_tokens_details": {"cached_tokens": 51844}
    });
    finish_response(&request, Some(usage)).await;
    let screen = application
        .wait_for_screen(|screen| read_bottom_line(screen).starts_with("↑156 ↓12k CH99.7%"))
        .await;

    let status = read_bottom_line(&screen);
    let provider_position = status
        .find("llama.cpp")
        .expect("the status panel displays the provider");
    let provider_column = status[..provider_position].chars().count();
    assert!(provider_column >= 60);
    assert!(status.contains("test-model"));
    assert!(status.contains('⏱'));

    application.press_key(KeyCode::PageUp);
    let scrolled_screen = application
        .wait_for_screen(|scrolled| read_history(scrolled) != read_history(&screen))
        .await;
    assert_eq!(read_bottom_line(&scrolled_screen), status);
}
