use super::support::{RecordedModels, read_bottom_line, run_test_application, start_mock_server};

#[tokio::test]
async fn user_starts_session_with_two_loaded_models() {
    let (server_address, _requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
}

#[tokio::test]
async fn user_starts_session_with_no_loaded_models() {
    let (server_address, _requests) = start_mock_server(RecordedModels::NoneRunning).await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
}

#[tokio::test]
async fn user_starts_session_with_second_model_loaded() {
    let (server_address, _requests) = start_mock_server(RecordedModels::SecondLoaded).await;
    let mut application = run_test_application(server_address, 100, 12);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("MiniCPM5-2B-Q4_K_M"))
        .await;
}
