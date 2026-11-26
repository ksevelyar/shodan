use std::process::Stdio;

use crossterm::event::KeyCode;
use shodan::settings::build_settings;

use super::support::{
    RecordedLlama, create_home, read_bottom_line, read_history, read_screen, receive_request,
    run_test_application, send_initial_recorded_response, send_recorded_response,
    wait_for_response_disconnect,
};

#[tokio::test]
async fn user_starts_session_with_project_instructions() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    std::fs::write(home.path().join(".shodan/read.md"), "Read instructions")
        .expect("the read prompt can be written");
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| {
            read_screen(screen).starts_with("read")
                && read_history(screen).contains("Read instructions")
        })
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Read instructions\n{}", include_str!("../../AGENTS.md"))
    );
    let names: Vec<_> = request.body["tools"]
        .as_array()
        .expect("tools are present")
        .iter()
        .map(|tool| tool["function"]["name"].as_str().expect("tool has a name"))
        .collect();
    assert_eq!(names, ["bash", "ls", "read"]);
}

#[tokio::test]
async fn user_switches_contexts() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    std::fs::write(home.path().join(".shodan/edit.md"), "Edit instructions")
        .expect("the edit prompt can be written");
    std::fs::write(home.path().join(".shodan/review.md"), "Review instructions")
        .expect("the review prompt can be written");
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| {
            read_screen(screen).starts_with("edit")
                && read_history(screen).contains("Edit instructions")
        })
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Edit instructions\n{}", include_str!("../../AGENTS.md"))
    );
    assert_eq!(request.body["messages"][1]["role"], "assistant");
    assert_eq!(request.body["messages"][2]["content"], "Continue");
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        4
    );
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("review"))
        .await;
    application.send_message("Review");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Review instructions\n{}", include_str!("../../AGENTS.md"))
    );
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        4
    );
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("blank"))
        .await;
    application.send_message("Blank");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][0]["content"], "");
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        4
    );
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("read"))
        .await;
}

#[tokio::test]
async fn user_starts_without_prompt_files() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let mut process = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home.path())
        .current_dir(home.path())
        .args(["--prompt", "Hello"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][0]["content"], "");
    send_recorded_response(&request).await;
    assert!(process.wait().await.expect("Shodan exits").success());
}

#[tokio::test]
async fn user_switches_during_response() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.send_message("First");
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    application.send_message("Second");
    let request = receive_request(&mut requests).await;
    send_initial_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Second"))
        .await;
    application.press_key(KeyCode::Tab);
    wait_for_response_disconnect(&request).await;
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("edit"))
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"]
            .as_array()
            .expect("messages are present")
            .len(),
        3
    );
    assert_eq!(request.body["messages"][1]["role"], "assistant");
    assert_eq!(request.body["messages"][2]["content"], "Continue");
}
