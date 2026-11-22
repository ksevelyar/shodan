use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use crossterm::event::KeyCode;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::json;

use super::support::{
    finish_response, read_history, read_screen, receive_request, run_test_application,
    send_multiline_response_chunk, send_response_chunk, start_mock_server,
};

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
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_shodan"));
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
}

#[tokio::test]
async fn user_starts_session_and_scrolls_through_long_response() {
    let (server_address, mut requests) = start_mock_server().await;
    let mut application = run_test_application(server_address, 80, 12);

    application.send_message("say hi in spanish");
    let first_request = receive_request(&mut requests).await;
    assert_eq!(
        first_request.body["messages"],
        json!([
            {"role": "system", "content": "you are helpful assistent"},
            {"role": "user", "content": "say hi in spanish"}
        ])
    );
    send_multiline_response_chunk(&first_request, "hola").await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("hola"))
        .await;
    finish_response(&first_request, None).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("test-model"))
        .await;

    let question =
        "explain what Feynman meant when he said no one truly understands quantum mechanics";
    application.enter_text(question);
    application
        .wait_for_screen(|screen| read_screen(screen).contains("quantum mechanics"))
        .await;
    application.press_key(KeyCode::Enter);
    let second_request = receive_request(&mut requests).await;
    assert_eq!(
        second_request.body["messages"],
        json!([
            {"role": "system", "content": "you are helpful assistent"},
            {"role": "user", "content": "say hi in spanish"},
            {"role": "assistant", "content": "hola"},
            {"role": "user", "content": question}
        ])
    );
    let response = include_str!("fixtures/feynman-response.txt");
    let (first_paragraph, remaining_response) = response
        .split_once("\n\n")
        .expect("the long response contains multiple paragraphs");
    send_response_chunk(&second_request, first_paragraph).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("Feynman meant"))
        .await;
    send_response_chunk(&second_request, &format!("\n\n{remaining_response}")).await;
    application
        .wait_for_screen(|screen| read_screen(screen).contains("comfortable picture"))
        .await;
    finish_response(&second_request, None).await;

    for _ in 0..20 {
        application.press_key(KeyCode::PageUp);
    }
    let earlier_history = application
        .wait_for_screen(|screen| {
            let contents = read_screen(screen);
            contents.contains("say hi in spanish") && contents.contains("hola")
        })
        .await;
    application.press_key(KeyCode::PageDown);
    application
        .wait_for_screen(|screen| read_history(screen) != read_history(&earlier_history))
        .await;
    for _ in 0..20 {
        application.press_key(KeyCode::PageDown);
    }
    application
        .wait_for_screen(|screen| read_screen(screen).contains("comfortable picture"))
        .await;
}
