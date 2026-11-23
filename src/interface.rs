use std::{io, ops::ControlFlow};

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::{Stream, StreamExt};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    application::{Application, Statistics},
    chats::{Message, Role, send_message},
};

struct TerminalSession {
    alternate_screen: bool,
    raw_mode: bool,
    mouse_capture: bool,
}

impl TerminalSession {
    fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut session = Self {
            alternate_screen: false,
            raw_mode: true,
            mouse_capture: false,
        };
        execute!(io::stdout(), EnterAlternateScreen)?;
        session.alternate_screen = true;
        session.mouse_capture = true;
        execute!(io::stdout(), EnableMouseCapture)?;
        Ok(session)
    }

    fn restore(&mut self) -> io::Result<()> {
        let mouse_result = if self.mouse_capture {
            let result = execute!(io::stdout(), DisableMouseCapture);
            if result.is_ok() {
                self.mouse_capture = false;
            }
            result
        } else {
            Ok(())
        };
        let screen_result = if self.alternate_screen {
            let result = execute!(io::stdout(), LeaveAlternateScreen);
            if result.is_ok() {
                self.alternate_screen = false;
            }
            result
        } else {
            Ok(())
        };
        let raw_result = if self.raw_mode {
            let result = disable_raw_mode();
            if result.is_ok() {
                self.raw_mode = false;
            }
            result
        } else {
            Ok(())
        };
        mouse_result.and(screen_result).and(raw_result)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[derive(Clone, Copy)]
pub enum TerminalMode {
    Managed,
    Unmanaged,
}

pub struct Interface<BackendType: Backend, EventSource> {
    terminal: Terminal<BackendType>,
    events: EventSource,
    terminal_session: Option<TerminalSession>,
}

pub(crate) fn build_interface<BackendType: Backend, EventSource>(
    backend: BackendType,
    events: EventSource,
    terminal_mode: TerminalMode,
) -> io::Result<Interface<BackendType, EventSource>>
where
    BackendType::Error: Send + Sync + 'static,
{
    let terminal_session = match terminal_mode {
        TerminalMode::Managed => Some(TerminalSession::start()?),
        TerminalMode::Unmanaged => None,
    };
    let terminal = Terminal::new(backend).map_err(io::Error::other)?;
    Ok(Interface {
        terminal,
        events,
        terminal_session,
    })
}

pub(crate) fn restore_terminal<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) -> io::Result<()> {
    interface
        .terminal_session
        .as_mut()
        .map_or(Ok(()), TerminalSession::restore)
}

pub(crate) fn draw<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
    application: &mut Application,
) -> io::Result<()>
where
    BackendType::Error: Send + Sync + 'static,
{
    interface
        .terminal
        .draw(|frame| {
            draw_interface(
                frame,
                &application.messages,
                &application.input,
                &application.statistics,
                application.error.as_deref(),
                application.scroll_from_bottom,
                &mut application.maximum_scroll,
            );
        })
        .map_err(io::Error::other)?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum InterfaceInput {
    Key(KeyEvent),
    Mouse(MouseEventKind),
    Exit,
    Redraw,
}

pub(crate) async fn wait_for_input<BackendType, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) -> io::Result<InterfaceInput>
where
    BackendType: Backend,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    match interface.events.next().await {
        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
            Ok(InterfaceInput::Key(key))
        }
        Some(Ok(Event::Mouse(mouse))) => Ok(InterfaceInput::Mouse(mouse.kind)),
        Some(Ok(_)) => Ok(InterfaceInput::Redraw),
        Some(Err(error)) => Err(error),
        None => Ok(InterfaceInput::Exit),
    }
}

pub(crate) fn handle_input(
    application: &mut Application,
    input: InterfaceInput,
) -> ControlFlow<()> {
    match input {
        InterfaceInput::Exit => ControlFlow::Break(()),
        InterfaceInput::Key(key)
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            ControlFlow::Break(())
        }
        InterfaceInput::Key(key) => {
            handle_key(application, key);
            ControlFlow::Continue(())
        }
        InterfaceInput::Mouse(kind) => {
            match kind {
                MouseEventKind::ScrollUp => scroll_up(application, 1),
                MouseEventKind::ScrollDown => scroll_down(application, 1),
                _ => {}
            }
            ControlFlow::Continue(())
        }
        InterfaceInput::Redraw => ControlFlow::Continue(()),
    }
}

fn handle_key(application: &mut Application, key: KeyEvent) {
    match key.code {
        KeyCode::Char(character)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            application.input.push(character);
        }
        KeyCode::Backspace => {
            application.input.pop();
        }
        KeyCode::Enter
            if application.statistics.response_started.is_none()
                && !application.statistics.model.is_empty()
                && !application.input.trim().is_empty() =>
        {
            let input = std::mem::take(&mut application.input);
            send_message(application, input);
            application.scroll_from_bottom = 0;
        }
        KeyCode::PageUp => scroll_up(application, 8),
        KeyCode::PageDown => scroll_down(application, 8),
        _ => {}
    }
}

fn scroll_up(application: &mut Application, lines: usize) {
    application.scroll_from_bottom = application
        .scroll_from_bottom
        .saturating_add(lines)
        .min(application.maximum_scroll);
}

fn scroll_down(application: &mut Application, lines: usize) {
    application.scroll_from_bottom = application
        .scroll_from_bottom
        .min(application.maximum_scroll)
        .saturating_sub(lines);
}

pub(crate) fn draw_interface(
    frame: &mut Frame<'_>,
    messages: &[Message],
    input: &str,
    statistics: &Statistics,
    error: Option<&str>,
    scroll_from_bottom: usize,
    maximum_scroll: &mut usize,
) {
    let areas = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let width = usize::from(areas[0].width).max(1);
    let mut lines = Vec::new();
    for message in messages {
        if matches!(message.role, Role::System) {
            continue;
        }
        for line in message.content.split('\n') {
            let wrapped_lines = if line.is_empty() {
                vec![std::borrow::Cow::Borrowed("")]
            } else {
                textwrap::wrap(line, width)
            };
            for wrapped_line in wrapped_lines {
                if matches!(message.role, Role::User) {
                    let padding = width.saturating_sub(wrapped_line.width());
                    let background = Color::Rgb(55, 55, 55);
                    lines.push(
                        Line::from(format!("{wrapped_line}{}", " ".repeat(padding)))
                            .style(Style::default().bg(background)),
                    );
                } else {
                    lines.push(Line::from(wrapped_line.into_owned()));
                }
            }
        }
        lines.push(Line::default());
    }
    if let Some(error) = error {
        lines.extend(
            textwrap::wrap(&format!("Error: {error}"), width)
                .into_iter()
                .map(|line| Line::from(line.into_owned())),
        );
    }
    let height = usize::from(areas[0].height);
    *maximum_scroll = lines.len().saturating_sub(height);
    let start = maximum_scroll.saturating_sub(scroll_from_bottom.min(*maximum_scroll));
    let visible = lines
        .into_iter()
        .skip(start)
        .take(height)
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible), areas[0]);
    for area in [areas[1], areas[3]] {
        let line = "─".repeat(usize::from(area.width));
        frame.render_widget(
            Paragraph::new(line).style(Style::default().fg(Color::Gray)),
            area,
        );
    }
    let input_width = input.width();
    let viewport_width = usize::from(areas[2].width);
    let horizontal_scroll = input_width.saturating_sub(viewport_width.saturating_sub(1));
    let horizontal_scroll = u16::try_from(horizontal_scroll).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(input).scroll((0, horizontal_scroll)),
        areas[2],
    );
    if areas[2].width > 0 && areas[2].height > 0 {
        let cursor_column = input_width
            .saturating_sub(usize::from(horizontal_scroll))
            .min(viewport_width - 1);
        let cursor_column = u16::try_from(cursor_column).unwrap_or(u16::MAX);
        frame.set_cursor_position((areas[2].x + cursor_column, areas[2].y));
    }
    draw_status(frame, areas[4], statistics);
}

fn draw_status(frame: &mut Frame<'_>, area: Rect, statistics: &Statistics) {
    let sent = statistics
        .prompt_tokens
        .saturating_sub(statistics.cached_tokens);
    let prompt_tokens = u128::from(statistics.prompt_tokens);
    let cache_tenths = (u128::from(statistics.cached_tokens) * 1000 + prompt_tokens / 2)
        .checked_div(prompt_tokens)
        .unwrap_or(0);
    let response_status = if let Some(started) = statistics.response_started {
        let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame_duration = 80;
        let frame_count = spinner_frames.len() as u128;
        let frame_index =
            usize::try_from(started.elapsed().as_millis() / frame_duration % frame_count)
                .expect("the spinner frame index fits in usize");
        Some(spinner_frames[frame_index].to_string())
    } else {
        statistics.response_time.map(|duration| {
            let seconds = duration.as_secs();
            let seconds_per_minute = 60;
            if seconds < seconds_per_minute {
                format!("⏱ {seconds}s")
            } else {
                let minutes = seconds / seconds_per_minute;
                let remaining_seconds = seconds % seconds_per_minute;
                format!("⏱ {minutes}m {remaining_seconds}s")
            }
        })
    };
    let response_prefix = response_status.map_or_else(String::new, |status| format!("{status} "));
    let left = format!(
        "{response_prefix}↑{} ↓{} CH{}.{}%",
        format_count(sent),
        format_count(statistics.completion_tokens),
        cache_tenths / 10,
        cache_tenths % 10
    );
    let right = format!("llama.cpp {}", statistics.model);
    let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let status_areas =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    frame.render_widget(Paragraph::new(left), status_areas[0]);
    frame.render_widget(Paragraph::new(right), status_areas[1]);
}

fn format_count(count: u64) -> String {
    if count >= 1000 {
        format!("{}k", count / 1000)
    } else {
        count.to_string()
    }
}
