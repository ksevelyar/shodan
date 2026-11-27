use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use super::support::{
    ASTEROIDS_PLAN_PROMPT, RecordedLlama, assert_received_messages, create_home, read_bottom_line,
    read_history, read_screen, receive_request, receive_request_or_error, run_test_application,
    run_test_application_with_configured_clipboard, send_initial_recorded_response,
    send_recorded_response, send_remaining_recorded_response, send_response_bytes,
    wait_for_response_disconnect,
};
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ratatui::style::{Color, Modifier};
use shodan::settings::build_settings;

#[tokio::test]
async fn user_copies_selected_text_with_configured_clipboard_command() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "model",
                "models": [
                    {
                        "name": "model"
                    }
                ]
            }
        },
        "clipboard_command": [
            "sh",
            "-c",
            "cat > \"$1\"",
            "clipboard",
            "temporary"
        ]
    }));
    let clipboard_path = home.path().join("clipboard");
    let settings_path = home.path().join(".shodan/settings.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).expect("the settings can be read"))
            .expect("the settings are valid");
    configuration["clipboard_command"][4] = serde_json::json!(clipboard_path);
    std::fs::write(settings_path, configuration.to_string()).expect("the settings can be written");
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application_with_configured_clipboard(settings);
    application.enter_text("clipboard text");
    application
        .wait_for_screen(|screen| read_screen(screen).contains("clipboard text"))
        .await;
    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 0, 21);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 8, 21);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 8, 21);
    let clipboard_timeout = Duration::from_secs(5);
    tokio::time::timeout(clipboard_timeout, async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&clipboard_path)
                && text == "clipboard"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the command receives the selected text");
}

#[tokio::test]
async fn user_copies_highlighted_text() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}},
        "clipboard_command": ["sh", "-c", "cat > \"$1\"", "clipboard", "temporary"]
    }));
    let clipboard_path = home.path().join("clipboard");
    let settings_path = home.path().join(".shodan/settings.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).expect("the settings can be read"))
            .expect("the settings are valid");
    configuration["clipboard_command"][4] = serde_json::json!(clipboard_path);
    std::fs::write(settings_path, configuration.to_string()).expect("the settings can be written");
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application_with_configured_clipboard(settings);
    application.send_message("*plain*");
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("plain"))
        .await;
    let row = (0..screen.area.height)
        .find(|&row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
                .trim()
                == "plain"
        })
        .expect("the message is visible");
    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 0, row);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 4, row);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 4, row);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(&clipboard_path).is_ok_and(|text| text == "plain") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the clipboard receives the displayed text");
}

#[tokio::test]
async fn user_reads_markdown_in_message() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("## Heading **bold** *plain* `code` [link](https://example.com)");
    let screen = application
        .wait_for_screen(|screen| {
            read_history(screen).contains("## Heading bold plain code https://example.com")
        })
        .await;
    let row = (0..screen.area.height)
        .find(|&row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
                .contains("## Heading bold plain code")
        })
        .expect("the message is visible");
    assert!(screen[(0, row)].modifier.contains(Modifier::BOLD));
    assert!(screen[(11, row)].modifier.contains(Modifier::BOLD));
    assert!(!screen[(16, row)].modifier.contains(Modifier::ITALIC));
    assert_ne!(screen[(22, row)].fg, screen[(16, row)].fg);
}

#[tokio::test]
async fn user_reads_markdown_in_model_response() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Write a plan");
    let request = receive_request(&mut requests).await;
    send_initial_recorded_response(&request).await;
    send_remaining_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("### 5. User collects all debris"))
        .await;
    let row = (0..screen.area.height)
        .find(|&row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
                .contains("### 5. User collects all debris")
        })
        .expect("the heading is visible");
    assert!(screen[(0, row)].modifier.contains(Modifier::BOLD));
}

#[tokio::test]
async fn user_reads_code_block() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.paste_text("```rust\nfn main() {}\n```");
    application.press_key(KeyCode::Enter);
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("fn main() {}"))
        .await;
    assert!(read_history(&screen).contains("```rust"));
    let row = (0..screen.area.height)
        .find(|&row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
                .contains("fn main() {}")
        })
        .expect("the code is visible");
    assert_ne!(screen[(0, row)].fg, Color::Reset);
}

#[tokio::test]
async fn user_reads_long_hyphenated_text() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "model",
                "models": [
                    {
                        "name": "model"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    let message = format!(
        "{}-{}-{}",
        "a".repeat(110),
        "b".repeat(130),
        "c".repeat(110)
    );
    application.send_message(&message);
    application
        .wait_for_screen(|screen| {
            let history: String = read_history(screen)
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            history.contains(&message)
        })
        .await;
}

#[tokio::test]
async fn user_aborts_model_response() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;

    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_initial_recorded_response(&first_request).await;
    application
        .wait_for_screen(|screen| {
            (0..screen.area.height - 4).any(|row| {
                let line = (0..screen.area.width)
                    .map(|column| screen[(column, row)].symbol())
                    .collect::<String>();
                line.trim() == "##"
            })
        })
        .await;

    application.press_key(KeyCode::Esc);
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("Request was aborted"))
        .await;
    let notice_row = (1..screen.area.height - 4)
        .find(|&row| {
            let line = (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>();
            line.trim() == "Request was aborted"
        })
        .expect("the abort notice is visible");
    let partial_response = (0..screen.area.width)
        .map(|column| screen[(column, notice_row - 1)].symbol())
        .collect::<String>();
    assert_eq!(partial_response.trim(), "##");
    for column in 0.."Request was aborted".len() {
        let column = u16::try_from(column).expect("the notice fits in the terminal");
        assert_eq!(screen[(column, notice_row)].fg, Color::Rgb(255, 16, 240));
    }
    wait_for_response_disconnect(&first_request).await;

    application.send_message("Continue");
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    assert_received_messages(
        &second_request,
        &[
            ("system", include_str!("../../AGENTS.md")),
            ("user", ASTEROIDS_PLAN_PROMPT),
            ("user", "Continue"),
        ],
    );
    send_recorded_response(&second_request).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
        .await;
}

#[tokio::test]
async fn user_sees_failed_request() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    send_initial_recorded_response(&first_request).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("##"))
        .await;
    drop(first_request);
    application
        .wait_for_screen(|screen| read_history(screen).contains("Request failed"))
        .await;
    application.send_message("Continue");
    let second_request = receive_request(&mut requests).await;
    assert_received_messages(
        &second_request,
        &[
            ("system", include_str!("../../AGENTS.md")),
            ("user", ASTEROIDS_PLAN_PROMPT),
            ("user", "Continue"),
        ],
    );
    send_recorded_response(&second_request).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
        .await;
}

#[tokio::test]
async fn user_aborts_tool_call_response() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
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
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
    let prompt = "List all entries in tests/features/fixtures/tools/user_lists_directory, including hidden entries, using the ls tool. Show the exact listing.";
    application.send_message(prompt);
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    let response = include_str!("fixtures/llama/tools/user_lists_directory/response-1.txt");
    let call_start = response
        .find("\"tool_calls\"")
        .expect("the recorded response contains a tool call");
    let call_end = response[call_start..]
        .find("\n\n")
        .expect("the recorded tool call chunk ends")
        + call_start
        + 2;
    send_response_bytes(&first_request, &response[..call_end]).await;
    application.press_key(KeyCode::Esc);
    application
        .wait_for_screen(|screen| read_history(screen).contains("Request was aborted"))
        .await;
    wait_for_response_disconnect(&first_request).await;
    let next_prompt = "Ignore the previous request. Say hello.";
    application.send_message(next_prompt);
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    assert_received_messages(
        &second_request,
        &[
            ("system", include_str!("../../AGENTS.md")),
            ("user", prompt),
            ("user", next_prompt),
        ],
    );
    super::tools::support::send_scenario_response(
        &second_request,
        "user_aborts_tool_call_response",
        2,
    )
    .await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello"))
        .await;
}

#[tokio::test]
async fn user_watches_model_thinking() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
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
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Qwen3.8-27B-UD-IQ4_XS"))
        .await;
    application.send_message("Read tests/features/fixtures/tools/user_reads_file/greeting.txt using the read tool and show its exact contents.");
    let first_request = receive_request_or_error(&mut requests, &mut application).await;
    let response = include_str!("fixtures/llama/tools/user_reads_file/response-1.txt");
    let thinking_end = response
        .find("\"reasoning_content\":\" asking\"")
        .expect("the recorded response contains the model's thinking");
    let thinking_end = thinking_end
        + response[thinking_end..]
            .find("\n\n")
            .expect("the recorded thinking chunk ends")
        + 2;
    send_response_bytes(&first_request, &response[..thinking_end]).await;
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("The user is asking"))
        .await;
    for text in ["Thinking:", "The user is asking"] {
        let row = (0..screen.area.height - 4)
            .find(|&row| {
                let line = (0..screen.area.width)
                    .map(|column| screen[(column, row)].symbol())
                    .collect::<String>();
                line.starts_with(text)
            })
            .expect("the thinking is visible");
        for column in 0..text.len() {
            let column = u16::try_from(column).expect("the thinking fits in the terminal");
            assert_eq!(screen[(column, row)].fg, Color::Rgb(128, 128, 128));
        }
    }
    send_response_bytes(&first_request, &response[thinking_end..]).await;
    let second_request = receive_request_or_error(&mut requests, &mut application).await;
    super::tools::support::send_scenario_response(&second_request, "user_reads_file", 2).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, Shodan!"))
        .await;
}

#[tokio::test]
async fn user_selects_visible_text() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("restart the game."))
        .await;
    application.enter_text("message");
    application
        .wait_for_screen(|screen| read_screen(screen).contains("message"))
        .await;

    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 2, 0);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 5, 11);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 5, 11);
    application
        .wait_for_screen(|screen| {
            [(2, 0), (0, 8), (0, 9), (0, 10), (5, 11)]
                .iter()
                .all(|position| screen[*position].modifier.contains(Modifier::REVERSED))
        })
        .await;

    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 7, 9);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 1, 9);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 1, 9);
    application
        .wait_for_screen(|screen| {
            screen[(1, 9)].modifier.contains(Modifier::REVERSED)
                && screen[(7, 9)].modifier.contains(Modifier::REVERSED)
                && !screen[(2, 0)].modifier.contains(Modifier::REVERSED)
        })
        .await;
}

#[tokio::test]
async fn user_scrolls_with_conversation_selection() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("restart the game."))
        .await;
    let original_history = read_history(&screen);
    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 0, 2);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 4, 2);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 4, 2);
    application
        .wait_for_screen(|screen| screen[(0, 2)].modifier.contains(Modifier::REVERSED))
        .await;
    application.scroll_mouse(MouseEventKind::ScrollUp, 0, 9);
    application
        .wait_for_screen(|screen| read_history(screen) != original_history)
        .await;
    application.scroll_mouse(MouseEventKind::ScrollDown, 0, 9);
    application
        .wait_for_screen(|screen| {
            read_history(screen) == original_history
                && screen[(0, 2)].modifier.contains(Modifier::REVERSED)
                && screen[(4, 2)].modifier.contains(Modifier::REVERSED)
        })
        .await;
}

#[tokio::test]
async fn user_extends_selection_beyond_visible_conversation() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request(&mut requests).await;
    send_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("restart the game."))
        .await;
    let original_history = read_history(&screen);
    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 2, 3);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 2, 0);
    application
        .wait_for_screen(|screen| {
            read_history(screen) != original_history
                && screen[(2, 0)].modifier.contains(Modifier::REVERSED)
        })
        .await;
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 2, 0);
}

#[tokio::test]
async fn user_scrolls_conversation_history_with_mouse_wheel() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request_or_error(&mut requests, &mut application).await;
    send_recorded_response(&request).await;
    let bottom_screen = application
        .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
        .await;
    let bottom_history = read_history(&bottom_screen);

    application.scroll_mouse(MouseEventKind::ScrollUp, 0, 9);
    let earlier_screen = application
        .wait_for_screen(|screen| read_history(screen) != bottom_history)
        .await;
    let earlier_history = read_history(&earlier_screen);
    let width = usize::from(earlier_screen.area.width);
    let scroll_lines = 3;
    let scroll_offset = width * scroll_lines;
    let earlier_lines = earlier_history
        .chars()
        .skip(scroll_offset)
        .collect::<String>();
    let bottom_lines = bottom_history
        .chars()
        .take(earlier_lines.chars().count())
        .collect::<String>();
    assert_eq!(earlier_lines, bottom_lines);

    application.scroll_mouse(MouseEventKind::ScrollDown, 0, 11);
    application
        .wait_for_screen(|screen| read_history(screen) == bottom_history)
        .await;
}

#[tokio::test]
async fn user_composes_message() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.enter_text("Hello");
    let screen = application
        .wait_for_screen(|screen| read_screen(screen).contains("Hello"))
        .await;
    for row in [screen.area.height - 4, screen.area.height - 2] {
        for column in 0..screen.area.width {
            let cell = &screen[(column, row)];
            assert_eq!(cell.symbol(), "─");
            assert_eq!(cell.fg, Color::Gray);
        }
    }
    application.press_key(KeyCode::Enter);
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("Hello"))
        .await;
    let row = (0..screen.area.height - 4)
        .find(|&row| screen[(0, row)].symbol() == "H")
        .expect("the user message is visible");
    for column in 0..screen.area.width {
        assert_eq!(screen[(column, row)].bg, Color::Rgb(55, 55, 55));
    }
}

#[cfg(unix)]
#[test]
fn user_exits_with_ctrl_c() {
    let terminal = native_pty_system()
        .openpty(PtySize::default())
        .expect("the feature test creates a pseudo-terminal");
    let original_settings = terminal
        .master
        .get_termios()
        .expect("the pseudo-terminal exposes its settings");
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_shodan"));
    command.env("HOME", &home_directory);
    command.env("TERM", "xterm-256color");
    let mut application = terminal
        .slave
        .spawn_command(command)
        .expect("the application starts in the pseudo-terminal");
    let timeout = Duration::from_secs(5);
    let polling_interval = Duration::from_millis(10);
    let startup_deadline = Instant::now() + timeout;
    loop {
        let settings = terminal
            .master
            .get_termios()
            .expect("the pseudo-terminal exposes its settings");
        if settings.local_flags != original_settings.local_flags {
            break;
        }
        assert!(
            application
                .try_wait()
                .expect("the application status is available")
                .is_none(),
            "the application exits before starting the terminal session"
        );
        if Instant::now() >= startup_deadline {
            application.kill().expect("the application can be stopped");
            panic!("the application does not start the terminal session");
        }
        thread::sleep(polling_interval);
    }

    let mut terminal_input = terminal
        .master
        .take_writer()
        .expect("the feature test can send terminal input");
    terminal_input
        .write_all(&[3])
        .expect("the user can press Ctrl-C");
    terminal_input.flush().expect("terminal input is sent");

    let exit_deadline = Instant::now() + timeout;
    let exit_status = loop {
        if let Some(status) = application
            .try_wait()
            .expect("the application status is available")
        {
            break status;
        }
        if Instant::now() >= exit_deadline {
            application.kill().expect("the application can be stopped");
            panic!("the application does not exit after Ctrl-C");
        }
        thread::sleep(polling_interval);
    };
    assert!(exit_status.success(), "the application exits successfully");
    let restored_settings = terminal
        .master
        .get_termios()
        .expect("the pseudo-terminal exposes its settings");
    assert_eq!(restored_settings, original_settings);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_session_and_scrolls_through_long_response() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": server_address,
                "default_model": "LFM3-Omni-2B-Q4_K_M",
                "models": [
                    {
                        "name": "LFM3-Omni-2B-Q4_K_M"
                    },
                    {
                        "name": "MiniCPM5-2B-Q4_K_M"
                    }
                ]
            }
        }
    }));
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);

    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request_or_error(&mut requests, &mut application).await;
    assert_received_messages(
        &request,
        &[
            ("system", include_str!("../../AGENTS.md")),
            ("user", ASTEROIDS_PLAN_PROMPT),
        ],
    );
    let before_response = application
        .wait_for_screen(|screen| read_history(screen).contains("Write a proof-of-concept plan"))
        .await;
    send_initial_recorded_response(&request).await;
    let screen = application
        .wait_for_screen(|screen| read_history(screen) != read_history(&before_response))
        .await;
    let history = read_history(&screen);
    assert!(history.contains("##"));
    send_remaining_recorded_response(&request).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
        .await;

    for _ in 0..100 {
        application.press_key(KeyCode::PageUp);
    }
    let earlier_history = application
        .wait_for_screen(|screen| {
            let contents = read_screen(screen);
            contents.contains("Write a proof-of-concept plan")
        })
        .await;
    application.press_key(KeyCode::PageDown);
    application
        .wait_for_screen(|screen| read_history(screen) != read_history(&earlier_history))
        .await;
    for _ in 0..100 {
        application.press_key(KeyCode::PageDown);
    }
    application
        .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
        .await;
}
