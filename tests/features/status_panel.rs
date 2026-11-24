use super::support::{
    ASTEROIDS_PLAN_PROMPT, RecordedModels, assert_received_messages, read_bottom_line,
    read_history, receive_request, run_test_application, send_recorded_response, start_mock_server,
};
use std::time::Duration;

use crossterm::event::KeyCode;

#[tokio::test]
async fn user_views_session_statistics() {
    let (server_address, mut requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 120, 12);

    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    assert_received_messages(
        &request,
        &[
            (
                "system",
                "Answer directly and accurately using the fewest words needed. For practical requests, give specific, actionable steps. Omit introductions, summaries, repetition, and unsolicited advice. Add detail only when needed or requested.",
            ),
            ("user", ASTEROIDS_PLAN_PROMPT),
        ],
    );
    send_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑82 ↓383 CH0.0%"))
        .await;

    let status = read_bottom_line(&screen);
    let provider_position = status
        .find("llama.cpp")
        .expect("the status panel displays the provider");
    let provider_column = status[..provider_position].chars().count();
    assert!(provider_column >= 60);
    assert!(status.contains("LFM3-Omni-2B-Q4_K_M"));
}

#[tokio::test]
async fn user_scrolls_conversation() {
    let (server_address, mut requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 120, 12);

    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑82 ↓383 CH0.0%"))
        .await;
    let status = read_bottom_line(&screen);

    application.press_key(KeyCode::PageUp);
    let scrolled_screen = application
        .wait_for_screen(|scrolled| read_history(scrolled) != read_history(&screen))
        .await;
    assert_eq!(read_bottom_line(&scrolled_screen), status);
}

#[tokio::test]
async fn user_views_response_time() {
    let (server_address, mut requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 120, 12);

    application
        .wait_for_screen(|screen| {
            let status = read_bottom_line(screen);
            status.starts_with("↑0 ↓0 CH0.0%") && status.contains("LFM3-Omni-2B-Q4_K_M")
        })
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    let screen = application
        .wait_for_screen(|screen| {
            read_bottom_line(screen)
                .chars()
                .any(|character| ('⠁'..='⣿').contains(&character))
        })
        .await;
    let status = read_bottom_line(&screen);
    assert!(status.ends_with("llama.cpp LFM3-Omni-2B-Q4_K_M"));
    let spinner = status
        .chars()
        .find(|character| ('⠁'..='⣿').contains(character))
        .expect("the status panel displays a spinner");
    assert!(status.starts_with(&format!("{spinner} 0s ↑0 ↓0 CH0.0%")));
    application
        .wait_for_screen(|screen| {
            read_bottom_line(screen)
                .chars()
                .any(|character| ('⠁'..='⣿').contains(&character) && character != spinner)
        })
        .await;

    tokio::time::pause();
    tokio::time::advance(Duration::from_millis(5200)).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains(" 5s ↑0 ↓0 CH0.0%"))
        .await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).starts_with("⏱ 5s ↑82 ↓383 CH0.0%"))
        .await;
    tokio::time::resume();

    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(72)).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains(" 1m 12s ↑82 ↓383 CH0.0%"))
        .await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).starts_with("⏱ 1m 12s ↑164 ↓766 CH0.0%"))
        .await;
    tokio::time::resume();
}
