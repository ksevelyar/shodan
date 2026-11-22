use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use super::support::{
    ASTEROIDS_PLAN_PROMPT, RecordedModels, assert_received_messages, read_bottom_line,
    read_history, read_screen, receive_request_or_error, run_test_application,
    send_initial_recorded_response, send_remaining_recorded_response, start_mock_server,
};
use crossterm::event::KeyCode;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ratatui::style::Color;

#[tokio::test]
async fn user_composes_message() {
    let (server_address, _requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 80, 12);
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
    let (server_address, mut requests) = start_mock_server(RecordedModels::BothLoaded).await;
    let mut application = run_test_application(server_address, 80, 12);

    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("LFM3-Omni-2B-Q4_K_M"))
        .await;
    application.send_message(ASTEROIDS_PLAN_PROMPT);
    let request = receive_request_or_error(&mut requests, &mut application).await;
    assert_received_messages(
        &request,
        &[
            ("system", "you are helpful assistent"),
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
