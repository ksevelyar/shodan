use std::{path::Path, sync::OnceLock};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use similar::{ChangeTag, TextDiff};
use syntect::{easy::HighlightLines, parsing::SyntaxSet};

use super::theme::{Palette, get_syntax_theme};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn highlight_source(content: &str, path: &str) -> Vec<Vec<(syntect::highlighting::Style, String)>> {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let syntax = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| syntaxes.find_syntax_by_extension(extension))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, get_syntax_theme());
    content
        .split_inclusive('\n')
        .map(|line| {
            highlighter
                .highlight_line(line, syntaxes)
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)])
                .into_iter()
                .map(|(style, text)| (style, text.to_owned()))
                .collect()
        })
        .collect()
}

fn format_diff_line(
    prefix: &str,
    segments: &[(syntect::highlighting::Style, String)],
    foreground: Color,
    background: Color,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut spans = vec![Span::styled(
        prefix.to_owned(),
        Style::default().fg(foreground).bg(background),
    )];
    let mut used_width = prefix.width();
    for (style, text) in segments {
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
                lines.push(
                    Line::from(std::mem::take(&mut spans)).style(Style::default().bg(background)),
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
    lines.push(Line::from(spans).style(Style::default().bg(background)));
    lines
}

pub(super) fn format_edit_result(
    path: &str,
    original: &str,
    updated: &str,
    width: usize,
) -> Vec<Line<'static>> {
    let diff = TextDiff::from_lines(original, updated);
    let original = highlight_source(original, path);
    let updated = highlight_source(updated, path);
    let number_width = original.len().max(updated.len()).to_string().len();
    let mut lines = Vec::new();
    let context_lines = 3;
    for (group_number, group) in diff.grouped_ops(context_lines).iter().enumerate() {
        if group_number > 0 {
            lines.push(Line::from("..."));
        }
        for operation in group {
            for change in diff.iter_changes(operation) {
                let background = Color::from(Palette::Background);
                let (marker, number, segments, foreground) = match change.tag() {
                    ChangeTag::Delete => {
                        let index = change
                            .old_index()
                            .expect("deleted lines have original positions");
                        (
                            '-',
                            index + 1,
                            &original[index],
                            Color::from(Palette::Error),
                        )
                    }
                    ChangeTag::Insert => {
                        let index = change
                            .new_index()
                            .expect("added lines have updated positions");
                        (
                            '+',
                            index + 1,
                            &updated[index],
                            Color::from(Palette::Addition),
                        )
                    }
                    ChangeTag::Equal => {
                        let index = change
                            .old_index()
                            .expect("context lines have original positions");
                        (
                            ' ',
                            index + 1,
                            &original[index],
                            Color::from(Palette::SecondaryText),
                        )
                    }
                };
                let prefix = format!("{marker} {number:>number_width$}  ");
                lines.extend(format_diff_line(
                    &prefix, segments, foreground, background, width,
                ));
            }
        }
    }
    lines
}
