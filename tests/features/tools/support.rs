use std::{fs, path::Path};

use super::super::{
    openai::create_openai_home,
    support::{
        CapturedRequest, RecordedProvider, read_history, receive_request, run_test_application,
        send_response_bytes,
    },
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
pub(crate) async fn run_tool_scenario(
    scenario: &str,
    tool: &str,
    expected_screen: &str,
) -> (String, ratatui::buffer::Buffer) {
    let (address, mut requests) = RecordedProvider::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings = shodan::settings::build_settings(home.path().to_owned(), Vec::new())
        .expect("settings can be built");
    let mut application = run_test_application(settings);
    if tool == "write" {
        application.press_key(crossterm::event::KeyCode::Tab);
    }
    application.send_message(&format!("Use the {tool} tool for {scenario}."));
    let request = receive_request(&mut requests).await;
    let directory = Path::new("tests/features/fixtures/openai/tools").join(scenario);
    let response =
        fs::read_to_string(directory.join("response-1.txt")).expect("tool call was recorded");
    send_response_bytes(&request, &response).await;
    let request = receive_request(&mut requests).await;
    let input = request.body["input"].as_array().expect("input is an array");
    let call = input
        .iter()
        .find(|item| item["type"] == "function_call" && item["name"] == tool)
        .expect("tool was called");
    let output = input
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == call["call_id"])
        .expect("model receives the result")["output"]
        .as_str()
        .expect("result is text")
        .to_owned();
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains(expected_screen))
        .await;
    let response =
        fs::read_to_string(directory.join("response-2.txt")).expect("model response was recorded");
    send_response_bytes(&request, &response).await;
    (output, screen)
}

pub(crate) fn prepare_scenario(directory: &str, scenario: &str) -> std::path::PathBuf {
    let path = Path::new("target").join(directory).join(scenario);
    if path.exists() {
        fs::remove_dir_all(&path).expect("previous scenario can be removed");
    }
    fs::create_dir_all(&path).expect("scenario directory can be created");
    path
}

pub(crate) fn read_tool_history(screen: &ratatui::buffer::Buffer) -> String {
    (0..screen.area.height.saturating_sub(4))
        .filter(|&row| screen[(0, row)].bg != ratatui::style::Color::Rgb(55, 55, 55))
        .map(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
