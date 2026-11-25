use std::{fs, path::Path};

use serde_json::Value;
use shodan::settings::build_settings;

use super::super::support::{
    CapturedRequest, RecordedLlama, create_home, read_bottom_line, read_screen,
    receive_request_or_error, run_test_application, send_response_bytes,
};

pub(crate) async fn send_scenario_response(
    request: &CapturedRequest,
    scenario: &str,
    response_number: usize,
) {
    let directory = Path::new("tests/features/fixtures/llama/tools").join(scenario);
    let path = directory.join(format!("response-{response_number}.txt"));
    let response =
        fs::read_to_string(&path).expect("the scenario has a recorded provider response");
    send_response_bytes(request, &response).await;
}

pub async fn run_scenario(
    scenario: &str,
    prompt: &str,
    expected_tool: &str,
    expected_arguments: Value,
    expected_tool_results: &[&str],
    expected_exact_tool_result: Option<&str>,
    expected_responses: &[&str],
) {
    let (address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}, {"name": "Qwen3.8-27B-UD-IQ4_XS", "thinking_level": "low"}]}}}),
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
    application.send_message(prompt);
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_scenario_response(&first_request, scenario, 1).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("the conversation is an array");
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .expect("the model requested a tool");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        expected_tool
    );
    let arguments = assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("the tool arguments are text");
    let arguments: Value = serde_json::from_str(arguments).expect("the tool arguments are JSON");
    assert_eq!(arguments, expected_arguments);
    let calls = assistant["tool_calls"]
        .as_array()
        .expect("the model returned tool calls");
    for call in calls {
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool" && message["tool_call_id"] == call["id"])
            .expect("the conversation contains the result for the tool call");
        let tool_content = tool_result["content"].as_str().expect("the result is text");
        for expected in expected_tool_results {
            assert!(tool_content.contains(expected), "{tool_content}");
        }
        if let Some(expected) = expected_exact_tool_result {
            assert_eq!(tool_content, expected);
        }
    }
    let tool_path = expected_arguments["path"]
        .as_str()
        .expect("the tool path is text");
    application
        .wait_for_screen(|screen| {
            let history = (0..screen.area.height.saturating_sub(4))
                .filter(|&row| screen[(0, row)].bg != ratatui::style::Color::Rgb(55, 55, 55))
                .map(|row| {
                    (0..screen.area.width)
                        .map(|column| screen[(column, row)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            history.lines().any(|line| {
                line.trim_start().starts_with(expected_tool) && line.contains(tool_path)
            }) && expected_tool_results
                .iter()
                .all(|expected| history.contains(expected))
        })
        .await;
    send_scenario_response(&second_request, scenario, 2).await;
    application
        .wait_for_screen(|screen| {
            let screen = read_screen(screen);
            expected_responses
                .iter()
                .all(|expected| screen.contains(expected))
        })
        .await;
}
