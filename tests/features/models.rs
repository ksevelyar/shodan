use std::fs;

use shodan::settings::build_settings;

use super::support::{
    CapturedRequest, RecordedLlama, create_home, read_bottom_line, read_screen, receive_request,
    run_test_application, send_response_bytes,
};

async fn send_recorded_chat_response(request: &CapturedRequest, fixture_name: &str) {
    let fixture_path = format!("tests/features/fixtures/llama/chat/{fixture_name}");
    let response =
        fs::read_to_string(&fixture_path).expect("the scenario has a recorded provider response");
    send_response_bytes(request, &response).await;
}

#[tokio::test]
async fn user_chats_with_inkling_at_its_configured_thinking_level() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
                "models": [
                    {
                        "name": "Inkling-2-Mini-UD-IQ3_XXS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
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
async fn user_cycles_through_configured_models() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
                "models": [
                    {
                        "name": "Inkling-2-Mini-UD-IQ3_XXS",
                        "thinking_level": "low"
                    },
                    {
                        "name": "Qwen3.8-27B-UD-IQ4_XS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
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
    send_recorded_chat_response(&request, "qwen-hello-low.txt").await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello! How can I help you today?"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
}
