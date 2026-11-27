use std::sync::OnceLock;

use ratatui::style::Color;
use syntect::highlighting::{StyleModifier, Theme, ThemeItem, ThemeSettings};

pub(super) enum Palette {
    Background,
    Text,
    SecondaryText,
    MutedText,
    PanelText,
    Yellow,
    Error,
    Addition,
    InlineCode,
}

impl From<Palette> for Color {
    fn from(color: Palette) -> Self {
        match color {
            Palette::Background => Self::Rgb(27, 22, 31),
            Palette::Text => Self::Rgb(200, 214, 243),
            Palette::SecondaryText => Self::Rgb(145, 159, 186),
            Palette::MutedText => Self::Rgb(94, 107, 133),
            Palette::PanelText => Self::Rgb(129, 124, 134),
            Palette::Yellow => Self::Rgb(213, 170, 42),
            Palette::Error => Self::Rgb(200, 78, 78),
            Palette::Addition => Self::Rgb(87, 161, 109),
            Palette::InlineCode => Self::Rgb(84, 198, 181),
        }
    }
}

fn get_syntax_color(color: Color) -> syntect::highlighting::Color {
    let Color::Rgb(red, green, blue) = color else {
        unreachable!("Joker colors are RGB")
    };
    syntect::highlighting::Color {
        r: red,
        g: green,
        b: blue,
        a: 255,
    }
}

pub(super) fn get_syntax_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let colors = [
            ("comment", Color::from(Palette::MutedText)),
            ("entity.name.function", Color::from(Palette::Addition)),
            (
                "keyword, storage, storage.type.function",
                Color::from(Palette::Error),
            ),
            ("string", Color::Rgb(89, 122, 144)),
            (
                "entity.name.type, support.type, storage.type",
                Color::from(Palette::Yellow),
            ),
            (
                "constant.numeric, constant.character, constant.language",
                Color::Rgb(255, 206, 80),
            ),
            ("variable", Color::from(Palette::InlineCode)),
            ("entity.name, support.function", Color::Rgb(193, 111, 180)),
        ];
        let scopes = colors
            .into_iter()
            .map(|(selector, color)| ThemeItem {
                scope: selector.parse().expect("the syntax selector is valid"),
                style: StyleModifier {
                    foreground: Some(get_syntax_color(color)),
                    ..StyleModifier::default()
                },
            })
            .collect();
        Theme {
            settings: ThemeSettings {
                foreground: Some(get_syntax_color(Color::from(Palette::Text))),
                background: Some(get_syntax_color(Color::from(Palette::Background))),
                ..ThemeSettings::default()
            },
            scopes,
            ..Theme::default()
        }
    })
}
