//! Syntax highlighting for code blocks.
//!
//! Uses syntect for highlighting with Sublime Text syntax definitions.

use std::sync::{Mutex, OnceLock};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::document::{InlineColor, InlineSpan, InlineStyle};

// TODO: Implement syntax highlighting
// - Load syntax set
// - Load theme
// - Highlight code blocks
// - Convert to ratatui spans

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_produces_colored_spans() {
        let code = "fn main() {\n    let x = 1;\n}\n";
        let lines = highlight_code(Some("rust"), code);

        assert_eq!(lines.len(), 3);
        let has_color = lines.iter().flatten().any(|span| span.style().fg.is_some());
        assert!(has_color, "Expected at least one colored span for Rust");
    }

    #[test]
    fn test_highlight_unknown_language_falls_back_to_plain() {
        let code = "just text";
        let lines = highlight_code(Some("nope"), code);

        assert_eq!(lines.len(), 1);
        let has_color = lines.iter().flatten().any(|span| span.style().fg.is_some());
        assert!(!has_color, "Unknown language should not colorize");
    }

    #[test]
    fn test_highlight_plain_code_sets_code_style() {
        let code = "plain";
        let lines = highlight_code(None, code);
        let spans = &lines[0];
        assert!(spans.iter().all(|span| span.style().code));
    }

    #[test]
    fn test_highlight_does_not_set_background_color() {
        let code = "fn main() {}";
        let lines = highlight_code(Some("rust"), code);
        let has_bg = lines.iter().flatten().any(|span| span.style().bg.is_some());
        assert!(!has_bg, "Highlighting should not override background");
    }

    #[test]
    fn test_colorfgbg_dark_background() {
        let mode = background_mode_from_colorfgbg(Some("15;0"));
        assert_eq!(mode, BackgroundMode::Dark);
    }

    #[test]
    fn test_colorfgbg_light_background() {
        let mode = background_mode_from_colorfgbg(Some("0;15"));
        assert_eq!(mode, BackgroundMode::Light);
    }

    #[test]
    fn test_background_override_light() {
        set_background_mode(Some(HighlightBackground::Light));
        assert_eq!(background_mode(), BackgroundMode::Light);
        set_background_mode(None);
    }

    #[test]
    fn test_background_override_dark() {
        set_background_mode(Some(HighlightBackground::Dark));
        assert_eq!(background_mode(), BackgroundMode::Dark);
        set_background_mode(None);
    }

    #[test]
    fn test_light_mode_darkens_bright_fg() {
        let bright = InlineColor {
            r: 240,
            g: 230,
            b: 120,
        };
        let adjusted = adjust_fg_for_background(bright, BackgroundMode::Light);
        assert!(adjusted.r < bright.r);
        assert!(adjusted.g < bright.g);
        assert!(adjusted.b < bright.b);
    }

    #[test]
    fn test_language_for_file_returns_rust_for_rs() {
        let lang = language_for_file(std::path::Path::new("foo.rs"));
        assert_eq!(lang, Some("Rust"));
    }

    #[test]
    fn test_language_for_file_returns_none_for_md() {
        let lang = language_for_file(std::path::Path::new("README.md"));
        assert!(lang.is_none(), "Markdown files should return None");
    }

    #[test]
    fn test_language_for_file_returns_none_for_unknown() {
        let lang = language_for_file(std::path::Path::new("foo.xyz"));
        assert!(lang.is_none(), "Unknown extensions should return None");
    }

    #[test]
    fn test_light_mode_caps_luma_for_readability() {
        let bright = InlineColor {
            r: 240,
            g: 230,
            b: 120,
        };
        let adjusted = adjust_fg_for_background(bright, BackgroundMode::Light);
        let luma = (0.2126 * adjusted.r as f32)
            + (0.7152 * adjusted.g as f32)
            + (0.0722 * adjusted.b as f32);
        assert!(luma < 120.0, "Adjusted color still too bright: {luma}");
    }
}

/// Returns the syntax language name for a file path, or `None` if the file
/// is markdown or has no recognized syntax.
pub fn language_for_file(path: &std::path::Path) -> Option<&'static str> {
    let ss = syntax_set();
    let syntax = ss.find_syntax_for_file(path).ok().flatten()?;
    if syntax.name == "Markdown" {
        return None;
    }
    // Leak the name so we get a 'static str — the SyntaxSet is 'static anyway.
    Some(leak_str(&syntax.name))
}

/// Cache interned strings to avoid leaking duplicates.
fn leak_str(s: &str) -> &'static str {
    use std::collections::HashSet;

    static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let lock = INTERNED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = lock.lock().expect("intern lock");
    if let Some(existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

pub fn highlight_code(language: Option<&str>, code: &str) -> Vec<Vec<InlineSpan>> {
    let mut lines = Vec::new();
    let syntax_set = syntax_set();
    let mode = background_mode();
    let syntax = language
        .and_then(|lang| syntax_set.find_syntax_by_token(lang))
        .or_else(|| language.and_then(|lang| syntax_set.find_syntax_by_name(lang)));

    let Some(syntax) = syntax else {
        for line in code.lines() {
            let mut style = InlineStyle::default();
            style.code = true;
            lines.push(vec![InlineSpan::new(line.to_string(), style)]);
        }
        return lines;
    };

    let mut highlighter = HighlightLines::new(syntax, theme());
    for line in code.lines() {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();
        let mut spans = Vec::new();
        for (style, text) in ranges {
            let mut inline_style = InlineStyle::default();
            inline_style.code = true;
            let fg = InlineColor {
                r: style.foreground.r,
                g: style.foreground.g,
                b: style.foreground.b,
            };
            inline_style.fg = Some(adjust_fg_for_background(fg, mode));
            spans.push(InlineSpan::new(text.to_string(), inline_style));
        }
        lines.push(spans);
    }

    lines
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(|| {
        let _scope = crate::perf::scope("highlight.syntax_set.load_defaults");
        SyntaxSet::load_defaults_newlines()
    })
}

fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let _scope = crate::perf::scope("highlight.theme.load_defaults");
        let theme_set = ThemeSet::load_defaults();
        let mode = background_mode();
        let preferred = match mode {
            BackgroundMode::Dark => [
                "Monokai Extended",
                "Monokai Extended Bright",
                "Dracula",
                "Solarized (dark)",
                "base16-ocean.dark",
            ]
            .as_slice(),
            BackgroundMode::Light => {
                ["InspiredGitHub", "Solarized (light)", "base16-ocean.light"].as_slice()
            }
        };

        for name in preferred {
            if let Some(theme) = theme_set.themes.get(*name) {
                return theme.clone();
            }
        }

        theme_set
            .themes
            .values()
            .next()
            .cloned()
            .unwrap_or_default()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackgroundMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightBackground {
    Light,
    Dark,
}

static BACKGROUND_OVERRIDE: OnceLock<Mutex<Option<HighlightBackground>>> = OnceLock::new();

pub fn set_background_mode(mode: Option<HighlightBackground>) {
    let lock = BACKGROUND_OVERRIDE.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().expect("highlight background lock");
    *guard = mode;
}

pub fn set_background_mode_from_rgb(r: u8, g: u8, b: u8) {
    let luma = (0.2126 * r as f32) + (0.7152 * g as f32) + (0.0722 * b as f32);
    let mode = if luma >= 140.0 {
        HighlightBackground::Light
    } else {
        HighlightBackground::Dark
    };
    set_background_mode(Some(mode));
}

fn background_mode() -> BackgroundMode {
    let lock = BACKGROUND_OVERRIDE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock() {
        if let Some(mode) = *guard {
            return match mode {
                HighlightBackground::Light => BackgroundMode::Light,
                HighlightBackground::Dark => BackgroundMode::Dark,
            };
        }
    }
    background_mode_from_colorfgbg(std::env::var("COLORFGBG").ok().as_deref())
}

pub fn is_light_background() -> bool {
    background_mode() == BackgroundMode::Light
}

fn background_mode_from_colorfgbg(colorfgbg: Option<&str>) -> BackgroundMode {
    let Some(value) = colorfgbg else {
        return BackgroundMode::Dark;
    };
    let bg_str = value.rsplit(';').next().unwrap_or(value);
    let Ok(bg) = bg_str.parse::<u8>() else {
        return BackgroundMode::Dark;
    };

    if bg >= 7 {
        BackgroundMode::Light
    } else {
        BackgroundMode::Dark
    }
}

fn adjust_fg_for_background(color: InlineColor, mode: BackgroundMode) -> InlineColor {
    match mode {
        BackgroundMode::Dark => color,
        BackgroundMode::Light => {
            let luma =
                (0.2126 * color.r as f32) + (0.7152 * color.g as f32) + (0.0722 * color.b as f32);
            if luma < 155.0 {
                return color;
            }

            InlineColor {
                r: ((color.r as f32) * 0.42).round() as u8,
                g: ((color.g as f32) * 0.42).round() as u8,
                b: ((color.b as f32) * 0.42).round() as u8,
            }
        }
    }
}
