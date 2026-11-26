use std::{fs, path::Path};

use ratatui::buffer::Buffer;
use shodan::settings::build_settings;

use super::super::{
    openai::create_openai_home,
    support::{
        RecordedLlama, read_history, receive_request, run_test_application, send_response_bytes,
    },
};

async fn run_edit_scenario(
    scenario: &str,
    prompt: &str,
    expected_screen: &str,
    expected_response: &str,
) -> (String, Buffer) {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.press_key(crossterm::event::KeyCode::Tab);
    application.send_message(prompt);
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["tools"]
            .as_array()
            .expect("tools are present")
            .len(),
        4
    );
    let recording_directory = Path::new("tests/features/fixtures/openai/tools").join(scenario);
    let response = fs::read_to_string(recording_directory.join("response-1.txt"))
        .expect("the recorded tool call exists");
    send_response_bytes(&request, &response).await;
    let request = receive_request(&mut requests).await;
    let input = request.body["input"]
        .as_array()
        .expect("the conversation input is an array");
    let call = input
        .iter()
        .find(|item| item["type"] == "function_call" && item["name"] == "edit")
        .expect("the model requested the edit tool");
    let result = input
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == call["call_id"])
        .expect("the model receives the tool result");
    let output = result["output"]
        .as_str()
        .expect("the tool result is text")
        .to_owned();
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains(expected_screen))
        .await;
    let response = fs::read_to_string(recording_directory.join("response-2.txt"))
        .expect("the recorded model response exists");
    send_response_bytes(&request, &response).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains(expected_response))
        .await;
    (output, screen)
}

fn prepare_file(scenario: &str, file: &str, content: Option<&str>) -> std::path::PathBuf {
    let directory = Path::new("target/edit-scenarios").join(scenario);
    fs::create_dir_all(&directory).expect("the scenario directory can be created");
    let path = directory.join(file);
    if let Some(content) = content {
        fs::write(&path, content).expect("the scenario file can be written");
    } else if path.exists() {
        fs::remove_file(&path).expect("the scenario file can be removed");
    }
    path
}

#[tokio::test]
async fn user_edits_file() {
    let original = "fn fibonacci(number: u32) -> u64 {\n    match number {\n        0 => 0,\n        1 => 1,\n        _ => fibonacci(number - 1) + fibonacci(number - 2),\n    }\n}\n";
    let updated = "fn fibonacci(number: u32) -> u64 {\n    let mut previous = 0;\n    let mut current = 1;\n    for _ in 0..number {\n        (previous, current) = (current, previous + current);\n    }\n    previous\n}\n";
    let path = prepare_file("user_edits_file", "fibonacci.rs", Some(original));
    let prompt = format!(
        "Use edit once on {} to replace the entire existing function:\n{original}with exactly:\n{updated}Report the result.",
        path.display()
    );
    let (output, screen) = run_edit_scenario(
        "user_edits_file",
        &prompt,
        "+ 2      let mut previous = 0;",
        "Replaced the function",
    )
    .await;
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        updated
    );
    assert!(output.contains("-    match number {"));
    assert!(output.contains("+    let mut previous = 0;"));
    assert!(read_history(&screen).contains("- 2      match number {"));
    let row = (0..screen.area.height)
        .find(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, *row)].symbol())
                .collect::<String>()
                .contains("+ 2      let mut previous = 0;")
        })
        .expect("the added line is visible");
    let line = (0..screen.area.width)
        .map(|column| screen[(column, row)].symbol())
        .collect::<String>();
    let keyword = u16::try_from(line.find("let mut").expect("the keyword is visible"))
        .expect("the keyword fits on screen");
    assert_ne!(screen[(keyword, row)].fg, screen[(keyword + 8, row)].fg);
}

#[tokio::test]
async fn user_edits_multiple_parts_of_file() {
    let path = prepare_file(
        "user_edits_multiple_parts_of_file",
        "greeting.txt",
        Some("Hello, world! Goodbye, world!"),
    );
    let prompt = format!(
        "Use the edit tool once on {} with two replacements: Hello to Welcome, and Goodbye to Farewell. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_edits_multiple_parts_of_file",
        &prompt,
        "+ 1  Welcome, world! Farewell, world!",
        "Successfully edited the file",
    )
    .await;
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        "Welcome, world! Farewell, world!"
    );
    assert!(output.contains("Welcome, world! Farewell, world!"));
}

#[tokio::test]
async fn user_edits_nonexistent_file() {
    let path = prepare_file("user_edits_nonexistent_file", "missing.txt", None);
    let prompt = format!(
        "Use the edit tool once on {} to replace Hello with Welcome. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_edits_nonexistent_file",
        &prompt,
        "Could not edit",
        "No such file or directory",
    )
    .await;
    assert!(output.contains("Could not edit"));
}

#[tokio::test]
async fn user_edits_text_that_cannot_be_found() {
    let path = prepare_file(
        "user_edits_text_that_cannot_be_found",
        "greeting.txt",
        Some("Hello, world!"),
    );
    let prompt = format!(
        "Use the edit tool once on {} to replace Goodbye with Farewell. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_edits_text_that_cannot_be_found",
        &prompt,
        "Could not find",
        "was not found",
    )
    .await;
    assert!(output.contains("Could not find"));
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        "Hello, world!"
    );
}

#[tokio::test]
async fn user_edits_text_that_occurs_more_than_once() {
    let path = prepare_file(
        "user_edits_text_that_occurs_more_than_once",
        "greeting.txt",
        Some("Hello, world! Goodbye, world!"),
    );
    let prompt = format!(
        "Use the edit tool once on {} to replace world with Shodan. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_edits_text_that_occurs_more_than_once",
        &prompt,
        "ambiguous",
        "occurs more than once",
    )
    .await;
    assert!(output.contains("ambiguous"));
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        "Hello, world! Goodbye, world!"
    );
}

#[tokio::test]
async fn user_requests_overlapping_changes() {
    let path = prepare_file(
        "user_requests_overlapping_changes",
        "greeting.txt",
        Some("Hello, world!"),
    );
    let prompt = format!(
        "Use the edit tool once on {} with two replacements: Hello, world to Welcome, and world to Shodan. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_requests_overlapping_changes",
        &prompt,
        "Replacement regions overlap",
        "Replacement regions overlap",
    )
    .await;
    assert!(output.contains("Replacement regions overlap"));
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        "Hello, world!"
    );
}

#[tokio::test]
async fn user_requests_change_alongside_invalid_change() {
    let path = prepare_file(
        "user_requests_change_alongside_invalid_change",
        "greeting.txt",
        Some("Hello, world!"),
    );
    let prompt = format!(
        "Use the edit tool once on {} with two replacements: Hello to Welcome, and Goodbye to Farewell. Report the tool result.",
        path.display()
    );
    let (output, _) = run_edit_scenario(
        "user_requests_change_alongside_invalid_change",
        &prompt,
        "Could not find",
        "was not found",
    )
    .await;
    assert!(output.contains("edits[1]"));
    assert_eq!(
        fs::read_to_string(path).expect("the file can be read"),
        "Hello, world!"
    );
}
