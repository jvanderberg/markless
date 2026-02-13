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
    fn test_highlight_continues_after_line_comment() {
        // Regression: highlighting stopped after the first // comment because
        // syntect's newlines-mode parser never saw the \n that ends the
        // comment scope, leaving all subsequent lines inside the comment.
        let code = "#include <stdio.h>\n// comment\nint main() {}\n";
        let lines = highlight_code(Some("c"), code);

        assert_eq!(lines.len(), 3, "expected 3 lines");
        // The third line ("int main() {}") must have coloured spans,
        // not plain comment grey.
        let has_color_after_comment = lines[2].iter().any(|span| {
            let fg = span.style().fg;
            // A real keyword like "int" should have a distinct colour;
            // if everything is the same grey as the comment, the bug is back.
            fg.is_some()
                && fg
                    != lines[1]
                        .first()
                        .and_then(|s| s.style().fg)
                        .as_ref()
                        .copied()
        });
        assert!(
            has_color_after_comment,
            "Code after a // comment must have different highlighting than the comment itself.\n\
             Line 2 (comment) spans: {:?}\nLine 3 (code) spans: {:?}",
            lines[1], lines[2]
        );
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
    fn test_dark_mode_code_spans_are_bright_enough() {
        // Syntax-highlighted code on a dark background should be compressed
        // into a comfortable range: dim colors boosted, bright colors tamed.
        set_background_mode(Some(HighlightBackground::Dark));
        let code = "fn main() {\n    let x = 42;\n    println!(\"hello\");\n}\n";
        let lines = highlight_code(Some("rust"), code);
        set_background_mode(None);

        let lumas: Vec<f32> = lines
            .iter()
            .flatten()
            .filter_map(|span| {
                let fg = span.style().fg?;
                Some(
                    0.2126f32 * f32::from(fg.r)
                        + 0.7152 * f32::from(fg.g)
                        + 0.0722 * f32::from(fg.b),
                )
            })
            .collect();
        assert!(!lumas.is_empty(), "Should have colored spans");

        let min_luma = lumas.iter().copied().fold(f32::INFINITY, f32::min);
        let max_luma = lumas.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        assert!(
            min_luma >= 145.0,
            "Darkest syntax color has luma {min_luma:.1}, too faint (need >= 145)"
        );
        assert!(
            max_luma <= 210.0,
            "Brightest syntax color has luma {max_luma:.1}, too hot (need <= 210)"
        );
    }

    #[test]
    fn test_dark_mode_boosts_dim_fg() {
        let dim = InlineColor {
            r: 101,
            g: 123,
            b: 131,
        };
        let adjusted = adjust_fg_for_background(dim, BackgroundMode::Dark);
        let luma =
            0.2126 * adjusted.r as f32 + 0.7152 * adjusted.g as f32 + 0.0722 * adjusted.b as f32;
        assert!(
            luma >= 145.0,
            "Boosted color luma {luma:.1} should be >= 145"
        );
        // Hue should be preserved: ratios between channels stay similar
        let orig_ratio = dim.r as f32 / dim.g as f32;
        let adj_ratio = adjusted.r as f32 / adjusted.g as f32;
        assert!(
            (orig_ratio - adj_ratio).abs() < 0.05,
            "Hue should be preserved: orig r/g={orig_ratio:.3} adj r/g={adj_ratio:.3}"
        );
    }

    #[test]
    fn test_dark_mode_compresses_bright_colors() {
        let bright = InlineColor {
            r: 200,
            g: 180,
            b: 160,
        };
        let adjusted = adjust_fg_for_background(bright, BackgroundMode::Dark);
        let orig_luma =
            0.2126 * bright.r as f32 + 0.7152 * bright.g as f32 + 0.0722 * bright.b as f32;
        let adj_luma =
            0.2126 * adjusted.r as f32 + 0.7152 * adjusted.g as f32 + 0.0722 * adjusted.b as f32;
        assert!(
            adj_luma < orig_luma,
            "Bright color luma {adj_luma:.1} should be reduced from {orig_luma:.1}"
        );
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
    let mut set = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            let style = InlineStyle {
                code: true,
                ..InlineStyle::default()
            };
            lines.push(vec![InlineSpan::new(line.to_string(), style)]);
        }
        return lines;
    };

    let mut highlighter = HighlightLines::new(syntax, theme());
    for line in code.lines() {
        // SyntaxSet::load_defaults_newlines() expects each line to end
        // with '\n'.  str::lines() strips newlines, so we must re-add it;
        // otherwise the parser never closes scopes that terminate at EOL
        // (e.g. `//` comments), and the unclosed scope bleeds into every
        // subsequent line.
        let line_nl = format!("{line}\n");
        let ranges = highlighter
            .highlight_line(&line_nl, syntax_set)
            .unwrap_or_default();
        let mut spans = Vec::new();
        for (style, text) in ranges {
            // Strip the trailing '\n' we added — it must not appear in the
            // rendered output.
            let text = text.trim_end_matches('\n');
            if text.is_empty() {
                continue;
            }
            let mut inline_style = InlineStyle {
                code: true,
                ..InlineStyle::default()
            };
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

/// Sets the background mode override for syntax highlighting.
///
pub fn set_background_mode(mode: Option<HighlightBackground>) {
    let lock = BACKGROUND_OVERRIDE.get_or_init(|| Mutex::new(None));
    let mut guard = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = mode;
}

pub fn set_background_mode_from_rgb(r: u8, g: u8, b: u8) {
    let luma = 0.2126f32.mul_add(
        f32::from(r),
        0.7152f32.mul_add(f32::from(g), 0.0722 * f32::from(b)),
    );
    let mode = if luma >= 140.0 {
        HighlightBackground::Light
    } else {
        HighlightBackground::Dark
    };
    set_background_mode(Some(mode));
}

fn background_mode() -> BackgroundMode {
    let lock = BACKGROUND_OVERRIDE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock()
        && let Some(mode) = *guard
    {
        return match mode {
            HighlightBackground::Light => BackgroundMode::Light,
            HighlightBackground::Dark => BackgroundMode::Dark,
        };
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
        BackgroundMode::Dark => {
            let luma = 0.2126f32.mul_add(
                f32::from(color.r),
                0.7152f32.mul_add(f32::from(color.g), 0.0722 * f32::from(color.b)),
            );
            if luma < 1.0 {
                return color;
            }
            // Compress dynamic range: pull all colors toward a center
            // brightness so dim tokens get boosted and overly bright
            // tokens are tamed.
            let center = 175.0;
            let ratio = 0.45;
            let new_luma = (luma - center).mul_add(ratio, center);
            let scale = new_luma / luma;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            InlineColor {
                r: (f32::from(color.r) * scale).round().clamp(0.0, 255.0) as u8,
                g: (f32::from(color.g) * scale).round().clamp(0.0, 255.0) as u8,
                b: (f32::from(color.b) * scale).round().clamp(0.0, 255.0) as u8,
            }
        }
        BackgroundMode::Light => {
            let luma = 0.2126f32.mul_add(
                f32::from(color.r),
                0.7152f32.mul_add(f32::from(color.g), 0.0722 * f32::from(color.b)),
            );
            if luma < 155.0 {
                return color;
            }

            // Clamped to [0, 255]
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            InlineColor {
                r: (f32::from(color.r) * 0.42).round().clamp(0.0, 255.0) as u8,
                g: (f32::from(color.g) * 0.42).round().clamp(0.0, 255.0) as u8,
                b: (f32::from(color.b) * 0.42).round().clamp(0.0, 255.0) as u8,
            }
        }
    }
}
