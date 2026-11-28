mod edit;
mod input;
mod markdown;
mod theme;

use input::Input;

use std::{io, path::Path, sync::OnceLock};

#[cfg(unix)]
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
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
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use syntect::{easy::HighlightLines, parsing::SyntaxSet};
use theme::{Palette, get_syntax_theme};
use tokio::io::AsyncWriteExt;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    application::ApplicationError,
    chats::Message,
    session::{Session, Statistics},
    tools::{ToolCall, ToolOutput},
};

pub(crate) async fn copy_to_clipboard(text: &str, command: Option<&[String]>) -> bool {
    let automatic_command = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        vec!["wl-copy".to_owned()]
    } else if std::env::var_os("DISPLAY").is_some() {
        vec![
            "xclip".to_owned(),
            "-selection".to_owned(),
            "clipboard".to_owned(),
        ]
    } else {
        Vec::new()
    };
    let command = command.unwrap_or(&automatic_command);
    let Some((program, arguments)) = command.split_first() else {
        return false;
    };
    let child = tokio::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    let Some(mut input) = child.stdin.take() else {
        return false;
    };
    if input.write_all(text.as_bytes()).await.is_err() {
        return false;
    }
    drop(input);
    matches!(tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await, Ok(Ok(status)) if status.success())
}

enum BracketedPaste {
    Enabled,
    Disabled,
}

#[cfg(unix)]
enum KeyboardReporting {
    Enabled,
    Disabled,
}

struct TerminalSession {
    alternate_screen: bool,
    raw_mode: bool,
    mouse_capture: bool,
    bracketed_paste: BracketedPaste,
    #[cfg(unix)]
    keyboard_reporting: KeyboardReporting,
}

impl TerminalSession {
    fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut session = Self {
            alternate_screen: false,
            raw_mode: true,
            mouse_capture: false,
            bracketed_paste: BracketedPaste::Disabled,
            #[cfg(unix)]
            keyboard_reporting: KeyboardReporting::Disabled,
        };
        execute!(io::stdout(), EnterAlternateScreen)?;
        session.alternate_screen = true;
        session.mouse_capture = true;
        execute!(io::stdout(), EnableMouseCapture)?;
        session.bracketed_paste = BracketedPaste::Enabled;
        execute!(io::stdout(), EnableBracketedPaste)?;
        #[cfg(unix)]
        {
            session.keyboard_reporting = KeyboardReporting::Enabled;
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        Ok(session)
    }

    fn restore(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        let keyboard_result = if matches!(self.keyboard_reporting, KeyboardReporting::Enabled) {
            let result = execute!(io::stdout(), PopKeyboardEnhancementFlags);
            if result.is_ok() {
                self.keyboard_reporting = KeyboardReporting::Disabled;
            }
            result
        } else {
            Ok(())
        };
        #[cfg(not(unix))]
        let keyboard_result: io::Result<()> = Ok(());
        let paste_result = if matches!(self.bracketed_paste, BracketedPaste::Enabled) {
            let result = execute!(io::stdout(), DisableBracketedPaste);
            if result.is_ok() {
                self.bracketed_paste = BracketedPaste::Disabled;
            }
            result
        } else {
            Ok(())
        };
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
        keyboard_result
            .and(paste_result)
            .and(mouse_result)
            .and(screen_result)
            .and(raw_result)
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
    formatted_messages: FormattedMessages,
    state: InterfaceState,
    clipboard_command: Option<Vec<String>>,
}

struct InterfaceState {
    input: Input,
    scroll_from_bottom: usize,
    maximum_scroll: usize,
    selection: Option<ScreenSelection>,
    screen_area: Rect,
}

#[derive(Default)]
struct FormattedMessages {
    width: usize,
    messages: Vec<FormattedMessage>,
}

struct FormattedMessage {
    message: Message,
    tool_results: Vec<Message>,
    lines: Vec<Line<'static>>,
}

#[doc = "# Errors\nReturns an error if the terminal cannot be initialized."]
pub fn build_interface<BackendType: Backend, EventSource>(
    backend: BackendType,
    events: EventSource,
    terminal_mode: TerminalMode,
    clipboard_command: Option<Vec<String>>,
) -> Result<Interface<BackendType, EventSource>, ApplicationError>
where
    BackendType::Error: Send + Sync + 'static,
{
    let terminal_session = match terminal_mode {
        TerminalMode::Managed => Some(TerminalSession::start().map_err(|error| {
            ApplicationError::Terminal(format!("Failed to initialize terminal: {error}"))
        })?),
        TerminalMode::Unmanaged => None,
    };
    let terminal = Terminal::new(backend).map_err(|error| {
        ApplicationError::Terminal(format!("Failed to initialize terminal: {error}"))
    })?;
    Ok(Interface {
        terminal,
        events,
        terminal_session,
        formatted_messages: FormattedMessages::default(),
        state: InterfaceState {
            input: Input::default(),
            scroll_from_bottom: usize::MAX,
            maximum_scroll: 0,
            selection: None,
            screen_area: Rect::default(),
        },
        clipboard_command,
    })
}

pub(crate) fn restore_terminal<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) -> Result<(), ApplicationError> {
    interface
        .terminal_session
        .as_mut()
        .map_or(Ok(()), TerminalSession::restore)
        .map_err(|error| ApplicationError::Terminal(format!("Failed to restore terminal: {error}")))
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum SelectionRow {
    Top,
    Separator,
    Conversation(usize),
    Panel(u16),
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct SelectionPosition {
    row: SelectionRow,
    column: u16,
}

struct ScreenSelection {
    anchor: SelectionPosition,
    focus: SelectionPosition,
    dragging: bool,
    drag_pointer: Option<MouseEvent>,
    selected_content: std::collections::BTreeMap<SelectionPosition, String>,
    copy_pending: bool,
}

pub(crate) fn draw<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
    session: &Session,
    error: Option<&ApplicationError>,
    response_started: Option<tokio::time::Instant>,
) -> Result<(), ApplicationError>
where
    BackendType::Error: Send + Sync + 'static,
{
    let state = &mut interface.state;
    interface
        .terminal
        .draw(|frame| {
            if state.screen_area != frame.area() {
                state.selection = None;
                state.screen_area = frame.area();
            }
            draw_interface(
                frame,
                state,
                &mut interface.formatted_messages,
                session,
                error,
                response_started,
            );
            let input_height = state
                .input
                .get_height(frame.area().width, frame.area().height);
            let conversation_height = frame
                .area()
                .height
                .saturating_sub(input_height.saturating_add(5));
            draw_selection(
                frame,
                &mut state.selection,
                state
                    .maximum_scroll
                    .saturating_sub(state.scroll_from_bottom),
                conversation_height,
            );
        })
        .map_err(|error| ApplicationError::Terminal(format!("Terminal failed: {error}")))?;
    if let Some(selection) = state
        .selection
        .as_mut()
        .filter(|selection| selection.copy_pending)
    {
        selection.copy_pending = false;
        let text = get_selected_text(selection);
        if !text.is_empty() {
            let clipboard_command = interface.clipboard_command.clone();
            tokio::spawn(async move {
                copy_to_clipboard(&text, clipboard_command.as_deref()).await;
            });
        }
    }
    Ok(())
}

pub(crate) fn take_input<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) -> Option<String> {
    (!interface.state.input.is_empty()).then(|| interface.state.input.take())
}

pub(crate) fn reset_view<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
    new_session: bool,
) {
    interface.state.selection = None;
    if new_session {
        interface.state.scroll_from_bottom = usize::MAX;
    }
}

pub(crate) fn finish_login<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) {
    interface.state.input.clear();
    interface.state.selection = None;
    interface.state.scroll_from_bottom = 0;
}

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Exit,
    Submit,
    Abort,
    SwitchContext { new_session: bool },
    SwitchModel,
}

pub(crate) enum InterfaceInput {
    Key(KeyEvent),
    Paste(String),
    Mouse(MouseEvent),
    Exit,
    Redraw,
}

pub(crate) async fn wait_for_input<BackendType, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) -> Result<InterfaceInput, ApplicationError>
where
    BackendType: Backend,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    match interface.events.next().await {
        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
            Ok(InterfaceInput::Key(key))
        }
        Some(Ok(Event::Mouse(mouse))) => Ok(InterfaceInput::Mouse(mouse)),
        Some(Ok(Event::Paste(text))) => Ok(InterfaceInput::Paste(text)),
        Some(Ok(_)) => Ok(InterfaceInput::Redraw),
        Some(Err(error)) => Err(ApplicationError::Terminal(format!(
            "Terminal failed: {error}"
        ))),
        None => Ok(InterfaceInput::Exit),
    }
}

pub(crate) fn handle_input<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
    input: InterfaceInput,
    messages: &[Message],
) -> Option<Action> {
    let state = &mut interface.state;
    match input {
        InterfaceInput::Exit => Some(Action::Exit),
        InterfaceInput::Key(key)
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            Some(Action::Exit)
        }
        InterfaceInput::Key(key) => handle_key(state, key, messages),
        InterfaceInput::Paste(text) => {
            state.input.insert(&text);
            None
        }
        InterfaceInput::Mouse(mouse) => {
            match mouse.kind {
                MouseEventKind::ScrollUp => scroll_up(state, 3),
                MouseEventKind::ScrollDown => scroll_down(state, 3),
                MouseEventKind::Down(MouseButton::Left) => begin_selection(state, mouse),
                MouseEventKind::Drag(MouseButton::Left) => update_selection(state, mouse, true),
                MouseEventKind::Up(MouseButton::Left) => update_selection(state, mouse, false),
                _ => {}
            }
            None
        }
        InterfaceInput::Redraw => None,
    }
}

fn handle_key(state: &mut InterfaceState, key: KeyEvent, messages: &[Message]) -> Option<Action> {
    match key.code {
        KeyCode::Char(character)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            state.input.insert(&character.to_string());
        }
        KeyCode::Backspace => state.input.delete_previous(),
        KeyCode::Left => state.input.move_left(),
        KeyCode::Right => state.input.move_right(),
        KeyCode::Up | KeyCode::Down => {
            let history = messages
                .iter()
                .filter_map(|message| {
                    if let Message::User(text) = message {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            state.input.move_vertical(
                key.code == KeyCode::Up,
                usize::from(state.screen_area.width),
                &history,
            );
        }
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => state.input.insert("\n"),
        KeyCode::Esc => return Some(Action::Abort),
        KeyCode::Tab => {
            return Some(Action::SwitchContext {
                new_session: key.modifiers.contains(KeyModifiers::CONTROL),
            });
        }
        KeyCode::Enter => return Some(Action::Submit),
        KeyCode::Char(' ') if key.modifiers == KeyModifiers::CONTROL => {
            return Some(Action::SwitchModel);
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.scroll_from_bottom = 0;
        }
        KeyCode::PageUp => scroll_up(state, 8),
        KeyCode::PageDown => scroll_down(state, 8),
        _ => {}
    }
    None
}

fn get_selection_position(state: &InterfaceState, mouse: MouseEvent) -> Option<SelectionPosition> {
    let area = state.screen_area;
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let row = mouse.row.clamp(area.y, area.bottom() - 1) - area.y;
    let input_height = state.input.get_height(area.width, area.height);
    let conversation_height = area.height.saturating_sub(input_height.saturating_add(5));
    let selection_row = if row == 0 {
        SelectionRow::Top
    } else if row == 1 {
        SelectionRow::Separator
    } else if row <= conversation_height + 1 {
        let viewport_start = state
            .maximum_scroll
            .saturating_sub(state.scroll_from_bottom);
        SelectionRow::Conversation(viewport_start + usize::from(row - 2))
    } else {
        SelectionRow::Panel(row - conversation_height - 2)
    };
    Some(SelectionPosition {
        row: selection_row,
        column: mouse.column.clamp(area.x, area.right() - 1) - area.x,
    })
}

fn begin_selection(state: &mut InterfaceState, mouse: MouseEvent) {
    state.selection = get_selection_position(state, mouse).map(|position| ScreenSelection {
        anchor: position,
        focus: position,
        dragging: true,
        drag_pointer: None,
        selected_content: std::collections::BTreeMap::new(),
        copy_pending: false,
    });
}

fn update_selection(state: &mut InterfaceState, mouse: MouseEvent, dragging: bool) {
    let Some(position) = get_selection_position(state, mouse) else {
        return;
    };
    let Some(selection) = state
        .selection
        .as_mut()
        .filter(|selection| selection.dragging)
    else {
        return;
    };
    selection.focus = position;
    selection.dragging = dragging;
    selection.copy_pending = !dragging && selection.anchor != selection.focus;
    selection.drag_pointer = dragging.then_some(mouse);
    let start = selection.anchor.min(selection.focus);
    let end = selection.anchor.max(selection.focus);
    selection
        .selected_content
        .retain(|position, _| *position >= start && *position <= end);
}

fn get_selected_text(selection: &ScreenSelection) -> String {
    let mut lines = Vec::new();
    let mut current_row = None;
    let mut line = String::new();
    let mut next_column = 0;
    for (position, content) in &selection.selected_content {
        if current_row != Some(position.row) {
            if current_row.is_some() {
                lines.push(std::mem::take(&mut line));
            }
            current_row = Some(position.row);
            next_column = 0;
        }
        if position.column < next_column {
            continue;
        }
        line.push_str(content);
        let width = u16::try_from(content.width()).unwrap_or(u16::MAX).max(1);
        next_column = position.column.saturating_add(width);
    }
    if current_row.is_some() {
        lines.push(line);
    }
    lines.join("\n")
}

pub(crate) fn is_selection_dragging<BackendType: Backend, EventSource>(
    interface: &Interface<BackendType, EventSource>,
) -> bool {
    let state = &interface.state;
    state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.drag_pointer.is_some())
}

pub(crate) fn advance_selection_scroll<BackendType: Backend, EventSource>(
    interface: &mut Interface<BackendType, EventSource>,
) {
    let state = &mut interface.state;
    let Some(mouse) = state
        .selection
        .as_ref()
        .and_then(|selection| selection.drag_pointer)
    else {
        return;
    };
    let area = state.screen_area;
    let input_height = state.input.get_height(area.width, area.height);
    let conversation_height = area.height.saturating_sub(input_height.saturating_add(5));
    if conversation_height == 0 {
        return;
    }
    if mouse.row <= area.y + 2 {
        scroll_up(state, 1);
    } else if mouse.row == area.y + conversation_height + 1 {
        scroll_down(state, 1);
    } else {
        return;
    }
    update_selection(state, mouse, true);
}

fn draw_selection(
    frame: &mut Frame<'_>,
    selection: &mut Option<ScreenSelection>,
    viewport_start: usize,
    conversation_height: u16,
) {
    let Some(active_selection) = selection.as_mut() else {
        return;
    };
    if active_selection.anchor == active_selection.focus {
        return;
    }
    let start = active_selection.anchor.min(active_selection.focus);
    let end = active_selection.anchor.max(active_selection.focus);
    let area = frame.area();
    let buffer = frame.buffer_mut();
    let mut positions = Vec::new();
    for row in 0..area.height {
        let selection_row = if row == 0 {
            SelectionRow::Top
        } else if row == 1 {
            SelectionRow::Separator
        } else if row <= conversation_height + 1 {
            SelectionRow::Conversation(viewport_start + usize::from(row - 2))
        } else {
            SelectionRow::Panel(row - conversation_height - 2)
        };
        for column in 0..area.width {
            let position = SelectionPosition {
                row: selection_row,
                column,
            };
            let cell = &buffer[(area.x + column, area.y + row)];
            let character_width = u16::try_from(cell.symbol().width())
                .unwrap_or(u16::MAX)
                .max(1);
            let last_character_cell = SelectionPosition {
                row: selection_row,
                column: column.saturating_add(character_width - 1),
            };
            if last_character_cell < start || position > end {
                continue;
            }
            if active_selection
                .selected_content
                .get(&position)
                .is_some_and(|previous| previous != cell.symbol())
            {
                *selection = None;
                return;
            }
            active_selection
                .selected_content
                .insert(position, cell.symbol().to_owned());
            positions.push((area.x + column, area.y + row));
        }
    }
    for position in positions {
        buffer[position].set_fg(Color::from(Palette::Background));
        buffer[position].set_bg(Color::from(Palette::Yellow));
    }
}

fn scroll_up(state: &mut InterfaceState, lines: usize) {
    state.scroll_from_bottom = state
        .scroll_from_bottom
        .saturating_add(lines)
        .min(state.maximum_scroll);
}

fn scroll_down(state: &mut InterfaceState, lines: usize) {
    state.scroll_from_bottom = state
        .scroll_from_bottom
        .min(state.maximum_scroll)
        .saturating_sub(lines);
}

fn format_model_message(content: &str, thinking: &str) -> Vec<(String, Style)> {
    let mut sections = Vec::new();
    if !thinking.is_empty() {
        sections.push((
            thinking.trim_end_matches('\n').to_owned(),
            Style::default().fg(Color::from(Palette::MutedText)),
        ));
    }
    if !content.is_empty() {
        sections.push((content.to_owned(), Style::default()));
    }
    sections
}

fn wrap_message_section(content: &str, style: Style, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for line in content.split('\n') {
        let wrapped_lines = if line.is_empty() {
            vec![std::borrow::Cow::Borrowed("")]
        } else {
            let options =
                textwrap::Options::new(width).word_splitter(textwrap::WordSplitter::NoHyphenation);
            textwrap::wrap(line, options)
        };
        for wrapped_line in wrapped_lines {
            lines.push(Line::from(wrapped_line.into_owned()).style(style));
        }
    }
    lines
}

fn format_bash_command(command: &str, background: Color, width: usize) -> Vec<Line<'static>> {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let syntax = syntaxes
        .find_syntax_by_extension("sh")
        .expect("Bash syntax is available");
    let mut highlighter = HighlightLines::new(syntax, get_syntax_theme());
    let mut lines = Vec::new();
    let mut spans = vec![Span::styled("$ ", Style::default().bg(background))];
    let mut used_width = 2;
    for line in command.split_inclusive('\n') {
        let segments = highlighter
            .highlight_line(line, syntaxes)
            .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
        for (style, text) in segments {
            let foreground = if background == Color::from(Palette::Error) {
                Color::from(Palette::Background)
            } else {
                Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)
            };
            let mut fragment = String::new();
            for character in text.chars() {
                let character_width = character.width().unwrap_or(0);
                if character == '\n' || (used_width + character_width > width && used_width > 0) {
                    if !fragment.is_empty() {
                        spans.push(Span::styled(
                            std::mem::take(&mut fragment),
                            Style::default().fg(foreground).bg(background),
                        ));
                    }
                    lines.push(
                        Line::from(std::mem::take(&mut spans))
                            .style(Style::default().bg(background)),
                    );
                    used_width = 0;
                    if character == '\n' {
                        continue;
                    }
                }
                fragment.push(character);
                used_width += character_width;
            }
            if !fragment.is_empty() {
                spans.push(Span::styled(
                    fragment,
                    Style::default().fg(foreground).bg(background),
                ));
            }
        }
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans).style(Style::default().bg(background)));
    }
    lines
}

fn format_read_result(
    content: &str,
    path: &str,
    offset: usize,
    preceding_content: &str,
    width: usize,
) -> Vec<Line<'static>> {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let syntax = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| syntaxes.find_syntax_by_extension(extension))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, get_syntax_theme());
    for line in preceding_content.split_inclusive('\n') {
        let _ = highlighter.highlight_line(line, syntaxes);
    }
    let line_count = content.lines().count();
    let number_width = (offset + line_count.saturating_sub(1)).to_string().len();
    let background = Color::from(Palette::Background);
    let mut output = Vec::new();
    for (position, line) in content.split_inclusive('\n').enumerate() {
        let prefix = format!("{:>number_width$}  ", offset + position);
        let mut spans = vec![Span::styled(
            prefix.clone(),
            Style::default()
                .fg(Color::from(Palette::SecondaryText))
                .bg(background),
        )];
        let mut used_width = prefix.width();
        let styled_segments = highlighter
            .highlight_line(line, syntaxes)
            .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
        for (style, text) in styled_segments {
            let foreground = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            let mut fragment = String::new();
            for character in text.trim_end_matches(['\r', '\n']).chars() {
                let character_width = character.width().unwrap_or(0);
                if used_width + character_width > width && used_width > prefix.width() {
                    if !fragment.is_empty() {
                        spans.push(Span::styled(
                            std::mem::take(&mut fragment),
                            Style::default().fg(foreground).bg(background),
                        ));
                    }
                    output.push(
                        Line::from(std::mem::take(&mut spans))
                            .style(Style::default().bg(background)),
                    );
                    spans.push(Span::styled(
                        " ".repeat(prefix.width()),
                        Style::default().bg(background),
                    ));
                    used_width = prefix.width();
                }
                fragment.push(character);
                used_width += character_width;
            }
            if !fragment.is_empty() {
                spans.push(Span::styled(
                    fragment,
                    Style::default().fg(foreground).bg(background),
                ));
            }
        }
        output.push(Line::from(spans).style(Style::default().bg(background)));
    }
    output
}

fn get_tool_call_background(call: &ToolCall, messages: &[Message]) -> Color {
    messages
        .iter()
        .find_map(|message| match message {
            Message::ToolResult { call_id, result } if call_id == &call.id => {
                Some(get_tool_background(result.is_ok()))
            }
            _ => None,
        })
        .unwrap_or_else(|| Color::from(Palette::Background))
}

fn format_tool_calls(
    tool_calls: &[ToolCall],
    messages: &[Message],
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for call in tool_calls {
        let background = get_tool_call_background(call, messages);
        let command = if call.name == "bash" {
            serde_json::from_str::<serde_json::Value>(&call.arguments)
                .ok()
                .and_then(|arguments| arguments["command"].as_str().map(str::to_owned))
        } else {
            None
        };
        if let Some(command) = command {
            lines.extend(format_bash_command(&command, background, width));
        } else {
            let path = matches!(call.name.as_str(), "edit" | "read" | "ls" | "write")
                .then(|| serde_json::from_str::<serde_json::Value>(&call.arguments).ok())
                .flatten()
                .and_then(|arguments| arguments["path"].as_str().map(str::to_owned));
            let content = format!(
                "{} {}",
                call.name,
                path.as_deref().unwrap_or(&call.arguments)
            );
            let foreground = if background == Color::from(Palette::Error) {
                Color::from(Palette::Background)
            } else {
                Color::from(Palette::Text)
            };
            lines.extend(wrap_message_section(
                &content,
                Style::default().fg(foreground).bg(background),
                width,
            ));
        }
        if let Some(result) = messages.iter().find(
            |message| matches!(message, Message::ToolResult { call_id, .. } if call_id == &call.id),
        ) {
            lines.push(Line::default());
            lines.extend(format_messages(std::slice::from_ref(result), &[], width));
        }
    }
    lines
}

fn format_conversation_section(
    content: &str,
    style: Style,
    user: bool,
    message: &Message,
    width: usize,
) -> Vec<Line<'static>> {
    let markdown_message = user || matches!(message, Message::System(_) | Message::Model { .. });
    if markdown_message {
        let message_style = if user || matches!(message, Message::System(_)) {
            style.fg(Color::from(Palette::Yellow))
        } else {
            style
        };
        markdown::format_markdown(content, message_style, width)
    } else {
        wrap_message_section(content, style, width)
    }
}

fn format_file_result(message: &Message, width: usize) -> Option<Vec<Line<'static>>> {
    match message {
        Message::ToolResult {
            result:
                Ok(ToolOutput::ReadText {
                    path,
                    content,
                    offset,
                    preceding_content,
                }),
            ..
        } => Some(format_read_result(
            content,
            path,
            *offset,
            preceding_content,
            width,
        )),
        Message::ToolResult {
            result: Ok(ToolOutput::ReadImage { path, content, .. }),
            ..
        } => Some(format_read_result(content, path, 1, "", width)),
        Message::ToolResult {
            result: Ok(ToolOutput::Write {
                path, file_content, ..
            }),
            ..
        } => Some(format_read_result(file_content, path, 1, "", width)),
        _ => None,
    }
}

fn get_tool_background(successful: bool) -> Color {
    Color::from(if successful {
        Palette::Background
    } else {
        Palette::Error
    })
}

fn get_tool_result_style(successful: bool) -> Style {
    let background = get_tool_background(successful);
    let foreground = if successful {
        Color::from(Palette::Text)
    } else {
        Color::from(Palette::Background)
    };
    Style::default().fg(foreground).bg(background)
}

fn format_messages(
    messages: &[Message],
    conversation: &[Message],
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for message in messages {
        if let Message::ToolResult { call_id, .. } = message
            && conversation.iter().any(|candidate| matches!(candidate, Message::Model { tool_calls, .. } if tool_calls.iter().any(|call| &call.id == call_id)))
        {
            continue;
        }
        if let Message::ToolResult {
            result:
                Ok(ToolOutput::Edit {
                    path,
                    original,
                    updated,
                    ..
                }),
            ..
        } = message
        {
            lines.extend(edit::format_edit_result(path, original, updated, width));
            lines.push(Line::default());
            continue;
        }
        if let Some(result) = format_file_result(message, width) {
            lines.extend(result);
            lines.push(Line::default());
            continue;
        }
        let (sections, user, aborted) = match message {
            Message::System(content) => (
                vec![(content.trim_end_matches('\n').to_owned(), Style::default())],
                false,
                false,
            ),
            Message::User(content) => (vec![(content.clone(), Style::default())], true, false),
            Message::FailedResponse(content) => (
                vec![(
                    content.clone(),
                    Style::default().fg(Color::from(Palette::Error)),
                )],
                false,
                false,
            ),
            Message::ToolResult { result, .. } => {
                let content = match result {
                    Ok(output) => output.content(),
                    Err(error) => error,
                };
                (
                    vec![(
                        content.trim_end_matches('\n').to_owned(),
                        get_tool_result_style(result.is_ok()),
                    )],
                    false,
                    false,
                )
            }
            Message::Model {
                content,
                thinking,
                aborted,
                ..
            } => (format_model_message(content, thinking), false, *aborted),
        };
        for (position, (content, style)) in sections.into_iter().enumerate() {
            if position > 0 {
                lines.push(Line::default());
            }
            lines.extend(format_conversation_section(
                &content, style, user, message, width,
            ));
        }
        if let Message::Model { tool_calls, .. } = message {
            if !tool_calls.is_empty() && !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.extend(format_tool_calls(tool_calls, conversation, width));
        }
        if aborted {
            for line in textwrap::wrap("Request was aborted", width) {
                lines.push(
                    Line::from(line.into_owned())
                        .style(Style::default().fg(Color::from(Palette::Error))),
                );
            }
        }
        if lines.last().is_some_and(|line| !line.spans.is_empty()) {
            lines.push(Line::default());
        }
    }
    for line in &mut lines {
        if line.style.bg.is_some() {
            let padding = width.saturating_sub(line.width());
            line.spans.push(Span::raw(" ".repeat(padding)));
        }
    }
    lines
}

impl FormattedMessages {
    fn update(&mut self, messages: &[Message], width: usize) {
        if self.width != width {
            self.messages.clear();
            self.width = width;
        }
        self.messages.truncate(messages.len());
        for (position, message) in messages.iter().enumerate() {
            let tool_results = match message {
                Message::Model { tool_calls, .. } => messages
                    .iter()
                    .filter(|candidate| matches!(candidate, Message::ToolResult { call_id, .. } if tool_calls.iter().any(|call| &call.id == call_id)))
                    .cloned()
                    .collect(),
                _ => Vec::new(),
            };
            if self.messages.get(position).is_some_and(|formatted| {
                formatted.message == *message && formatted.tool_results == tool_results
            }) {
                continue;
            }
            let formatted = FormattedMessage {
                message: message.clone(),
                tool_results,
                lines: format_messages(std::slice::from_ref(message), messages, width),
            };
            if position == self.messages.len() {
                self.messages.push(formatted);
            } else {
                self.messages[position] = formatted;
            }
        }
    }
}

fn draw_interface(
    frame: &mut Frame<'_>,
    state: &mut InterfaceState,
    formatted_messages: &mut FormattedMessages,
    session: &Session,
    error: Option<&ApplicationError>,
    response_started: Option<tokio::time::Instant>,
) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .fg(Color::from(Palette::Text))
                .bg(Color::from(Palette::Background)),
        ),
        frame.area(),
    );
    let input_height = state
        .input
        .get_height(frame.area().width, frame.area().height);
    let statistics = &session.statistics;
    let scroll_from_bottom = &mut state.scroll_from_bottom;
    let maximum_scroll = &mut state.maximum_scroll;
    let panels = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(frame.area());
    frame.render_widget(
        Paragraph::new(session.get_context_name())
            .style(Style::default().fg(Color::from(Palette::PanelText))),
        panels[0],
    );
    let areas = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(input_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(panels[2]);
    let width = usize::from(areas[0].width).max(1);
    formatted_messages.update(&session.messages, width);
    let error_lines = error.map_or_else(Vec::new, |error| {
        textwrap::wrap(&format!("Request failed: {error}"), width)
            .into_iter()
            .map(|line| {
                Line::from(line.into_owned())
                    .style(Style::default().fg(Color::from(Palette::Error)))
            })
            .collect::<Vec<_>>()
    });
    let line_count = formatted_messages
        .messages
        .iter()
        .map(|message| message.lines.len())
        .sum::<usize>()
        + error_lines.len();
    let height = usize::from(areas[0].height);
    let previous_maximum_scroll = *maximum_scroll;
    *maximum_scroll = line_count.saturating_sub(height);
    if *scroll_from_bottom > 0 && *scroll_from_bottom != usize::MAX {
        *scroll_from_bottom = scroll_from_bottom
            .saturating_add(maximum_scroll.saturating_sub(previous_maximum_scroll))
            .saturating_sub(previous_maximum_scroll.saturating_sub(*maximum_scroll));
    }
    *scroll_from_bottom = (*scroll_from_bottom).min(*maximum_scroll);
    let start = maximum_scroll.saturating_sub(*scroll_from_bottom);
    let visible = formatted_messages
        .messages
        .iter()
        .flat_map(|message| message.lines.iter())
        .chain(error_lines.iter())
        .skip(start)
        .take(height)
        .cloned()
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible), areas[0]);
    for area in [panels[1], areas[1], areas[3]] {
        let line = "─".repeat(usize::from(area.width));
        frame.render_widget(
            Paragraph::new(line).style(Style::default().fg(Color::from(Palette::MutedText))),
            area,
        );
    }
    state.input.draw(frame, areas[2]);
    draw_status(frame, areas[4], statistics, response_started, session);
}

fn draw_status(
    frame: &mut Frame<'_>,
    area: Rect,
    statistics: &Statistics,
    response_started: Option<tokio::time::Instant>,
    session: &crate::session::Session,
) {
    let sent = statistics
        .prompt_tokens
        .saturating_sub(statistics.cached_tokens);
    let prompt_tokens = u128::from(statistics.prompt_tokens);
    let cache_percentage = (u128::from(statistics.cached_tokens) * 100 + prompt_tokens / 2)
        .checked_div(prompt_tokens)
        .unwrap_or(0);
    let response_status = if let Some(started) = response_started {
        let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame_duration = 80;
        let frame_count = spinner_frames.len() as u128;
        let frame_index =
            usize::try_from(started.elapsed().as_millis() / frame_duration % frame_count)
                .expect("the spinner frame index fits in usize");
        Some(format!(
            "{} {}",
            spinner_frames[frame_index],
            format_response_time(started.elapsed())
        ))
    } else {
        statistics
            .response_time
            .map(|duration| format!("⏱ {}", format_response_time(duration)))
    };
    let response_prefix = response_status.map_or_else(String::new, |status| format!("{status} "));
    let left = format!(
        "{response_prefix}↑{} ↓{} CH{cache_percentage}%",
        format_count(sent),
        format_count(statistics.completion_tokens)
    );
    let model = session.get_selected_model();
    let thinking_level = model.thinking_level.map_or_else(String::new, |level| {
        format!(" {}", level.as_provider_value())
    });
    let provider = session.provider.get_name();
    let right = format!("{provider} {}{thinking_level}", model.name);
    frame.render_widget(
        Block::default().style(Style::default().fg(Color::from(Palette::PanelText))),
        area,
    );
    let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let status_areas =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    frame.render_widget(Paragraph::new(left), status_areas[0]);
    frame.render_widget(Paragraph::new(right), status_areas[1]);
}

fn format_response_time(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    let seconds_per_minute = 60;
    if seconds < seconds_per_minute {
        format!("{seconds}s")
    } else {
        let minutes = seconds / seconds_per_minute;
        let remaining_seconds = seconds % seconds_per_minute;
        format!("{minutes}m {remaining_seconds}s")
    }
}

fn format_count(count: u64) -> String {
    if count >= 1000 {
        format!("{}k", count / 1000)
    } else {
        count.to_string()
    }
}
