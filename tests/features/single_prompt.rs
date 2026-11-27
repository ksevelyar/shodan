use std::{process::Stdio, time::Duration};

use serde_json::json;
use tokio::{
    process::{Child, Command},
    time::timeout,
};

use super::support::{RecordedProvider, create_home, receive_request, send_response_bytes};

fn start_shodan(home_directory: &std::path::Path, arguments: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home_directory)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts")
}

fn start_recorded_prompt(home_directory: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home_directory)
        .current_dir(home_directory)
        .args(["--record", "--prompt", "Say hello."])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts")
}

async fn collect_output(process: Child) -> std::process::Output {
    timeout(Duration::from_secs(10), process.wait_with_output())
        .await
        .expect("Shodan exits")
        .expect("Shodan output is available")
}

#[tokio::test]
async fn user_submits_prompt() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Qwen3.8-27B-UD-IQ4_XS",
                "models": [
                    {
                        "name": "Qwen3.8-27B-UD-IQ4_XS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let prompt = "Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.";
    let process = start_shodan(home.path(), &["--prompt", prompt]);
    let first_request = receive_request(&mut requests).await;
    send_response_bytes(
        &first_request,
        include_str!("fixtures/llama/tools/user_reads_file/response-1.txt"),
    )
    .await;
    let second_request = receive_request(&mut requests).await;
    let messages = second_request.body["messages"]
        .as_array()
        .expect("messages are available");
    assert!(messages.iter().any(|message| {
        message["role"] == "tool"
            && message["content"]
                .as_str()
                .is_some_and(|content| content.contains("Hello, Shodan!"))
    }));
    send_response_bytes(
        &second_request,
        include_str!("fixtures/llama/tools/user_reads_file/response-2.txt"),
    )
    .await;
    let output = collect_output(process).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "The exact contents of `tests/features/fixtures/tools/user_reads_file/greeting.txt` are:\n\n```\nHello, Shodan!\n```\n"
    );
}

#[tokio::test]
async fn user_selects_provider_and_model() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(json!({
        "default_provider": "openai",
        "providers": {
            "openai": {
                "default_model": "openai-model",
                "models": [
                    {
                        "name": "openai-model",
                        "thinking_level": "high"
                    }
                ]
            },
            "llama": {
                "address": address,
                "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
                "models": [
                    {
                        "name": "Inkling-2-Mini-UD-IQ3_XXS",
                        "thinking_level": "low"
                    },
                    {
                        "name": "Qwen3.8-27B-UD-IQ4_XS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let process = start_shodan(
        home.path(),
        &[
            "--provider",
            "llama",
            "--model",
            "Qwen3.8-27B-UD-IQ4_XS",
            "--prompt",
            "Say hello.",
        ],
    );
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "Qwen3.8-27B-UD-IQ4_XS");
    assert!(
        request.body["messages"]
            .as_array()
            .expect("messages are available")
            .iter()
            .any(|message| message["role"] == "user" && message["content"] == "Say hello.")
    );
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/qwen-hello-low.txt"),
    )
    .await;
    let output = collect_output(process).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Hello! How can I help you today?\n"
    );
}

#[tokio::test]
async fn user_sees_prompt_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a local port is available");
    let address = listener.local_addr().expect("the port has an address");
    let server = tokio::spawn(async move {
        let (connection, _) = listener
            .accept()
            .await
            .expect("the provider receives a connection");
        drop(connection);
    });
    let home = create_home(json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": format!("http://{address}"),
                "default_model": "model",
                "models": [
                    {
                        "name": "model",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let output = collect_output(start_shodan(home.path(), &["--prompt", "Say hello."])).await;
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("error sending request"), "{error}");
    server.await.expect("the provider closes the connection");
}

#[tokio::test]
async fn user_combines_prompt_with_login() {
    let home = create_home(json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": "http://127.0.0.1:8080",
                "default_model": "model",
                "models": [
                    {
                        "name": "model",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let output = collect_output(start_shodan(
        home.path(),
        &["login", "openai", "--prompt", "hello"],
    ))
    .await;
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("login") && error.contains("--prompt"),
        "{error}"
    );
}

#[tokio::test]
async fn user_records_prompt() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Qwen3.8-27B-UD-IQ4_XS",
                "models": [
                    {
                        "name": "Qwen3.8-27B-UD-IQ4_XS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let process = start_recorded_prompt(home.path());
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/qwen-hello-low.txt"),
    )
    .await;
    let output = collect_output(process).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Hello! How can I help you today?\n"
    );
    let path = home.path().join("recordings/llama/v1-chat-completions");
    assert_eq!(
        std::fs::read_dir(path)
            .expect("the recording directory exists")
            .count(),
        1
    );
}

#[tokio::test]
async fn user_sees_response_recording_error() {
    let (address, mut requests) = RecordedProvider::new().start().await;
    let home = create_home(json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Qwen3.8-27B-UD-IQ4_XS",
                "models": [
                    {
                        "name": "Qwen3.8-27B-UD-IQ4_XS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    }));
    let directory = home.path().join("recordings/llama");
    std::fs::create_dir_all(&directory).expect("the recording directory can be created");
    std::fs::write(directory.join("v1-chat-completions"), "file")
        .expect("the recording path can be occupied");
    let process = start_recorded_prompt(home.path());
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/qwen-hello-low.txt"),
    )
    .await;
    let output = collect_output(process).await;
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("recording"), "{error}");
    assert!(error.contains("v1-chat-completions"), "{error}");
}
