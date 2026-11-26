use crossterm::event::KeyCode;
use shodan::settings::build_settings;

use super::super::support::{
    RecordedLlama, create_home, read_bottom_line, read_history, read_screen,
    receive_request_or_error, run_test_application, send_response_bytes,
};

fn inkling_server() -> RecordedLlama {
    RecordedLlama::new()
}

fn tool_scenario_stream(scenario: &str, response_number: usize) -> String {
    format!("tests/features/fixtures/llama/tools/{scenario}/response-{response_number}.txt")
}

async fn is_process_running(command_pattern: &str) -> bool {
    tokio::process::Command::new("pgrep")
        .arg("-f")
        .arg(command_pattern)
        .output()
        .await
        .expect("the process list can be queried")
        .status
        .success()
}

async fn wait_for_tool_process_start(command_pattern: &str) {
    let start_timeout = std::time::Duration::from_secs(5);
    let deadline = tokio::time::Instant::now() + start_timeout;
    while tokio::time::Instant::now() < deadline {
        if is_process_running(command_pattern).await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the command never started running: {command_pattern}");
}

async fn wait_for_tool_process_death(command_pattern: &str) {
    let death_timeout = std::time::Duration::from_secs(5);
    let deadline = tokio::time::Instant::now() + death_timeout;
    while tokio::time::Instant::now() < deadline {
        if !is_process_running(command_pattern).await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the aborted command keeps running: {command_pattern}");
}

#[tokio::test]
async fn user_runs_command() {
    let (address, mut requests) = inkling_server()
        .mock_stream(
            "Inkling-2-Mini-UD-IQ3_XXS",
            &tool_scenario_stream("user_runs_command", 1),
        )
        .start()
        .await;
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
    application
        .send_message("Run the command echo hello using the bash tool. Show the exact output.");
    receive_request_or_error(&mut requests, &mut application).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("bash"))
        .await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "bash");
    let tool_result = messages
        .iter()
        .find(|message| {
            message["role"] == "tool" && message["tool_call_id"] == assistant["tool_calls"][0]["id"]
        })
        .expect("the conversation contains the result for the tool call");
    let tool_content = tool_result["content"].as_str().expect("the result is text");
    assert!(tool_content.contains("hello"), "{tool_content}");
    send_response_bytes(
        &second_request,
        include_str!("../fixtures/llama/tools/user_runs_command/response-2.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| {
            let last_line = (0..screen.area.height.saturating_sub(4))
                .map(|row| {
                    (0..screen.area.width)
                        .map(|column| screen[(column, row)].symbol())
                        .collect::<String>()
                })
                .rfind(|line| !line.trim().is_empty());
            read_bottom_line(screen).contains('⏱')
                && last_line.is_some_and(|line| line.trim() == "hello")
        })
        .await;
}

#[tokio::test]
async fn user_is_informed_when_command_fails() {
    let (address, mut requests) = inkling_server()
        .mock_stream(
            "Inkling-2-Mini-UD-IQ3_XXS",
            &tool_scenario_stream("user_is_informed_when_command_fails", 1),
        )
        .mock_stream(
            "Inkling-2-Mini-UD-IQ3_XXS",
            &tool_scenario_stream("user_is_informed_when_command_fails", 2),
        )
        .start()
        .await;
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
    application
        .send_message("Run the command exit 3 using the bash tool and report what happened.");
    receive_request_or_error(&mut requests, &mut application).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("bash"))
        .await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let tool_result = messages
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("the conversation contains the result for the tool call");
    let tool_content = tool_result["content"].as_str().expect("the result is text");
    assert!(
        tool_content.contains("Command failed with exit code 3"),
        "{tool_content}"
    );
    application
        .wait_for_screen(|screen| read_history(screen).contains("Command failed with exit code 3"))
        .await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("exit code 3"))
        .await;
}

#[tokio::test]
async fn user_aborts_running_command() {
    let (address, mut requests) = inkling_server()
        .mock_stream(
            "Inkling-2-Mini-UD-IQ3_XXS",
            &tool_scenario_stream("user_aborts_running_command", 1),
        )
        .mock_stream(
            "Inkling-2-Mini-UD-IQ3_XXS",
            &tool_scenario_stream("user_aborts_running_command", 2),
        )
        .start()
        .await;
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
    application.send_message("Run the command sleep 30 using the bash tool.");
    receive_request_or_error(&mut requests, &mut application).await;
    wait_for_tool_process_start("sleep 30").await;
    application.press_key(KeyCode::Esc);
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Request was aborted"))
        .await;
    wait_for_tool_process_death("sleep 30").await;
    application.send_message("Ignore the previous request. Say hello.");
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    assert!(messages.iter().any(|message| message["role"] == "user"
        && message["content"] == "Run the command sleep 30 using the bash tool."));
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello"))
        .await;
}
