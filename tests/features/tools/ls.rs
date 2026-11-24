use super::support::run_scenario;

#[tokio::test]
async fn user_lists_directory() {
    run_scenario(
        "user_lists_directory",
        "List all entries in tests/features/fixtures/tools/user_lists_directory, including hidden entries, using the ls tool. Show the exact listing.",
        "ls",
        serde_json::json!({"path":"tests/features/fixtures/tools/user_lists_directory"}),
        &[".settings", "notes.txt", "src/"],
        None,
        &["Entries shown", ".settings", "notes.txt", "src/"],
    )
    .await;
}

#[tokio::test]
async fn user_lists_nonexistent_directory() {
    run_scenario(
        "user_lists_nonexistent_directory",
        "List the directory missing-directory using the ls tool and report the result.",
        "ls",
        serde_json::json!({"path":"missing-directory"}),
        &["Error:"],
        None,
        &["I attempted to list", "failed"],
    )
    .await;
}
