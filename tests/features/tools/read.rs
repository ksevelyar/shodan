use super::support::run_scenario;

#[tokio::test]
async fn user_reads_file() {
    run_scenario(
        "user_reads_file",
        "Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.",
        "read",
        serde_json::json!({"path":"tests/features/fixtures/tools/user_reads_file/greeting.txt"}),
        &["Hello, Shodan!"],
        None,
        &["The exact contents", "Hello, Shodan!"],
    )
    .await;
}

#[tokio::test]
async fn user_reads_part_of_file() {
    run_scenario(
        "user_reads_part_of_file",
        "Read only line 2 of tests/features/fixtures/tools/user_reads_part_of_file/lines.txt using the read tool with offset 2 and limit 1. Show the exact line.",
        "read",
        serde_json::json!({"path":"tests/features/fixtures/tools/user_reads_part_of_file/lines.txt","offset":2,"limit":1}),
        &["Second line"],
        Some("Second line\n"),
        &["Line 2 is:", "Second line"],
    )
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
