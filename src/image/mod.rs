//! Image loading and rendering.
//!
//! Supports multiple terminal graphics protocols:
//! - Kitty graphics protocol
//! - Sixel
//! - iTerm2
//! - Unicode half-blocks (fallback)

mod loader;
mod protocol;

pub use loader::{ImageCache, ImageLoader};
pub use protocol::{detect_protocol, ImageProtocol};

use std::path::Path;
use std::time::Duration;

use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;

const PICKER_QUERY_TIMEOUT_MS: u64 = 250;

/// Create a picker for terminal image rendering.
///
/// The picker detects terminal capabilities and chooses the best protocol.
pub fn create_picker() -> Option<Picker> {
    // Try to create a picker, which will detect the terminal's capabilities
    let picker = Picker::from_query_stdio_with_options(query_options()).ok()?;
    crate::perf::log_event(
        "image.create_picker",
        format!(
            "term_program={} term={} colorterm={} protocol={:?}",
            std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("TERM").unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("COLORTERM").unwrap_or_else(|_| "<unset>".to_string()),
            picker.protocol_type()
        ),
    );
    Some(picker)
}

/// Load an image from a file path relative to a base directory.
pub fn load_image(base_path: &Path, image_path: &str) -> Option<DynamicImage> {
    let full_path = if Path::new(image_path).is_absolute() {
        image_path.into()
    } else {
        base_path.join(image_path)
    };

    image::open(&full_path).ok()
}

fn query_options() -> QueryStdioOptions {
    QueryStdioOptions {
        timeout: Duration::from_millis(PICKER_QUERY_TIMEOUT_MS),
        ..QueryStdioOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picker_query_timeout_is_fast() {
        let options = query_options();
        assert_eq!(options.timeout, Duration::from_millis(250));
    }
}
