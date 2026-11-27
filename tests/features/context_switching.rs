use std::process::Stdio;

use crossterm::event::KeyCode;
use shodan::settings::build_settings;

use super::support::{
    RecordedProvider, create_home, read_bottom_line, read_history, read_screen, receive_request,
    run_test_application, send_initial_recorded_response, send_recorded_response,
    wait_for_response_disconnect,
};

#[tokio::test]
async fn user_starts_session_with_project_instructions() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    for (context, prompt) in [
        ("read", "Read instructions"),
        ("edit", "Edit instructions"),
        ("review", "Review instructions"),
    ] {
        std::fs::write(home.path().join(format!(".shodan/{context}.md")), prompt)
            .expect("the context prompt can be written");
    }
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    let screen = application
        .wait_for_screen(|screen| {
            read_screen(screen).starts_with("read")
                && read_history(screen).contains("Read instructions")
        })
        .await;
    let instruction_row = (0..screen.area.height)
        .find(|row| screen[(0, *row)].symbol() == "R")
        .expect("the context instructions are visible");
    assert_eq!(
        screen[(0, instruction_row)].fg,
        ratatui::style::Color::Rgb(213, 170, 42)
    );
    let empty_line = (0..screen.area.width)
        .map(|column| screen[(column, instruction_row + 1)].symbol())
        .collect::<String>();
    assert_eq!(empty_line.trim(), "");
    assert_eq!(screen[(0, instruction_row + 2)].symbol(), "*");
    for (context, prompt) in [
        ("read", "Read instructions"),
        ("edit", "Edit instructions"),
        ("review", "Review instructions"),
    ] {
        let contents = std::fs::read_to_string(home.path().join(format!(".shodan/{context}.md")))
            .expect("the context prompt exists");
        assert_eq!(contents, prompt);
    }
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Read instructions\n\n{}", include_str!("../../AGENTS.md"))
    );
    let names: Vec<_> = request.body["tools"]
        .as_array()
        .expect("tools are present")
        .iter()
        .map(|tool| tool["function"]["name"].as_str().expect("tool has a name"))
        .collect();
    assert_eq!(names, ["bash", "ls", "read", "grep", "find"]);
}

#[tokio::test]
async fn user_switches_contexts_and_keeps_conversation() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    std::fs::write(home.path().join(".shodan/read.md"), "Read instructions")
        .expect("the read prompt can be written");
    std::fs::write(home.path().join(".shodan/edit.md"), "Edit instructions")
        .expect("the edit prompt can be written");
    std::fs::write(home.path().join(".shodan/review.md"), "Review instructions")
        .expect("the review prompt can be written");
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    let conversation = read_history(&screen);
    application.press_key(KeyCode::Tab);
    let screen = application
        .wait_for_screen(|screen| read_screen(screen).starts_with("edit"))
        .await;
    assert_eq!(read_history(&screen), conversation);
    for _ in 0..10 {
        application.press_key(KeyCode::PageUp);
    }
    application
        .wait_for_screen(|screen| read_history(screen).contains("Edit instructions"))
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Edit instructions\n\n{}", include_str!("../../AGENTS.md"))
    );
    assert_eq!(request.body["messages"][1]["content"], "Hello");
    assert_eq!(request.body["messages"][2]["role"], "assistant");
    assert_eq!(request.body["messages"][3]["content"], "Continue");
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        7
    );
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("review"))
        .await;
    application.send_message("Review");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][0]["content"],
        format!("Review instructions\n\n{}", include_str!("../../AGENTS.md"))
    );
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        7
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
        7
    );
    application.press_key(KeyCode::Tab);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("read"))
        .await;
}

#[tokio::test]
async fn user_starts_new_session_with_last_model_response() {
    let (address, mut requests) = RecordedProvider::new().start().await;
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
    application.press_control_tab();
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("edit"))
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][1]["role"], "assistant");
    assert_eq!(request.body["messages"][2]["content"], "Continue");
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    drop(application);
    let settings = build_settings(
        home.path().to_owned(),
        vec!["--session".to_owned(), "0".to_owned()],
    )
    .expect("settings are valid");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_screen(screen).starts_with("edit"))
        .await;
    application.send_message("Next");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][1]["role"], "assistant");
    assert_eq!(request.body["messages"][2]["content"], "Continue");
}

#[tokio::test]
async fn user_starts_without_prompt_files() {
    let (address, mut requests) = RecordedProvider::new().start().await;
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
    for context in ["read", "edit", "review"] {
        let contents = std::fs::read_to_string(home.path().join(format!(".shodan/{context}.md")))
            .expect("the context prompt exists");
        assert_eq!(contents, "");
    }
    send_recorded_response(&request).await;
    assert!(process.wait().await.expect("Shodan exits").success());
}

#[tokio::test]
async fn user_starts_with_unavailable_prompt_file() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    std::fs::create_dir(home.path().join(".shodan/read.md"))
        .expect("the read prompt path is a directory");
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
async fn user_switches_context_during_response() {
    let (address, mut requests) = RecordedProvider::new().start().await;
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
        5
    );
    assert_eq!(request.body["messages"][1]["content"], "First");
    assert_eq!(request.body["messages"][2]["role"], "assistant");
    assert_eq!(request.body["messages"][3]["content"], "Second");
    assert_eq!(request.body["messages"][4]["content"], "Continue");
}
