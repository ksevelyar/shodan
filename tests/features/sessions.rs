use super::support::{
    RecordedProvider, create_home, read_bottom_line, read_screen, receive_request,
    run_test_application, send_recorded_response,
};
use shodan::settings::build_settings;

#[tokio::test]
async fn user_resumes_session() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.send_message("first message");
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    drop(application);
    let current_directory = std::env::current_dir().expect("the project directory is available");
    let sessions = home.path().join(".shodan/sessions").join(
        current_directory
            .strip_prefix("/")
            .expect("the project directory is absolute"),
    );
    assert_eq!(
        std::fs::read_dir(sessions)
            .expect("the session directory exists")
            .count(),
        1
    );
    let settings = build_settings(
        home.path().to_owned(),
        vec!["--session".to_owned(), "0".to_owned()],
    )
    .expect("settings are valid");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_screen(screen).contains("first message"))
        .await;
    application.send_message("second message");
    let request = receive_request(&mut requests).await;
    let messages = request.body["messages"]
        .as_array()
        .expect("messages are present");
    assert_eq!(messages[1]["content"], "first message");
    assert_eq!(
        messages.last().expect("the new message is present")["content"],
        "second message"
    );
}

#[tokio::test]
async fn user_selects_older_session() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider":"llama", "providers":{"llama":{"address":address, "default_model":"model", "models":[{"name":"model"}]}}}),
    );
    for message in ["first message", "second message"] {
        let settings =
            build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
        let mut application = run_test_application(settings);
        application.send_message(message);
        let request = receive_request(&mut requests).await;
        send_recorded_response(&request).await;
        application
            .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
            .await;
        drop(application);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let settings = build_settings(
        home.path().to_owned(),
        vec!["--session".to_owned(), "1".to_owned()],
    )
    .expect("settings are valid");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_screen(screen).contains("first message"))
        .await;
    application.send_message("updated message");
    let request = receive_request(&mut requests).await;
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
        .wait_for_screen(|screen| read_screen(screen).contains("first message"))
        .await;
    application.send_message("verify");
    let request = receive_request(&mut requests).await;
    assert!(
        request.body["messages"]
            .as_array()
            .expect("messages are present")
            .iter()
            .any(|message| message["content"] == "updated message")
    );
}

#[tokio::test]
async fn user_resumes_project_session_with_its_context_model_and_tool_results() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider":"llama", "providers":{"llama":{"address":address, "default_model":"model", "models":[{"name":"model"}, {"name":"another"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.press_key(crossterm::event::KeyCode::Tab);
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| {
            read_screen(screen).starts_with("edit") && read_bottom_line(screen).contains("another")
        })
        .await;
    application.send_message("Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.");
    let request = receive_request(&mut requests).await;
    super::tools::support::send_scenario_response(&request, "user_reads_file", 1).await;
    let request = receive_request(&mut requests).await;
    super::tools::support::send_scenario_response(&request, "user_reads_file", 2).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
        .await;
    drop(application);
    let other_project = home.path().join("other-project");
    std::fs::create_dir(&other_project).expect("the other project exists");
    let mut process = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home.path())
        .current_dir(&other_project)
        .args(["--prompt", "other project message"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts");
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    assert!(process.wait().await.expect("Shodan exits").success());
    let project_directory = std::env::current_dir().expect("the project directory exists");
    for directory in [&project_directory, &other_project] {
        let directory = home.path().join(".shodan/sessions").join(
            directory
                .strip_prefix("/")
                .expect("project path is absolute"),
        );
        assert_eq!(
            std::fs::read_dir(directory)
                .expect("the project sessions exist")
                .count(),
            1
        );
    }
    let settings = build_settings(
        home.path().to_owned(),
        vec!["--session".to_owned(), "0".to_owned()],
    )
    .expect("settings are valid");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| {
            read_screen(screen).starts_with("edit") && read_bottom_line(screen).contains("another")
        })
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    let messages = request.body["messages"]
        .as_array()
        .expect("messages are present");
    assert!(
        messages
            .iter()
            .any(|message| message["tool_calls"].is_array())
    );
    assert!(messages.iter().any(|message| {
        message["role"] == "tool"
            && message["content"]
                .as_str()
                .is_some_and(|text| text.contains("Hello, Shodan!"))
    }));
    assert_eq!(request.body["model"], "another");
    let mut process = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home.path())
        .current_dir(&other_project)
        .args(["--session", "0", "--prompt", "other continuation"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts");
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][1]["content"],
        "other project message"
    );
    send_recorded_response(&request).await;
    assert!(process.wait().await.expect("Shodan exits").success());
}

#[tokio::test]
async fn user_selects_unavailable_session() {
    let (address, _requests) = RecordedProvider::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider":"llama", "providers":{"llama":{"address":address, "default_model":"model", "models":[{"name":"model"}]}}}),
    );
    let error = build_settings(
        home.path().to_owned(),
        vec!["--session".to_owned(), "8".to_owned()],
    )
    .err()
    .expect("the unavailable session returns an error");
    assert!(error.to_string().contains("session 8"));
}
