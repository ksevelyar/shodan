use super::support::{RecordedLlama, read_bottom_line, run_test_application};

#[tokio::test]
async fn user_starts_session_with_two_loaded_models() {
    let (server_address, _requests) = RecordedLlama::new()
        .mock_available_models(
            "tests/features/fixtures/llama-cpp/models/lfm3-and-minicpm-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
}

#[tokio::test]
async fn user_starts_session_with_no_loaded_models() {
    let (server_address, _requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/none-loaded.json")
        .mock_model_load(
            "LFM3-Omni-2B-Q4_K_M",
            "tests/features/fixtures/llama-cpp/models/lfm3-loading.json",
            "tests/features/fixtures/llama-cpp/models/lfm3-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
}

#[tokio::test]
async fn user_starts_session_with_second_model_loaded() {
    let (server_address, _requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/minicpm-loaded.json")
        .start()
        .await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("MiniCPM5-2B-Q4_K_M"))
        .await;
}
