use super::{
    super::support::{
        RecordedLlama, create_home, read_bottom_line, read_screen, receive_request_or_error,
        run_test_application,
    },
    support::{read_tool_history, send_scenario_response},
};
use shodan::settings::build_settings;

#[tokio::test]
async fn user_lists_directory() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider":"llama", "providers": {"llama": {
            "address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [
                {"name":"Inkling-2-Mini-UD-IQ3_XXS", "thinking_level":"low"},
                {"name":"Qwen3.8-27B-UD-IQ4_XS", "thinking_level":"low"}
            ]
        }}}),
    );
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
    application.send_message("List all entries in tests/features/fixtures/tools/user_lists_directory, including hidden entries, using the ls tool. Show the exact listing.");
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, "user_lists_directory", 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "ls");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).expect("the tool arguments are JSON");
    let expected_arguments =
        serde_json::json!({"path":"tests/features/fixtures/tools/user_lists_directory"});
    assert_eq!(arguments, expected_arguments);
    let calls = assistant["tool_calls"]
        .as_array()
        .expect("the model returned tool calls");
    let expected_tool_results: &[&str] = &[".settings", "notes.txt", "src/"];
    for call in calls {
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool" && message["tool_call_id"] == call["id"])
            .expect("the conversation contains the result for the tool call");
        let tool_content = tool_result["content"].as_str().expect("the result is text");
        for expected in expected_tool_results {
            assert!(tool_content.contains(expected), "{tool_content}");
        }
    }
    let tool_path = expected_arguments["path"]
        .as_str()
        .expect("the tool path is text");
    application
        .wait_for_screen(|screen| {
            let history = read_tool_history(screen);
            history
                .lines()
                .any(|line| line.trim() == format!("ls {tool_path}"))
                && expected_tool_results
                    .iter()
                    .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, "user_lists_directory", 2).await;
    let expected_responses: &[&str] = &["Entries shown", ".settings", "notes.txt", "src/"];
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
}

#[tokio::test]
async fn user_lists_nonexistent_directory() {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider":"llama", "providers": {"llama": {
            "address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [
                {"name":"Inkling-2-Mini-UD-IQ3_XXS", "thinking_level":"low"},
                {"name":"Qwen3.8-27B-UD-IQ4_XS", "thinking_level":"low"}
            ]
        }}}),
    );
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
    application.send_message(
        "List the directory missing-directory using the ls tool and report the result.",
    );
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, "user_lists_nonexistent_directory", 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "ls");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).expect("the tool arguments are JSON");
    let expected_arguments = serde_json::json!({"path":"missing-directory"});
    assert_eq!(arguments, expected_arguments);
    let calls = assistant["tool_calls"]
        .as_array()
        .expect("the model returned tool calls");
    let expected_tool_results: &[&str] = &["Error:"];
    for call in calls {
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool" && message["tool_call_id"] == call["id"])
            .expect("the conversation contains the result for the tool call");
        let tool_content = tool_result["content"].as_str().expect("the result is text");
        for expected in expected_tool_results {
            assert!(tool_content.contains(expected), "{tool_content}");
        }
    }
    let tool_path = expected_arguments["path"]
        .as_str()
        .expect("the tool path is text");
    application
        .wait_for_screen(|screen| {
            let history = read_tool_history(screen);
            history
                .lines()
                .any(|line| line.trim_start().starts_with("ls") && line.contains(tool_path))
                && expected_tool_results
                    .iter()
                    .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, "user_lists_nonexistent_directory", 2).await;
    let expected_responses: &[&str] = &["I attempted to list", "failed"];
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
}
