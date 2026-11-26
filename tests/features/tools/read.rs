use shodan::settings::build_settings;

use super::{
    super::{
        openai::create_openai_home,
        support::{
            RecordedLlama, read_history, receive_request, run_test_application, send_response_bytes,
        },
    },
    support::run_scenario,
};

#[tokio::test]
async fn user_reads_file() {
    let screen = run_scenario(
        "user_reads_file",
        "Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.",
        "read",
        serde_json::json!({"path":"tests/features/fixtures/tools/user_reads_file/greeting.txt"}),
        &["Hello, Shodan!"],
        None,
        &["The exact contents", "Hello, Shodan!"],
    )
    .await;
    assert!(read_history(&screen).contains("1  Hello, Shodan!"));
}

#[tokio::test]
async fn user_reads_part_of_file() {
    let screen = run_scenario(
        "user_reads_part_of_file",
        "Read only line 2 of tests/features/fixtures/tools/user_reads_part_of_file/lines.txt using the read tool with offset 2 and limit 1. Show the exact line.",
        "read",
        serde_json::json!({
            "path": "tests/features/fixtures/tools/user_reads_part_of_file/lines.txt",
            "offset": 2,
            "limit": 1
        }),
        &["Second line"],
        Some("Second line\n"),
        &["Line 2 is:", "Second line"],
    )
    .await;
    assert!(read_history(&screen).contains("2  Second line"));
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
    run_scenario(
        "user_reads_nonexistent_file",
        "Read missing.txt using the read tool and report the result.",
        "read",
        serde_json::json!({"path":"missing.txt"}),
        &["Error:"],
        None,
        &["I attempted to read", "failed"],
    )
    .await;
}
