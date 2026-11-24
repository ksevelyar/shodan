use super::support::{
    RecordedModels, read_bottom_line, read_screen, receive_request, run_test_application,
    send_response_bytes, start_mock_server,
};

#[tokio::test]
async fn user_starts_session_with_preferred_model_unloaded() {
    let (address, _requests) = start_mock_server(RecordedModels::QwenUnloaded).await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
}

#[tokio::test]
async fn user_chats_with_qwen_at_its_default_thinking_level() {
    let (address, mut requests) = start_mock_server(RecordedModels::QwenLoaded).await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["reasoning_effort"], "low");
    send_response_bytes(
        &request,
        include_str!("fixtures/llama-cpp/chat/qwen-hello-low.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello! How can I help you today?"))
        .await;
}
