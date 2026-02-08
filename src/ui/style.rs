//! Theming and color definitions.
//!
//! This module defines the visual styling for rendered markdown elements.
//! Uses ANSI colors that adapt to the terminal's color palette.

use ratatui::style::{Color, Modifier, Style};

use crate::document::{InlineStyle, LineType};

/// Get the style for a given line type.
///
/// Uses semantic ANSI colors that respect the terminal's theme.
pub fn style_for_line_type(line_type: &LineType) -> Style {
    let light_bg = crate::highlight::is_light_background();
    match line_type {
        // Headings - bold with distinct colors per level
        LineType::Heading(1) => Style::default()
            .fg(if light_bg {
                Color::Indexed(24)
            } else {
                Color::Cyan
            })
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        LineType::Heading(2) => Style::default()
            .fg(if light_bg {
                Color::Indexed(22)
            } else {
                Color::Green
            })
            .add_modifier(Modifier::BOLD),
        LineType::Heading(3) => Style::default()
            .fg(if light_bg {
                Color::Indexed(58)
            } else {
                Color::Yellow
            })
            .add_modifier(Modifier::BOLD),
        LineType::Heading(4) => Style::default()
            .fg(if light_bg {
                Color::Indexed(24)
            } else {
                Color::Blue
            })
            .add_modifier(Modifier::BOLD),
        LineType::Heading(5) => Style::default()
            .fg(if light_bg {
                Color::Indexed(54)
            } else {
                Color::Magenta
            })
            .add_modifier(Modifier::BOLD),
        LineType::Heading(_) => Style::default()
            .fg(if light_bg {
                Color::Indexed(24)
            } else {
                Color::Cyan
            })
            .add_modifier(Modifier::BOLD),

        // Code blocks - use a dimmer color, italic for differentiation
        LineType::CodeBlock => Style::default()
            .fg(if light_bg {
                Color::Indexed(238)
            } else {
                Color::Indexed(245)
            })
            .add_modifier(Modifier::DIM),

        // Block quotes - italic blue
        LineType::BlockQuote => Style::default()
            .fg(if light_bg {
                Color::Indexed(24)
            } else {
                Color::Blue
            })
            .add_modifier(Modifier::ITALIC),

        // Horizontal rule - dim
        LineType::HorizontalRule => Style::default()
            .fg(if light_bg {
                Color::Indexed(241)
            } else {
                Color::Indexed(240)
            })
            .add_modifier(Modifier::DIM),

        // Images - magenta italic to stand out as placeholder
        LineType::Image => Style::default()
            .fg(if light_bg {
                Color::Indexed(90)
            } else {
                Color::Magenta
            })
            .add_modifier(Modifier::ITALIC),

        // List items, tables, paragraphs, empty lines - normal style
        LineType::ListItem(_) | LineType::Table | LineType::Paragraph | LineType::Empty => {
            Style::default()
        }
    }
}

/// Get the style for an inline span, merged with a base line style.
pub fn style_for_inline(base: Style, inline: InlineStyle) -> Style {
    let mut style = base;

    if let Some(fg) = inline.fg {
        style = style
            .fg(fg_color_for_terminal(fg))
            .remove_modifier(Modifier::DIM);
    }
    if let Some(bg) = inline.bg {
        style = style.bg(Color::Rgb(bg.r, bg.g, bg.b));
    }

    if inline.emphasis {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if inline.strong {
        style = style.add_modifier(Modifier::BOLD);
    }
    if inline.strikethrough {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if inline.link {
        style = style.add_modifier(Modifier::UNDERLINED);
        if inline.fg.is_none() {
            let light_bg = crate::highlight::is_light_background();
            style = style.fg(if light_bg {
                Color::Blue
            } else {
                Color::LightBlue
            });
        }
    }
    if inline.code && inline.fg.is_none() {
        let light_bg = crate::highlight::is_light_background();
        style = style
            .fg(if light_bg {
                Color::Indexed(88)
            } else {
                Color::Red
            })
            .add_modifier(Modifier::BOLD);
    }

    style
}

fn fg_color_for_terminal(fg: crate::document::InlineColor) -> Color {
    if supports_truecolor() {
        Color::Rgb(fg.r, fg.g, fg.b)
    } else {
        Color::Indexed(rgb_to_xterm_256(fg.r, fg.g, fg.b))
    }
}

fn supports_truecolor() -> bool {
    if let Ok(force) = std::env::var("MARKLESS_TRUECOLOR") {
        let value = force.to_ascii_lowercase();
        return matches!(value.as_str(), "1" | "true" | "yes" | "on");
    }
    supports_truecolor_from_env(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

fn supports_truecolor_from_env(colorterm: Option<&str>, term: Option<&str>) -> bool {
    if let Some(ct) = colorterm {
        let lower = ct.to_ascii_lowercase();
        if lower.contains("truecolor") || lower.contains("24bit") {
            return true;
        }
    }
    if let Some(t) = term {
        let lower = t.to_ascii_lowercase();
        if lower.contains("direct") || lower.contains("truecolor") {
            return true;
        }
    }
    false
}

fn rgb_to_xterm_256(r: u8, g: u8, b: u8) -> u8 {
    // Result is always 0-5, fits in u8
    #[allow(clippy::cast_possible_truncation)]
    let to_cube = |v: u8| ((u16::from(v) * 5) / 255) as u8;
    let ri = to_cube(r);
    let gi = to_cube(g);
    let bi = to_cube(b);
    16 + (36 * ri) + (6 * gi) + bi
}

/// Theme configuration for the entire application.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Heading level 1 style
    pub h1: Style,
    /// Heading level 2 style
    pub h2: Style,
    /// Heading level 3 style
    pub h3: Style,
    /// Heading level 4+ style
    pub h4: Style,
    /// Code block style
    pub code: Style,
    /// Inline code style
    pub inline_code: Style,
    /// Block quote style
    pub quote: Style,
    /// Link style
    pub link: Style,
    /// Emphasis (italic) style
    pub emphasis: Style,
    /// Strong (bold) style
    pub strong: Style,
    /// Strikethrough style
    pub strikethrough: Style,
    /// List bullet/number style
    pub list_marker: Style,
    /// Table border style
    pub table_border: Style,
    /// Image placeholder style
    pub image: Style,
    /// Horizontal rule style
    pub hr: Style,
    /// Status bar background
    pub status_bg: Color,
    /// Status bar foreground
    pub status_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            h1: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            h2: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            h3: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            h4: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            code: Style::default().fg(Color::Indexed(245)),
            inline_code: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            quote: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::ITALIC),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            strong: Style::default().add_modifier(Modifier::BOLD),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            list_marker: Style::default().fg(Color::Yellow),
            table_border: Style::default().fg(Color::Indexed(240)),
            image: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::ITALIC),
            hr: Style::default().fg(Color::Indexed(240)),
            status_bg: Color::Indexed(236), // Dark gray that works on both
            status_fg: Color::Indexed(252), // Light gray
        }
    }
}

impl Theme {
    /// Create a theme optimized for dark terminals.
    pub fn dark() -> Self {
        Self::default()
    }

    /// Create a theme optimized for light terminals.
    pub fn light() -> Self {
        Self {
            h1: Style::default()
                .fg(Color::Indexed(31)) // Darker cyan
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            h2: Style::default()
                .fg(Color::Indexed(28)) // Darker green
                .add_modifier(Modifier::BOLD),
            h3: Style::default()
                .fg(Color::Indexed(136)) // Darker yellow/olive
                .add_modifier(Modifier::BOLD),
            h4: Style::default()
                .fg(Color::Indexed(25)) // Darker blue
                .add_modifier(Modifier::BOLD),
            code: Style::default().fg(Color::Indexed(240)),
            inline_code: Style::default()
                .fg(Color::Indexed(124)) // Darker red
                .add_modifier(Modifier::BOLD),
            quote: Style::default()
                .fg(Color::Indexed(25))
                .add_modifier(Modifier::ITALIC),
            link: Style::default()
                .fg(Color::Indexed(25))
                .add_modifier(Modifier::UNDERLINED),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            strong: Style::default().add_modifier(Modifier::BOLD),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            list_marker: Style::default().fg(Color::Indexed(136)),
            table_border: Style::default().fg(Color::Indexed(245)),
            image: Style::default()
                .fg(Color::Indexed(133))
                .add_modifier(Modifier::ITALIC),
            hr: Style::default().fg(Color::Indexed(245)),
            status_bg: Color::Indexed(252),
            status_fg: Color::Indexed(235),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::InlineColor;

    #[test]
    fn test_heading_styles_are_bold() {
        for level in 1..=6 {
            let style = style_for_line_type(&LineType::Heading(level));
            assert!(style.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn test_h1_is_underlined() {
        let style = style_for_line_type(&LineType::Heading(1));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_code_block_style() {
        let style = style_for_line_type(&LineType::CodeBlock);
        assert!(style.fg.is_some());
    }

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert!(theme.h1.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert!(theme.h1.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert!(theme.h1.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_inline_color_removes_dim_modifier() {
        let base = Style::default().add_modifier(Modifier::DIM);
        let mut inline = InlineStyle::default();
        inline.fg = Some(InlineColor { r: 255, g: 0, b: 0 });

        let styled = style_for_inline(base, inline);
        assert!(!styled.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn test_truecolor_detection_without_colorterm() {
        assert!(!supports_truecolor_from_env(None, Some("xterm-256color")));
    }

    #[test]
    fn test_truecolor_detection_with_colorterm() {
        assert!(supports_truecolor_from_env(
            Some("truecolor"),
            Some("xterm-256color")
        ));
    }

    #[test]
    fn test_fallback_indexed_color_when_not_truecolor() {
        let idx = rgb_to_xterm_256(255, 0, 0);
        assert_eq!(idx, 196);
    }
}
