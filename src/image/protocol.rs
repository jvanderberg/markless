//! Terminal graphics protocol detection.

use std::env;

/// Supported image rendering protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Kitty graphics protocol (best quality)
    Kitty,
    /// Sixel graphics (wide support)
    Sixel,
    /// iTerm2 inline images
    ITerm2,
    /// Unicode half-blocks (universal fallback)
    Halfblock,
}

impl std::fmt::Display for ImageProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kitty => write!(f, "Kitty"),
            Self::Sixel => write!(f, "Sixel"),
            Self::ITerm2 => write!(f, "iTerm2"),
            Self::Halfblock => write!(f, "Halfblock"),
        }
    }
}

/// Detect the best available image protocol for the current terminal.
///
/// Checks environment variables and terminal capabilities.
pub fn detect_protocol() -> ImageProtocol {
    // Check for Kitty
    if env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageProtocol::Kitty;
    }

    // Check for iTerm2
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        if term_program == "iTerm.app" {
            return ImageProtocol::ITerm2;
        }
        if term_program == "WezTerm" {
            return ImageProtocol::ITerm2;
        }
    }

    // Check for Ghostty (uses Kitty protocol)
    if let Ok(term) = env::var("TERM") {
        if term.contains("ghostty") {
            return ImageProtocol::Kitty;
        }
    }

    // Check for sixel support via TERM
    if let Ok(term) = env::var("TERM") {
        if term.contains("sixel") || term == "xterm-256color" || term.contains("foot") {
            return ImageProtocol::Sixel;
        }
    }

    // Fall back to half-blocks
    ImageProtocol::Halfblock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", ImageProtocol::Kitty), "Kitty");
        assert_eq!(format!("{}", ImageProtocol::Sixel), "Sixel");
        assert_eq!(format!("{}", ImageProtocol::ITerm2), "iTerm2");
        assert_eq!(format!("{}", ImageProtocol::Halfblock), "Halfblock");
    }

    #[test]
    fn test_detect_protocol_returns_valid() {
        let protocol = detect_protocol();
        // Should return one of the valid protocols
        assert!(matches!(
            protocol,
            ImageProtocol::Kitty
                | ImageProtocol::Sixel
                | ImageProtocol::ITerm2
                | ImageProtocol::Halfblock
        ));
    }
}
