use std::{fs, path::Path};

use serde_json::Value;

use super::super::support::{
    CapturedRequest, RecordedLlama, read_bottom_line, read_screen, receive_request_or_error,
    run_test_application, send_response_bytes,
};

pub(crate) async fn send_scenario_response(
    request: &CapturedRequest,
    scenario: &str,
    response_number: usize,
) {
    let directory = Path::new("tests/features/fixtures/llama-cpp/tools").join(scenario);
    let path = directory.join(format!("response-{response_number}.txt"));
    let response = if std::env::var_os("SHODAN_RECORD_LLAMA").is_some() {
        let address = std::env::var("SHODAN_LLAMA_ADDRESS")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
        let client = reqwest::Client::new();
        let models: Value = client
            .get(format!("{address}/v1/models"))
            .send()
            .await
            .expect("the running server lists models")
            .json()
            .await
            .expect("the running server returns model data");
        let model = models["data"]
            .as_array()
            .expect("the model list is an array")
            .iter()
            .find(|model| model["status"]["value"] == "loaded")
            .and_then(|model| model["id"].as_str())
            .expect("the running server has a loaded model");
        assert_eq!(
            request.body["model"], model,
            "the recorded model must be loaded"
        );
        let response = client
            .post(format!("{address}/v1/chat/completions"))
            .json(&request.body)
            .send()
            .await
            .expect("the running server accepts the request")
            .error_for_status()
            .expect("the running server accepts the conversation")
            .text()
            .await
            .expect("the running server streams a response");
        fs::create_dir_all(&directory).expect("the recording directory can be created");
        fs::write(&path, &response).expect("the response can be recorded");
        response
    } else {
        fs::read_to_string(&path).expect("the scenario has a recorded provider response")
    };
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
    let (address, mut requests) = RecordedLlama::new()
        .mock_available_models("tests/features/fixtures/llama-cpp/models/inkling-loaded.json")
        .mock_model_load(
            "Qwen3.8-27B-UD-IQ4_XS",
            "tests/features/fixtures/llama-cpp/models/qwen-loading.json",
            "tests/features/fixtures/llama-cpp/models/qwen-loaded.json",
        )
        .start()
        .await;
    let mut application = run_test_application(address, 100, 30);
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
