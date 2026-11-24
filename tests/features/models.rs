use std::fs;

use super::support::{
    CapturedRequest, RecordedLlama, read_bottom_line, read_screen, receive_request,
    run_test_application, send_response_bytes,
};

async fn send_recorded_chat_response(request: &CapturedRequest, fixture_name: &str) {
    let fixture_path = format!("tests/features/fixtures/llama-cpp/chat/{fixture_name}");
    let response = if std::env::var_os("SHODAN_RECORD_LLAMA").is_some() {
        let address = std::env::var("SHODAN_LLAMA_ADDRESS")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{address}/v1/chat/completions"))
            .json(&request.body)
            .send()
            .await
            .expect("the running server accepts the chat request")
            .error_for_status()
            .expect("the running server accepts the conversation")
            .text()
            .await
            .expect("the running server streams a response");
        fs::write(&fixture_path, &response).expect("the response can be recorded");
        response
    } else {
        fs::read_to_string(&fixture_path).expect("the scenario has a recorded provider response")
    };
    send_response_bytes(request, &response).await;
}

#[tokio::test]
async fn user_starts_session_with_preferred_model_unloaded() {
    let (address, _requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/qwen-loaded.json")
        .mock_model_load(
            "Inkling-2-Mini-UD-IQ3_XXS",
            "tests/features/fixtures/llama-cpp/models/inkling-loading.json",
            "tests/features/fixtures/llama-cpp/models/inkling-loaded.json",
        )
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
}

#[tokio::test]
async fn user_chats_with_inkling_at_its_default_thinking_level() {
    let (address, mut requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/qwen-loaded.json")
        .mock_model_load(
            "Inkling-2-Mini-UD-IQ3_XXS",
            "tests/features/fixtures/llama-cpp/models/inkling-loading.json",
            "tests/features/fixtures/llama-cpp/models/inkling-loaded.json",
        )
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "Inkling-2-Mini-UD-IQ3_XXS");
    assert_eq!(request.body["reasoning_effort"], "low");
    send_recorded_chat_response(&request, "inkling-hello-low.txt").await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello! How can I help you today?"))
        .await;
}

#[tokio::test]
async fn user_cycles_to_qwen_with_ctrl_space() {
    let (address, _requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/qwen-loaded.json")
        .mock_model_load(
            "Inkling-2-Mini-UD-IQ3_XXS",
            "tests/features/fixtures/llama-cpp/models/inkling-loading.json",
            "tests/features/fixtures/llama-cpp/models/inkling-loaded.json",
        )
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
}

#[tokio::test]
async fn user_chats_with_qwen_after_cycling() {
    let (address, mut requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/qwen-loaded.json")
        .mock_model_load(
            "Inkling-2-Mini-UD-IQ3_XXS",
            "tests/features/fixtures/llama-cpp/models/inkling-loading.json",
            "tests/features/fixtures/llama-cpp/models/inkling-loaded.json",
        )
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "Qwen3.8-27B-UD-IQ4_XS");
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

#[tokio::test]
async fn user_cycles_around_back_to_inkling() {
    let (address, _requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/qwen-loaded.json")
        .mock_model_load(
            "Inkling-2-Mini-UD-IQ3_XXS",
            "tests/features/fixtures/llama-cpp/models/inkling-loading.json",
            "tests/features/fixtures/llama-cpp/models/inkling-loaded.json",
        )
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
}
