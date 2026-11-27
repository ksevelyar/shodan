use crossterm::event::KeyCode;
use shodan::settings::build_settings;

use super::super::support::{
    RecordedLlama, create_home, read_screen, receive_request, run_test_application,
    run_test_application_with_size, send_recorded_response,
};

#[tokio::test]
async fn user_edits_message_with_horizontal_arrow_keys() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.enter_text("ac");
    application.press_key(KeyCode::Left);
    application.enter_text("b");
    application
        .wait_for_screen(|screen| read_screen(screen).contains("abc"))
        .await;
    application.press_key(KeyCode::Left);
    application.press_key(KeyCode::Backspace);
    application.press_key(KeyCode::Right);
    application.enter_text("d");
    application
        .wait_for_screen(|screen| read_screen(screen).contains("bdc"))
        .await;
    application.press_key(KeyCode::Enter);
    application
        .wait_for_screen(|screen| super::super::support::read_history(screen).contains("bdc"))
        .await;
}

#[tokio::test]
async fn user_sends_multiline_message() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.enter_text("first");
    application.press_shift_enter();
    application.enter_text("second");
    application
        .wait_for_screen(|screen| {
            let text = read_screen(screen);
            text.contains("first") && text.contains("second")
        })
        .await;
    application.press_key(KeyCode::Enter);
    application
        .wait_for_screen(|screen| {
            let history = super::super::support::read_history(screen);
            history.contains("first") && history.contains("second")
        })
        .await;
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][1]["content"], "first\nsecond");

    #[cfg(unix)]
    verify_multiline_message_in_managed_terminal(home.path(), &mut requests).await;
}

#[cfg(unix)]
async fn verify_multiline_message_in_managed_terminal(
    home_directory: &std::path::Path,
    requests: &mut tokio::sync::mpsc::Receiver<super::super::support::CapturedRequest>,
) {
    use std::io::{Read, Write};

    let terminal = portable_pty::native_pty_system()
        .openpty(portable_pty::PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("the feature test creates a terminal");
    let mut command = portable_pty::CommandBuilder::new(env!("CARGO_BIN_EXE_shodan"));
    command.env("HOME", home_directory);
    command.env("TERM", "xterm-256color");
    let mut process = terminal
        .slave
        .spawn_command(command)
        .expect("Shodan starts");
    let mut reader = terminal
        .master
        .try_clone_reader()
        .expect("terminal output is readable");
    let mut writer = terminal
        .master
        .take_writer()
        .expect("terminal input is writable");
    let (output_sender, mut output_receiver) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(size) = reader.read(&mut buffer) {
            if size == 0 || output_sender.send(buffer[..size].to_vec()).is_err() {
                break;
            }
        }
    });
    let timeout = std::time::Duration::from_secs(5);
    let mut output = Vec::new();
    tokio::time::timeout(timeout, async {
        while !output.windows(5).any(|window| window == b"\x1b[>1u") {
            let bytes = output_receiver.recv().await.expect("terminal remains open");
            output.extend(bytes);
        }
    })
    .await
    .expect("Shodan enables enhanced keyboard reporting");
    writer
        .write_all(b"first\x1b[13;2usecond\r")
        .expect("the user enters a multiline message");
    writer.flush().expect("terminal input is sent");
    let request = receive_request(requests).await;
    assert_eq!(request.body["messages"][1]["content"], "first\nsecond");
    writer.write_all(&[3]).expect("the user exits");
    writer.flush().expect("terminal input is sent");
    tokio::time::timeout(timeout, async {
        while !output.windows(5).any(|window| window == b"\x1b[<1u") {
            let bytes = output_receiver
                .recv()
                .await
                .expect("terminal remains open until keyboard reporting is restored");
            output.extend(bytes);
        }
    })
    .await
    .expect("Shodan restores keyboard reporting");
    let status = process.wait().expect("Shodan exits");
    assert!(status.success());
}

#[tokio::test]
async fn user_edits_multiline_message_with_vertical_arrow_keys() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.enter_text("first");
    application.press_shift_enter();
    application.enter_text("third");
    application.press_key(KeyCode::Up);
    application.press_shift_enter();
    application.enter_text("second");
    application.press_key(KeyCode::Down);
    application.enter_text("!");
    application
        .wait_for_screen(|screen| read_screen(screen).contains("third!"))
        .await;
    application.press_key(KeyCode::Enter);
    application
        .wait_for_screen(|screen| {
            let history = super::super::support::read_history(screen);
            history.contains("first") && history.contains("second") && history.contains("third!")
        })
        .await;
    let request = receive_request(&mut requests).await;
    assert_eq!(
        request.body["messages"][1]["content"],
        "first\nsecond\nthird!"
    );
}

#[tokio::test]
async fn user_edits_wrapped_message_with_vertical_arrow_keys() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let application = run_test_application_with_size(settings, 30, 20);
    application.enter_text(&format!("{}bbb", "a".repeat(30)));
    application.press_key(KeyCode::Up);
    application.enter_text("X");
    application.press_key(KeyCode::Down);
    application.enter_text("Y");
    application.press_key(KeyCode::Enter);
    let request = receive_request(&mut requests).await;
    let expected = format!("aaaX{}bbbY", "a".repeat(27));
    assert_eq!(request.body["messages"][1]["content"], expected);
}

#[tokio::test]
async fn user_navigates_message_history_and_restores_draft() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    for message in ["first message", "second message"] {
        application.send_message(message);
        let request = receive_request(&mut requests).await;
        send_recorded_response(&request).await;
        application
            .wait_for_screen(|screen| read_screen(screen).contains("restart the game."))
            .await;
    }
    application.enter_text("draft");
    application.press_key(KeyCode::Up);
    application
        .wait_for_screen(|screen| get_input_line(screen).contains("second message"))
        .await;
    application.press_key(KeyCode::Up);
    application
        .wait_for_screen(|screen| get_input_line(screen).contains("first message"))
        .await;
    application.press_key(KeyCode::Down);
    application
        .wait_for_screen(|screen| get_input_line(screen).contains("second message"))
        .await;
    application.press_key(KeyCode::Down);
    application
        .wait_for_screen(|screen| get_input_line(screen).contains("draft"))
        .await;
}

fn get_input_line(screen: &ratatui::buffer::Buffer) -> String {
    let row = screen.area.height - 3;
    (0..screen.area.width)
        .map(|column| screen[(column, row)].symbol())
        .collect()
}

#[tokio::test]
async fn user_pastes_multiline_text_at_cursor() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    application.enter_text("start end");
    for _ in 0..4 {
        application.press_key(KeyCode::Left);
    }
    application.paste_text("one\ntwo");
    application
        .wait_for_screen(|screen| {
            let text = read_screen(screen);
            text.contains("startone") && text.contains("two end")
        })
        .await;
    application.press_key(KeyCode::Enter);
    application
        .wait_for_screen(|screen| {
            let history = super::super::support::read_history(screen);
            history.contains("startone") && history.contains("two end")
        })
        .await;
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][1]["content"], "startone\ntwo end");
}

#[tokio::test]
async fn user_grows_and_shrinks_message_input() {
    let (server_address, _requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application_with_size(settings, 30, 20);
    application.enter_text(&"a".repeat(31));
    application
        .wait_for_screen(|screen| {
            screen[(0, 15)].symbol() == "─"
                && screen[(0, 16)].symbol() == "a"
                && screen[(0, 17)].symbol() == "a"
        })
        .await;
    application.press_key(KeyCode::Backspace);
    application
        .wait_for_screen(|screen| {
            screen[(0, 16)].symbol() == "─" && screen[(0, 17)].symbol() == "a"
        })
        .await;
}

#[tokio::test]
async fn user_edits_input_beyond_height_limit() {
    let (server_address, mut requests) = RecordedLlama::new().start().await;
    let home = create_home(
        serde_json::json!({"default_provider": "llama", "providers": {"llama": {"address": server_address, "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application_with_size(settings, 30, 20);
    let message = (1..=12)
        .map(|number| format!("line {number:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    application.paste_text(&message);
    application
        .wait_for_screen(|screen| {
            screen[(0, 7)].symbol() == "─" && read_screen(screen).contains("line 12")
        })
        .await;
    for _ in 0..message.chars().count() {
        application.press_key(KeyCode::Left);
    }
    application
        .wait_for_screen(|screen| read_screen(screen).contains("line 01"))
        .await;
    application.press_key(KeyCode::Enter);
    application
        .wait_for_screen(|screen| read_screen(screen).contains("line 12"))
        .await;
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["messages"][1]["content"], message);
}
