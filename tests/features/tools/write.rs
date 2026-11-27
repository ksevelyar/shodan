use std::fs;

use super::support::{prepare_scenario, run_tool_scenario};

#[tokio::test]
async fn user_creates_file() {
    let path = prepare_scenario("write-scenarios", "user_creates_file");
    let (output, _) = run_tool_scenario("user_creates_file", "write", "Use write once to write Hello, Shodan! to target/write-scenarios/user_creates_file/notes/greeting.txt. Report the result.", "Hello, Shodan!").await;
    assert_eq!(
        fs::read_to_string(path.join("notes/greeting.txt")).expect("file can be read"),
        "Hello, Shodan!"
    );
    assert!(output.contains("Successfully wrote"));
}

#[tokio::test]
async fn user_replaces_file() {
    let path = prepare_scenario("write-scenarios", "user_replaces_file");
    fs::write(path.join("greeting.txt"), "Hello, world!").expect("file can be written");
    let (output, _) = run_tool_scenario("user_replaces_file", "write", "Use write once to write Hello, Shodan! to target/write-scenarios/user_replaces_file/greeting.txt. Report the result.", "Hello, Shodan!").await;
    assert_eq!(
        fs::read_to_string(path.join("greeting.txt")).expect("file can be read"),
        "Hello, Shodan!"
    );
    assert!(output.contains("Successfully wrote"));
}

#[tokio::test]
async fn user_writes_to_invalid_path() {
    let path = prepare_scenario("write-scenarios", "user_writes_to_invalid_path");
    fs::create_dir_all(path.join("greeting.txt")).expect("directory can be created");
    let (output, _) =
        run_tool_scenario("user_writes_to_invalid_path", "write", "Use write once to write Hello, Shodan! to target/write-scenarios/user_writes_to_invalid_path/greeting.txt. Do not retry if it fails. Report the result.", "Is a directory").await;
    assert!(output.contains("Is a directory"));
}

#[tokio::test]
async fn user_writes_source_file() {
    let path = prepare_scenario("write-scenarios", "user_writes_source_file");
    let (output, screen) =
        run_tool_scenario("user_writes_source_file", "write", "Use write once to write fn main() {} to target/write-scenarios/user_writes_source_file/main.rs. Report the result.", "1  fn main()").await;
    assert_eq!(
        fs::read_to_string(path.join("main.rs")).expect("file can be read"),
        "fn main() {}"
    );
    assert!(output.contains("Successfully wrote"));
    let row = (0..screen.area.height)
        .find(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, *row)].symbol())
                .collect::<String>()
                .contains("1  fn main()")
        })
        .expect("written source is visible");
    let line = (0..screen.area.width)
        .map(|column| screen[(column, row)].symbol())
        .collect::<String>();
    let keyword = u16::try_from(line.find("fn main").expect("keyword is visible"))
        .expect("keyword fits on screen");
    assert_ne!(screen[(keyword, row)].fg, screen[(keyword + 3, row)].fg);
    for column in 0..screen.area.width {
        assert_eq!(
            screen[(column, row)].bg,
            ratatui::style::Color::Rgb(84, 70, 125)
        );
    }
}
