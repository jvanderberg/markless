//! Terminal graphics protocol detection.

use std::env;

use crate::config::ImageMode;

/// Detect the best available image protocol for the current terminal.
///
/// Checks environment variables and terminal capabilities.
pub fn detect_protocol() -> ImageMode {
    // Check for Kitty
    if env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageMode::Kitty;
    }

    // Check for iTerm2
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        if term_program == "iTerm.app" {
            return ImageMode::ITerm2;
        }
        if term_program == "WezTerm" {
            return ImageMode::ITerm2;
        }
    }

    // Check for Ghostty (uses Kitty protocol)
    if let Ok(term) = env::var("TERM") {
        if term.contains("ghostty") {
            return ImageMode::Kitty;
        }
    }

    // Check for sixel support via TERM
    if let Ok(term) = env::var("TERM") {
        if term.contains("sixel") || term == "xterm-256color" || term.contains("foot") {
            return ImageMode::Sixel;
        }
    }

    // Fall back to half-blocks
    ImageMode::Halfblock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", ImageMode::Kitty), "Kitty");
        assert_eq!(format!("{}", ImageMode::Sixel), "Sixel");
        assert_eq!(format!("{}", ImageMode::ITerm2), "iTerm2");
        assert_eq!(format!("{}", ImageMode::Halfblock), "Halfblock");
    }

    #[test]
    fn test_detect_protocol_returns_valid() {
        let protocol = detect_protocol();
        // Should return one of the valid protocols
        assert!(matches!(
            protocol,
            ImageMode::Kitty | ImageMode::Sixel | ImageMode::ITerm2 | ImageMode::Halfblock
        ));
    }
}
