use std::fs;

use super::support::{prepare_scenario, run_tool_scenario};

#[tokio::test]
async fn user_finds_files() {
    let path = prepare_scenario("find-scenarios", "user_finds_files");
    fs::create_dir_all(path.join("src/nested")).expect("directory can be created");
    fs::write(path.join("src/main.rs"), "").expect("file can be written");
    fs::write(path.join("src/nested/lib.rs"), "").expect("file can be written");
    let (output, _) = run_tool_scenario("user_finds_files", "find", "nested/lib.rs").await;
    assert!(output.contains("main.rs"));
    assert!(output.contains("nested/lib.rs"));
}

#[tokio::test]
async fn user_finds_hidden_entries() {
    let path = prepare_scenario("find-scenarios", "user_finds_hidden_entries");
    fs::write(path.join(".gitignore"), ".ignored\n").expect("file can be written");
    fs::write(path.join(".settings"), "").expect("file can be written");
    fs::write(path.join(".ignored"), "").expect("file can be written");
    let (output, _) = run_tool_scenario("user_finds_hidden_entries", "find", ".settings").await;
    assert_eq!(output, ".gitignore\n.settings");
}

#[tokio::test]
async fn user_finds_directory() {
    let path = prepare_scenario("find-scenarios", "user_finds_directory");
    fs::create_dir_all(path.join("assets")).expect("directory can be created");
    let (output, _) = run_tool_scenario("user_finds_directory", "find", "assets/").await;
    assert_eq!(output, "assets/");
}

#[tokio::test]
async fn user_limits_results() {
    let path = prepare_scenario("find-scenarios", "user_limits_results");
    for file in ["a.txt", "b.txt", "c.txt"] {
        fs::write(path.join(file), "").expect("file can be written");
    }
    let (output, _) = run_tool_scenario("user_limits_results", "find", "results truncated").await;
    assert_eq!(output, "a.txt\nb.txt\n[results truncated]");
}

#[tokio::test]
async fn user_finds_no_matching_paths() {
    prepare_scenario("find-scenarios", "user_finds_no_matching_paths");
    let (output, _) =
        run_tool_scenario("user_finds_no_matching_paths", "find", "No paths found").await;
    assert_eq!(output, "No paths found");
}

#[tokio::test]
async fn user_searches_invalid_directory() {
    prepare_scenario("find-scenarios", "user_searches_invalid_directory");
    let (output, _) =
        run_tool_scenario("user_searches_invalid_directory", "find", "No such file").await;
    assert!(output.contains("No such file"));
}
