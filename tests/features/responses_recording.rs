use std::{
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use super::support::{RecordedLlama, create_home, receive_request, send_response_bytes};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn list_recordings(directory: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .map(|entry| {
            entry
                .expect("recording directory entries are readable")
                .path()
        })
        .collect()
}

async fn wait_for_terminal_output(
    mut terminal_output: Box<dyn Read + Send>,
    expected: &'static str,
) {
    let timeout = Duration::from_secs(5);
    tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(move || {
            let mut output = Vec::new();
            let mut buffer = [0; 1024];
            while !output
                .windows(expected.len())
                .any(|window| window == expected.as_bytes())
            {
                let count = terminal_output
                    .read(&mut buffer)
                    .expect("terminal output can be read");
                assert!(count > 0, "terminal remains open");
                output.extend_from_slice(&buffer[..count]);
            }
        }),
    )
    .await
    .expect("terminal becomes ready")
    .expect("terminal output task completes");
}

#[tokio::test]
async fn user_records_openai_response() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_home(
        serde_json::json!({"default_provider": "openai", "providers": {"openai": {
            "address": address, "default_model": "gpt-6.1-terra", "models": [{"name": "gpt-6.1-terra"}]
        }}}),
    );
    std::fs::write(home.path().join(".shodan/openai.json"), serde_json::json!({
        "type":"oauth", "access":"recorded-access", "refresh":"recorded-refresh",
        "expires":4_102_444_800_000_u64, "clientId":"recorded-client", "scopes":["chatgpt.tokens.use.direct"]
    }).to_string()).expect("the credentials can be written");
    let application = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .current_dir(home.path())
        .env("HOME", home.path())
        .args(["--provider", "openai", "--record", "--prompt", "Greet me"])
        .output();
    let request = async {
        let request = receive_request(&mut requests).await;
        send_response_bytes(&request, include_str!("fixtures/openai/chat.txt")).await;
    };
    let (output, ()) = tokio::join!(application, request);
    assert!(output.expect("Shodan exits").status.success());
    let recordings = list_recordings(&home.path().join("recordings/openai/v1-responses"));
    assert_eq!(recordings.len(), 1);
    let response = std::fs::read_to_string(&recordings[0]).expect("the recording can be read");
    assert_eq!(response, include_str!("fixtures/openai/chat.txt"));
    uuid::Uuid::parse_str(
        recordings[0]
            .file_stem()
            .expect("the recording has a name")
            .to_str()
            .expect("the name is text"),
    )
    .expect("the recording name is a UUID");
}

#[tokio::test]
async fn user_records_openai_credential_refresh() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_home(
        serde_json::json!({"default_provider": "openai", "providers": {"openai": {
            "address": address, "authentication_address": address,
            "default_model": "gpt-6.1-terra", "models": [{"name": "gpt-6.1-terra"}]
        }}}),
    );
    std::fs::write(
        home.path().join(".shodan/openai.json"),
        serde_json::json!({
            "type": "oauth", "access": "expired-access", "refresh": "expired-refresh",
            "expires": 0, "clientId": "recorded-client", "scopes": ["chatgpt.tokens.use.direct"]
        })
        .to_string(),
    )
    .expect("the credentials can be written");
    let application = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .current_dir(home.path())
        .env("HOME", home.path())
        .args(["--provider", "openai", "--record", "--prompt", "Greet me"])
        .output();
    let request = async {
        let request = receive_request(&mut requests).await;
        send_response_bytes(&request, include_str!("fixtures/openai/chat.txt")).await;
    };
    let (output, ()) = tokio::join!(application, request);
    assert!(output.expect("Shodan exits").status.success());
    let recordings = list_recordings(&home.path().join("recordings/openai/auth-token"));
    assert_eq!(recordings.len(), 1);
    let recording: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&recordings[0]).expect("the recording can be read"))
            .expect("the recording is valid JSON");
    let response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/openai/refresh.json"))
            .expect("the recorded provider response is valid JSON");
    for field in ["scope", "token_type", "expires_in"] {
        assert_eq!(recording[field], response[field]);
    }
    for field in ["access_token", "refresh_token", "id_token"] {
        assert_ne!(recording[field], response[field]);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn user_records_provider_responses() {
    let directory =
        std::env::temp_dir().join(format!("shodan-recording-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("the working directory can be created");
    let completions_directory = directory.join("recordings/llama/v1-chat-completions");
    let (address, mut requests) = RecordedLlama::new().start().await;
    let configuration_directory = directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let configuration = serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "LFM3-Omni-2B-Q4_K_M", "models": [{"name": "LFM3-Omni-2B-Q4_K_M"}]}}});
    std::fs::write(
        configuration_directory.join("settings.json"),
        configuration.to_string(),
    )
    .expect("the configuration can be written");
    let terminal = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("the terminal is available");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_shodan"));
    command.cwd(&directory);
    command.env("HOME", &directory);
    command.env("TERM", "xterm-256color");
    command.args([
        "--provider",
        "llama",
        "--model",
        "LFM3-Omni-2B-Q4_K_M",
        "--record",
    ]);
    let mut application = terminal
        .slave
        .spawn_command(command)
        .expect("Shodan starts");
    let mut terminal_input = terminal
        .master
        .take_writer()
        .expect("terminal input is available");
    let timeout = Duration::from_secs(5);
    let terminal_output = terminal
        .master
        .try_clone_reader()
        .expect("terminal output is available");
    wait_for_terminal_output(terminal_output, "LFM3-Omni-2B-Q4_K_M").await;
    terminal_input
        .write_all(b"hello\r")
        .expect("the user sends a message");
    terminal_input.flush().expect("terminal input is sent");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "LFM3-Omni-2B-Q4_K_M");
    let response = include_str!("fixtures/llama/chat/lfm3-asteroids-plan.txt");
    send_response_bytes(&request, response).await;
    let completions = tokio::time::timeout(timeout, async {
        loop {
            let completions = list_recordings(&completions_directory);
            if completions
                .iter()
                .any(|path| std::fs::read(path).is_ok_and(|bytes| bytes == response.as_bytes()))
            {
                break completions;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the streamed response is recorded");
    for path in &completions {
        assert_eq!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("txt")
        );
        let filename = path
            .file_stem()
            .expect("recording has a filename")
            .to_str()
            .expect("recording filename is text");
        uuid::Uuid::parse_str(filename).expect("recording filename is a UUID");
    }
    application.kill().expect("Shodan can be stopped");
    application.wait().expect("Shodan exits");
    std::fs::remove_dir_all(directory).expect("the working directory can be removed");
}
