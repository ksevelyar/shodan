use shodan::settings::build_settings;

use super::{
    super::{
        openai::create_openai_home,
        support::{
            RecordedLlama, create_home, read_bottom_line, read_history, read_screen,
            receive_request, receive_request_or_error, run_test_application, send_response_bytes,
        },
    },
    support::{read_tool_history, send_scenario_response},
};

#[tokio::test]
async fn user_reads_file() {
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
    application.send_message("Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.");
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, "user_reads_file", 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "read");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).expect("the tool arguments are JSON");
    let expected_arguments =
        serde_json::json!({"path":"tests/features/fixtures/tools/user_reads_file/greeting.txt"});
    assert_eq!(arguments, expected_arguments);
    let calls = assistant["tool_calls"]
        .as_array()
        .expect("the model returned tool calls");
    let expected_tool_results: &[&str] = &["Hello, Shodan!"];
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
    let screen = application
        .wait_for_screen(|screen| {
            let history = read_tool_history(screen);
            history
                .lines()
                .any(|line| line.trim() == format!("read {tool_path}"))
                && expected_tool_results
                    .iter()
                    .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, "user_reads_file", 2).await;
    let expected_responses: &[&str] = &["The exact contents", "Hello, Shodan!"];
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
    assert!(read_history(&screen).contains("1  Hello, Shodan!"));
    let row = (0..screen.area.height)
        .find(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, *row)].symbol())
                .collect::<String>()
                .contains("1  Hello, Shodan!")
        })
        .expect("the file contents are visible");
    for column in 0..screen.area.width {
        assert_eq!(
            screen[(column, row)].bg,
            ratatui::style::Color::Rgb(84, 70, 125)
        );
        assert_eq!(
            screen[(column, row - 1)].bg,
            ratatui::style::Color::Rgb(84, 70, 125)
        );
    }
}

#[tokio::test]
async fn user_reads_part_of_file() {
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
    application.send_message("Read only line 2 of tests/features/fixtures/tools/user_reads_part_of_file/lines.txt using the read tool with offset 2 and limit 1. Show the exact line.");
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, "user_reads_part_of_file", 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "read");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).expect("the tool arguments are JSON");
    let expected_arguments = serde_json::json!({
        "path": "tests/features/fixtures/tools/user_reads_part_of_file/lines.txt",
        "offset": 2,
        "limit": 1
    });
    assert_eq!(arguments, expected_arguments);
    let calls = assistant["tool_calls"]
        .as_array()
        .expect("the model returned tool calls");
    let expected_tool_results: &[&str] = &["Second line"];
    for call in calls {
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool" && message["tool_call_id"] == call["id"])
            .expect("the conversation contains the result for the tool call");
        let tool_content = tool_result["content"].as_str().expect("the result is text");
        for expected in expected_tool_results {
            assert!(tool_content.contains(expected), "{tool_content}");
        }
        assert_eq!(tool_content, "Second line\n");
    }
    let tool_path = expected_arguments["path"]
        .as_str()
        .expect("the tool path is text");
    let screen = application
        .wait_for_screen(|screen| {
            let history = read_tool_history(screen);
            history
                .lines()
                .any(|line| line.trim_start().starts_with("read") && line.contains(tool_path))
                && expected_tool_results
                    .iter()
                    .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, "user_reads_part_of_file", 2).await;
    let expected_responses: &[&str] = &["Line 2 is:", "Second line"];
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
    assert!(read_history(&screen).contains("2  Second line"));
}

#[tokio::test]
async fn user_scrolls_through_file_read_and_aborts_response() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("what do you think is biggest tech debt? write plan with all details");
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("../../../benches/fixtures/technical_debt/response-3.txt"),
    )
    .await;
    let request = receive_request(&mut requests).await;
    let response = include_str!("../../../benches/fixtures/technical_debt/response-6.txt");
    let streamed_events = response
        .split("\n\n")
        .take(16)
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n\n";
    send_response_bytes(&request, &streamed_events).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Biggest technical debt"))
        .await;
    let screen = application
        .wait_for_screen(|screen| {
            read_history(screen).contains("chats::apply_event(application, event)")
        })
        .await;
    let history = read_history(&screen);
    application.press_key(crossterm::event::KeyCode::PageUp);
    application.press_key(crossterm::event::KeyCode::PageUp);
    application
        .wait_for_screen(|screen| {
            let current_history = read_history(screen);
            current_history != history && current_history.contains("EventSource")
        })
        .await;
    application.press_key(crossterm::event::KeyCode::Esc);
    application.press_key(crossterm::event::KeyCode::PageDown);
    application.press_key(crossterm::event::KeyCode::PageDown);
    application
        .wait_for_screen(|screen| read_history(screen).contains("Request was aborted"))
        .await;
}

#[tokio::test]
async fn user_reads_source_file() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Read tests/features/fixtures/tools/user_reads_source_file/fibonacci.rs using the read tool and show its exact contents.");
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("../fixtures/openai/tools/user_reads_source_file/response-1.txt"),
    )
    .await;
    let request = receive_request(&mut requests).await;
    let output = request.body["input"]
        .as_array()
        .expect("the conversation input is an array")
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .expect("the model receives the file contents");
    assert_eq!(
        output["output"],
        include_str!("../fixtures/tools/user_reads_source_file/fibonacci.rs")
    );
    let screen = application
        .wait_for_screen(|screen| {
            read_history(screen).contains("1  fn fibonacci(number: u32) -> u64 {")
        })
        .await;
    let row = (0..screen.area.height)
        .find(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, *row)].symbol())
                .collect::<String>()
                .contains("1  fn fibonacci(number: u32) -> u64 {")
        })
        .expect("the numbered source line is visible");
    let line = (0..screen.area.width)
        .map(|column| screen[(column, row)].symbol())
        .collect::<String>();
    let keyword = u16::try_from(line.find("fn fibonacci").expect("the keyword is visible"))
        .expect("the keyword fits on screen");
    assert_ne!(screen[(keyword, row)].fg, screen[(keyword + 3, row)].fg);
    send_response_bytes(
        &request,
        include_str!("../fixtures/openai/tools/user_reads_source_file/response-2.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("fn fibonacci(number: u32) -> u64"))
        .await;
}

#[tokio::test]
async fn user_reads_nonexistent_file() {
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
    application.send_message("Read missing.txt using the read tool and report the result.");
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, "user_reads_nonexistent_file", 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "read");
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).expect("the tool arguments are JSON");
    let expected_arguments = serde_json::json!({"path":"missing.txt"});
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
                .any(|line| line.trim_start().starts_with("read") && line.contains(tool_path))
                && expected_tool_results
                    .iter()
                    .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, "user_reads_nonexistent_file", 2).await;
    let expected_responses: &[&str] = &["I attempted to read", "failed"];
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
}
