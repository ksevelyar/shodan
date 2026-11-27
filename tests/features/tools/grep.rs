use std::fs;

use super::support::{prepare_scenario, run_tool_scenario};

#[tokio::test]
async fn user_searches_file_contents() {
    let path = prepare_scenario("search-scenarios", "user_searches_file_contents");
    fs::create_dir_all(path.join("src")).expect("directory can be created");
    fs::write(path.join("src/main.rs"), "fn main() {}   \n").expect("file can be written");
    let (output, _) =
        run_tool_scenario("user_searches_file_contents", "grep", "Use grep once to search target/search-scenarios/user_searches_file_contents/src for fn main. Report the result.", "main.rs:1:fn main()").await;
    assert_eq!(output, "main.rs:1:fn main() {}   ");
}

#[tokio::test]
async fn user_searches_with_file_filter() {
    let path = prepare_scenario("search-scenarios", "user_searches_with_file_filter");
    fs::create_dir_all(path.join("src")).expect("directory can be created");
    fs::write(path.join("src/main.rs"), "hello\n").expect("file can be written");
    fs::write(path.join("src/notes.txt"), "hello\n").expect("file can be written");
    let (output, _) =
        run_tool_scenario("user_searches_with_file_filter", "grep", "Use grep once to search target/search-scenarios/user_searches_with_file_filter/src for hello with glob *.rs. Report the result.", "main.rs:1:hello").await;
    assert_eq!(output, "main.rs:1:hello");
}

#[tokio::test]
async fn user_searches_for_literal_text_without_regard_to_case() {
    let path = prepare_scenario(
        "search-scenarios",
        "user_searches_for_literal_text_without_regard_to_case",
    );
    fs::write(path.join("notes.txt"), "Version 1.0\n").expect("file can be written");
    let (output, _) = run_tool_scenario(
        "user_searches_for_literal_text_without_regard_to_case",
        "grep",
        "Use grep once to search target/search-scenarios/user_searches_for_literal_text_without_regard_to_case/notes.txt for literal text version 1.0 ignoring case. Report the result.",
        "notes.txt:1:Version 1.0",
    )
    .await;
    assert!(output.contains("notes.txt:1:Version 1.0"));
}

#[tokio::test]
async fn user_searches_directory_with_ignored_files() {
    let path = prepare_scenario(
        "search-scenarios",
        "user_searches_directory_with_ignored_files",
    );
    fs::write(path.join(".gitignore"), "ignored.txt\n").expect("file can be written");
    fs::write(path.join(".notes"), "hello\n").expect("file can be written");
    fs::write(path.join("ignored.txt"), "hello\n").expect("file can be written");
    let (output, _) = run_tool_scenario(
        "user_searches_directory_with_ignored_files",
        "grep",
        "Use grep once to search target/search-scenarios/user_searches_directory_with_ignored_files for hello. Report the result.",
        ".notes:1:hello",
    )
    .await;
    assert_eq!(output, ".notes:1:hello");
}

#[tokio::test]
async fn user_sees_truncated_search_results() {
    let path = prepare_scenario("search-scenarios", "user_sees_truncated_search_results");
    fs::write(path.join("notes.txt"), "match\n".repeat(101)).expect("file can be written");
    let (output, _) = run_tool_scenario(
        "user_sees_truncated_search_results",
        "grep",
        "Use grep once to search target/search-scenarios/user_sees_truncated_search_results for match. Report the result.",
        "results truncated",
    )
    .await;
    assert_eq!(
        output
            .lines()
            .filter(|line| line.contains(":match"))
            .count(),
        100
    );
    assert!(output.contains("results truncated"));
}

#[tokio::test]
async fn user_searches_with_no_matches() {
    let path = prepare_scenario("search-scenarios", "user_searches_with_no_matches");
    fs::write(path.join("notes.txt"), "hello\n").expect("file can be written");
    let (output, _) =
        run_tool_scenario("user_searches_with_no_matches", "grep", "Use grep once to search target/search-scenarios/user_searches_with_no_matches/notes.txt for missing. Report the result.", "No matches found").await;
    assert_eq!(output, "No matches found");
}

#[tokio::test]
async fn user_searches_invalid_path() {
    prepare_scenario("search-scenarios", "user_searches_invalid_path");
    let (output, _) = run_tool_scenario("user_searches_invalid_path", "grep", "Use grep once to search target/search-scenarios/user_searches_invalid_path/missing for hello. Do not retry if it fails. Report the result.", "No such file").await;
    assert!(output.contains("No such file"));
}
