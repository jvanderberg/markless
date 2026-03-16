//! Kitty text sizing protocol support for heading rendering.
//!
//! When the terminal supports the Kitty text sizing protocol (kitty >= 0.40.0),
//! headings can be rendered at larger sizes using the OSC 66 escape sequence.
//! This makes headers visually distinct without relying solely on color or
//! `#` prefixes.
//!
//! Protocol reference: <https://sw.kovidgoyal.net/kitty/text-sizing-protocol/>

/// Return the scale factor for a heading level when using large text.
///
/// H1 and H2 are rendered at 2× scale; all other levels use normal size.
pub const fn heading_scale(level: u8) -> u8 {
    match level {
        1 | 2 => 2,
        _ => 1,
    }
}

/// Format the OSC 66 escape sequence for scaled text.
///
/// The sequence renders `text` at `scale`× size, occupying `scale` cells tall
/// and `scale` cells wide per character. The width parameter `w` is set to 0
/// (auto-calculate).
///
/// Returns `None` if `scale <= 1` (no escape needed) or if `text` is empty.
pub fn format_osc66(text: &str, scale: u8) -> Option<String> {
    if scale <= 1 || text.is_empty() {
        return None;
    }
    // OSC 66 ; s=<scale> ; <text> BEL
    Some(format!("\x1b]66;s={scale};{text}\x07"))
}

/// Truncate heading text to fit within the available column width at the given scale.
///
/// Each character occupies `scale` columns, so the maximum number of characters
/// is `available_cols / scale`.
pub fn truncate_to_fit(text: &str, available_cols: u16, scale: u8) -> &str {
    if scale == 0 {
        return text;
    }
    let max_chars = (available_cols / u16::from(scale)) as usize;
    if text.chars().count() <= max_chars {
        return text;
    }
    // Find the byte index at the max_chars boundary
    text.char_indices()
        .nth(max_chars)
        .map_or(text, |(byte_idx, _)| &text[..byte_idx])
}

/// Detect whether the current terminal supports Kitty's text sizing protocol.
///
/// Checks `KITTY_WINDOW_ID` (always set by Kitty) and falls back to
/// `TERM_PROGRAM=kitty` or `TERM` containing `xterm-kitty` for environments
/// where `KITTY_WINDOW_ID` may not propagate (e.g. some SSH sessions).
pub fn detect_support() -> bool {
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }
    if std::env::var("TERM_PROGRAM")
        .ok()
        .is_some_and(|v| v.eq_ignore_ascii_case("kitty"))
    {
        return true;
    }
    std::env::var("TERM")
        .ok()
        .is_some_and(|v| v.contains("xterm-kitty"))
}

/// Information about a heading that needs post-render large text output.
#[derive(Debug, Clone)]
pub struct LargeTextHeading {
    /// Terminal column (x) where the heading text starts
    pub col: u16,
    /// Terminal row (y) where the heading text starts
    pub row: u16,
    /// The heading text to render
    pub text: String,
    /// Scale factor (2 for H1/H2)
    pub scale: u8,
    /// ANSI color escape to apply before the heading
    pub color_seq: String,
}

/// Collect visible headings that need large text rendering.
///
/// Returns a list of headings within the current viewport that have scale > 1.
pub fn collect_visible_large_headings(
    model: &crate::app::Model,
    doc_area_x: u16,
    doc_area_y: u16,
    doc_area_width: u16,
    doc_area_height: u16,
) -> Vec<LargeTextHeading> {
    let mut headings = Vec::new();
    let vp_offset = model.viewport.offset();

    for heading_ref in model.document.headings() {
        let scale = heading_scale(heading_ref.level);
        if scale <= 1 {
            continue;
        }

        let heading_line = heading_ref.line;
        // Check if heading is visible in viewport
        if heading_line < vp_offset {
            continue;
        }
        let rel_y = heading_line - vp_offset;
        if rel_y >= doc_area_height as usize {
            continue;
        }

        // Get the heading text from the document line
        let text = if let Some(line) = model.document.line_at(heading_line) {
            line.content().to_string()
        } else {
            continue;
        };

        let truncated = truncate_to_fit(&text, doc_area_width, scale);

        // Build ANSI color sequence for the heading style
        let style = crate::ui::style::style_for_line_type(&crate::document::LineType::Heading(
            heading_ref.level,
        ));
        let color_seq = ansi_color_from_style(style);

        // rel_y is bounded by doc_area_height (u16), safe to truncate
        #[allow(clippy::cast_possible_truncation)]
        let rel_y_u16 = rel_y as u16;
        headings.push(LargeTextHeading {
            col: doc_area_x,
            row: doc_area_y + rel_y_u16,
            text: truncated.to_string(),
            scale,
            color_seq,
        });
    }

    headings
}

/// Write large text headings to stdout using OSC 66 escape sequences.
///
/// This should be called after `terminal.draw()` to overlay the scaled text
/// on top of ratatui's rendered frame.
///
/// # Errors
///
/// Returns an error if writing to stdout fails.
pub fn write_large_headings(headings: &[LargeTextHeading]) -> std::io::Result<()> {
    use std::io::Write;

    if headings.is_empty() {
        return Ok(());
    }

    let mut out = std::io::stdout();
    for heading in headings {
        let Some(osc) = format_osc66(&heading.text, heading.scale) else {
            continue;
        };
        // Move cursor to position (1-indexed for ANSI)
        write!(
            out,
            "\x1b[{};{}H{}{}\x1b[0m",
            heading.row + 1,
            heading.col + 1,
            heading.color_seq,
            osc
        )?;
    }
    out.flush()
}

/// Convert a ratatui Style's foreground color + modifiers to an ANSI escape sequence.
fn ansi_color_from_style(style: ratatui::style::Style) -> String {
    use ratatui::style::{Color, Modifier};

    let mut parts = Vec::new();

    // Bold
    if style.add_modifier.contains(Modifier::BOLD) {
        parts.push("1".to_string());
    }
    // Underlined
    if style.add_modifier.contains(Modifier::UNDERLINED) {
        parts.push("4".to_string());
    }

    // Foreground color
    if let Some(fg) = style.fg {
        match fg {
            Color::Black => parts.push("30".to_string()),
            Color::Red => parts.push("31".to_string()),
            Color::Green => parts.push("32".to_string()),
            Color::Yellow => parts.push("33".to_string()),
            Color::Blue => parts.push("34".to_string()),
            Color::Magenta => parts.push("35".to_string()),
            Color::Cyan => parts.push("36".to_string()),
            Color::White => parts.push("37".to_string()),
            Color::Indexed(idx) => parts.push(format!("38;5;{idx}")),
            Color::Rgb(r, g, b) => parts.push(format!("38;2;{r};{g};{b}")),
            Color::LightRed => parts.push("91".to_string()),
            Color::LightGreen => parts.push("92".to_string()),
            Color::LightYellow => parts.push("93".to_string()),
            Color::LightBlue => parts.push("94".to_string()),
            Color::LightMagenta => parts.push("95".to_string()),
            Color::LightCyan => parts.push("96".to_string()),
            Color::Gray | Color::DarkGray => parts.push("90".to_string()),
            Color::Reset => {}
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", parts.join(";"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_scale_h1_returns_2() {
        assert_eq!(heading_scale(1), 2);
    }

    #[test]
    fn test_heading_scale_h2_returns_2() {
        assert_eq!(heading_scale(2), 2);
    }

    #[test]
    fn test_heading_scale_h3_returns_1() {
        assert_eq!(heading_scale(3), 1);
    }

    #[test]
    fn test_heading_scale_h4_through_h6_returns_1() {
        for level in 4..=6 {
            assert_eq!(heading_scale(level), 1, "H{level} should have scale 1");
        }
    }

    #[test]
    fn test_format_osc66_scale_2() {
        let seq = format_osc66("Hello", 2).unwrap();
        assert_eq!(seq, "\x1b]66;s=2;Hello\x07");
    }

    #[test]
    fn test_format_osc66_scale_1_returns_none() {
        assert!(format_osc66("Hello", 1).is_none());
    }

    #[test]
    fn test_format_osc66_empty_text_returns_none() {
        assert!(format_osc66("", 2).is_none());
    }

    #[test]
    fn test_truncate_to_fit_no_truncation_needed() {
        let text = "Short";
        assert_eq!(truncate_to_fit(text, 80, 2), "Short");
    }

    #[test]
    fn test_truncate_to_fit_truncates_at_scale_2() {
        // 10 cols / scale 2 = 5 chars max
        let text = "Hello World";
        assert_eq!(truncate_to_fit(text, 10, 2), "Hello");
    }

    #[test]
    fn test_truncate_to_fit_unicode() {
        // 8 cols / scale 2 = 4 chars max
        let text = "Héllo";
        assert_eq!(truncate_to_fit(text, 8, 2), "Héll");
    }

    #[test]
    fn test_truncate_to_fit_scale_1_no_truncation() {
        let text = "Hello World";
        assert_eq!(truncate_to_fit(text, 80, 1), "Hello World");
    }

    #[test]
    fn test_ansi_color_bold_cyan() {
        use ratatui::style::{Color, Modifier, Style};
        let style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let seq = ansi_color_from_style(style);
        assert!(seq.contains("1"), "should have bold");
        assert!(seq.contains("36"), "should have cyan fg");
    }

    #[test]
    fn test_ansi_color_bold_underlined() {
        use ratatui::style::{Color, Modifier, Style};
        let style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let seq = ansi_color_from_style(style);
        assert!(seq.contains("1"), "should have bold");
        assert!(seq.contains("4"), "should have underline");
    }

    #[test]
    fn test_ansi_color_indexed() {
        use ratatui::style::{Color, Style};
        let style = Style::default().fg(Color::Indexed(24));
        let seq = ansi_color_from_style(style);
        assert!(seq.contains("38;5;24"), "should have indexed color 24");
    }

    #[test]
    fn test_ansi_color_empty_for_default_style() {
        use ratatui::style::Style;
        let style = Style::default();
        let seq = ansi_color_from_style(style);
        assert!(seq.is_empty(), "default style should produce empty seq");
    }

    #[test]
    fn test_format_osc66_scale_3() {
        let seq = format_osc66("Title", 3).unwrap();
        assert_eq!(seq, "\x1b]66;s=3;Title\x07");
    }

    #[test]
    fn test_write_large_headings_empty_is_ok() {
        assert!(write_large_headings(&[]).is_ok());
    }
}
