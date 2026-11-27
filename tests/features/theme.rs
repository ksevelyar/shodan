use super::support::{create_home, read_bottom_line, read_history, run_test_application};
use crossterm::event::{MouseButton, MouseEventKind};
use ratatui::style::Color;
use shodan::settings::build_settings;

#[tokio::test]
async fn user_views_interface() {
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": "http://127.0.0.1:8080", "default_model": "model", "models": [{"name": "model"}]}}
    }));
    let settings = build_settings(home.path().to_owned(), Vec::new()).expect("settings are valid");
    let mut application = run_test_application(settings);
    let screen = application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("llama model"))
        .await;
    assert_eq!(screen[(0, 0)].fg, Color::Rgb(129, 124, 134));
    assert_eq!(screen[(0, 0)].bg, Color::Rgb(27, 22, 31));
    assert_eq!(
        screen[(0, screen.area.height - 1)].fg,
        Color::Rgb(129, 124, 134)
    );
    assert_eq!(
        screen[(0, screen.area.height - 1)].bg,
        Color::Rgb(27, 22, 31)
    );
    assert_eq!(screen[(0, 1)].symbol(), "─");
    assert_eq!(screen[(0, 2)].bg, Color::Rgb(27, 22, 31));
    let bottom_row = screen.area.height - 1;
    application.scroll_mouse(MouseEventKind::Down(MouseButton::Left), 0, bottom_row);
    application.scroll_mouse(MouseEventKind::Drag(MouseButton::Left), 3, bottom_row);
    application.scroll_mouse(MouseEventKind::Up(MouseButton::Left), 3, bottom_row);
    application
        .wait_for_screen(|screen| {
            screen[(0, bottom_row)].bg == Color::Rgb(213, 170, 42)
                && screen[(0, bottom_row)].fg == Color::Rgb(27, 22, 31)
        })
        .await;
    assert_eq!(
        screen[(0, screen.area.height - 4)].fg,
        Color::Rgb(94, 107, 133)
    );
    application.send_message("Hello");
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("Hello"))
        .await;
    let row = (0..screen.area.height)
        .find(|row| screen[(0, *row)].symbol() == "H")
        .expect("the user message is visible");
    assert_eq!(screen[(0, row)].bg, Color::Rgb(27, 22, 31));
    assert_eq!(screen[(0, row)].fg, Color::Rgb(213, 170, 42));
}
