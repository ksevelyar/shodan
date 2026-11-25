use shodan::settings::build_settings;

use super::support::{
    RecordedLlama, create_home, read_history, receive_request, run_test_application,
    send_response_bytes,
};

#[tokio::test]
async fn user_connects_to_configured_server() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}}),
    );
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "Inkling-2-Mini-UD-IQ3_XXS");
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/inkling-hello-low.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello! How can I help you today?"))
        .await;
}
