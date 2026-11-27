use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use unicode_width::UnicodeWidthChar;

#[derive(Default)]
pub(crate) struct Input {
    text: String,
    cursor: usize,
    history_position: Option<usize>,
    draft: String,
    scroll: usize,
}

struct InputLine {
    start: usize,
    end: usize,
}

impl Input {
    pub(crate) fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    pub(crate) fn delete_previous(&mut self) {
        if let Some((position, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.text.drain(position..self.cursor);
            self.cursor = position;
        }
    }

    pub(crate) fn move_left(&mut self) {
        if let Some((position, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.cursor = position;
        }
    }

    pub(crate) fn move_right(&mut self) {
        if let Some(character) = self.text[self.cursor..].chars().next() {
            self.cursor += character.len_utf8();
        }
    }

    fn get_lines(&self, width: usize) -> Vec<InputLine> {
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut start = 0;
        let mut used_width = 0;
        for (position, character) in self.text.char_indices() {
            if character == '\n' {
                lines.push(InputLine {
                    start,
                    end: position,
                });
                start = position + character.len_utf8();
                used_width = 0;
                continue;
            }
            let character_width = character.width().unwrap_or(0);
            if used_width + character_width > width && position > start {
                lines.push(InputLine {
                    start,
                    end: position,
                });
                start = position;
                used_width = 0;
            }
            used_width += character_width;
        }
        lines.push(InputLine {
            start,
            end: self.text.len(),
        });
        lines
    }

    fn get_cursor_line(&self, lines: &[InputLine]) -> usize {
        lines
            .iter()
            .rposition(|line| line.start <= self.cursor)
            .unwrap_or(0)
    }

    pub(crate) fn move_vertical(&mut self, upward: bool, width: usize, history: &[&str]) {
        let lines = self.get_lines(width);
        let current = self.get_cursor_line(&lines);
        let target = if upward {
            current.checked_sub(1)
        } else {
            (current + 1 < lines.len()).then_some(current + 1)
        };
        if let Some(target) = target {
            let column: usize = self.text[lines[current].start..self.cursor]
                .chars()
                .map(|character| character.width().unwrap_or(0))
                .sum();
            let line = &lines[target];
            let mut used_width = 0;
            self.cursor = line.start;
            for character in self.text[line.start..line.end].chars() {
                let character_width = character.width().unwrap_or(0);
                if used_width + character_width > column {
                    break;
                }
                used_width += character_width;
                self.cursor += character.len_utf8();
            }
            return;
        }
        if upward {
            if history.is_empty() {
                return;
            }
            let position = if let Some(position) = self.history_position {
                position.saturating_sub(1).min(history.len() - 1)
            } else {
                self.draft.clone_from(&self.text);
                history.len() - 1
            };
            self.history_position = Some(position);
            history[position].clone_into(&mut self.text);
        } else if let Some(position) = self.history_position {
            if position + 1 < history.len() {
                self.history_position = Some(position + 1);
                history[position + 1].clone_into(&mut self.text);
            } else {
                self.history_position = None;
                self.text.clone_from(&self.draft);
            }
        } else {
            return;
        }
        self.cursor = self.text.len();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn take(&mut self) -> String {
        std::mem::take(self).text
    }

    pub(crate) fn get_height(&self, width: u16, terminal_height: u16) -> u16 {
        let line_count =
            u16::try_from(self.get_lines(usize::from(width)).len()).unwrap_or(u16::MAX);
        line_count.min((terminal_height / 2).max(1))
    }

    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let lines = self.get_lines(usize::from(area.width));
        let cursor_line = self.get_cursor_line(&lines);
        let height = usize::from(area.height);
        if cursor_line < self.scroll {
            self.scroll = cursor_line;
        }
        if cursor_line >= self.scroll + height {
            self.scroll = cursor_line + 1 - height;
        }
        self.scroll = self.scroll.min(lines.len().saturating_sub(height));
        let visible = lines
            .iter()
            .skip(self.scroll)
            .take(height)
            .map(|line| &self.text[line.start..line.end])
            .collect::<Vec<_>>()
            .join("\n");
        frame.render_widget(Paragraph::new(visible), area);
        let column: usize = self.text[lines[cursor_line].start..self.cursor]
            .chars()
            .map(|character| character.width().unwrap_or(0))
            .sum();
        let column = u16::try_from(column)
            .unwrap_or(u16::MAX)
            .min(area.width - 1);
        let row = u16::try_from(cursor_line - self.scroll).unwrap_or(u16::MAX);
        frame.set_cursor_position((area.x + column, area.y + row));
    }
}
