use std::{collections::BTreeMap, ops::Range, sync::OnceLock};

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, parsing::SyntaxSet};

use super::theme::{Palette, get_syntax_theme};
use unicode_width::UnicodeWidthStr;

fn hide_delimiters(visible: &mut [bool], range: Range<usize>, length: usize) {
    if range.len() >= length * 2 {
        visible[range.start..range.start + length].fill(false);
        visible[range.end - length..range.end].fill(false);
    }
}

fn highlight_code(content: &str, range: Range<usize>, language: &str, styles: &mut [Style]) {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let Some(syntax) = syntaxes.find_syntax_by_token(language) else {
        return;
    };
    let mut highlighter = HighlightLines::new(syntax, get_syntax_theme());
    let mut position = range.start;
    for line in content[range.clone()].split_inclusive('\n') {
        if let Ok(segments) = highlighter.highlight_line(line, syntaxes) {
            for (style, segment) in segments {
                let end = (position + segment.len()).min(range.end);
                let color = style.foreground;
                for character_style in &mut styles[position..end] {
                    *character_style = character_style.fg(Color::Rgb(color.r, color.g, color.b));
                }
                position = end;
            }
        } else {
            position += line.len();
        }
    }
}

pub(super) fn format_markdown(
    content: &str,
    base_style: Style,
    width: usize,
) -> Vec<Line<'static>> {
    let mut visible = vec![true; content.len()];
    let mut styles = vec![base_style; content.len()];
    let mut replacements = BTreeMap::new();
    let mut code_language = None;
    for (event, range) in Parser::new(content).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                for style in &mut styles[range] {
                    *style = style
                        .fg(Color::from(Palette::Yellow))
                        .add_modifier(Modifier::BOLD);
                }
            }
            Event::Start(Tag::Strong) => {
                for style in &mut styles[range.clone()] {
                    *style = style.add_modifier(Modifier::BOLD);
                }
                hide_delimiters(&mut visible, range, 2);
            }
            Event::Start(Tag::Emphasis) => hide_delimiters(&mut visible, range, 1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                visible[range.clone()].fill(false);
                replacements.insert(range.start, dest_url.into_string());
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) => {
                code_language = Some(
                    language
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
            Event::End(TagEnd::CodeBlock) => code_language = None,
            Event::Text(_) if code_language.is_some() && base_style.fg.is_none() => {
                highlight_code(
                    content,
                    range,
                    code_language.as_deref().unwrap_or_default(),
                    &mut styles,
                );
            }
            Event::Code(_) => {
                let delimiter_length = content[range.clone()]
                    .bytes()
                    .take_while(|byte| *byte == b'`')
                    .count();
                hide_delimiters(&mut visible, range.clone(), delimiter_length);
                if base_style.fg.is_none() {
                    for style in &mut styles[range] {
                        *style = style.fg(Color::from(Palette::InlineCode));
                    }
                }
            }
            _ => {}
        }
    }
    let mut rendered = String::new();
    let mut rendered_styles = Vec::new();
    for (position, character) in content.char_indices() {
        if let Some(replacement) = replacements.get(&position) {
            rendered.push_str(replacement);
            rendered_styles.extend(std::iter::repeat_n(styles[position], replacement.len()));
        }
        if visible[position] {
            rendered.push(character);
            rendered_styles.extend(std::iter::repeat_n(styles[position], character.len_utf8()));
        }
    }
    let mut lines = Vec::new();
    let mut position = 0;
    for line in rendered.split('\n') {
        let options =
            textwrap::Options::new(width).word_splitter(textwrap::WordSplitter::NoHyphenation);
        let wrapped = if line.is_empty() {
            vec![std::borrow::Cow::Borrowed("")]
        } else {
            textwrap::wrap(line, options)
        };
        let mut consumed = 0;
        for part in wrapped {
            let part = part.as_ref();
            let offset = consumed + line[consumed..].find(part).unwrap_or_default();
            consumed = offset + part.len();
            let mut spans = Vec::new();
            for (character_position, character) in part.char_indices() {
                let style = rendered_styles[position + offset + character_position];
                spans.push(Span::styled(character.to_string(), style));
            }
            if base_style.bg.is_some() {
                spans.push(Span::styled(
                    " ".repeat(width.saturating_sub(part.width())),
                    base_style,
                ));
            }
            lines.push(Line::from(spans).style(base_style));
        }
        position += line.len() + 1;
    }
    lines
}
