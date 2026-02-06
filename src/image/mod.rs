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
pub use protocol::{ImageProtocol, detect_protocol};

use std::path::Path;
use std::time::Duration;

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use ratatui_image::picker::Picker;
#[cfg(unix)]
use ratatui_image::picker::cap_parser::QueryStdioOptions;

const PICKER_QUERY_TIMEOUT_MS: u64 = 250;

/// Create a picker for terminal image rendering.
///
/// The picker detects terminal capabilities and chooses the best protocol.
pub fn create_picker(force_half_cell: bool) -> Option<Picker> {
    if force_half_cell {
        crate::perf::log_event(
            "image.create_picker",
            "force_half_cell=true protocol=Halfblocks",
        );
        return Some(Picker::halfblocks());
    }

    // On Windows, skip the stdio capability query — it can leave orphaned reader
    // threads on the console input buffer, causing the app to lock up in some
    // terminals (e.g. Fluent Terminal). Fall back to half-block rendering.
    #[cfg(not(unix))]
    {
        crate::perf::log_event(
            "image.create_picker",
            "windows fallback protocol=Halfblocks",
        );
        return Some(Picker::halfblocks());
    }

    // Try to create a picker, which will detect the terminal's capabilities
    #[cfg(unix)]
    {
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

/// Whether terminal output should be treated as truecolor-capable.
pub fn supports_truecolor_terminal() -> bool {
    if let Ok(force) = std::env::var("MARKLESS_TRUECOLOR") {
        let value = force.to_ascii_lowercase();
        return matches!(value.as_str(), "1" | "true" | "yes" | "on");
    }
    if std::env::var("TERM_PROGRAM")
        .ok()
        .as_deref()
        .is_some_and(|v| v == "Apple_Terminal")
    {
        return false;
    }
    supports_truecolor_from_env(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

/// Quantize image RGB channels to the ANSI-256 palette while preserving alpha.
pub fn quantize_to_ansi256(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let mut out = RgbaImage::new(width, height);
    let src = image.to_rgba8();

    for (x, y, px) in src.enumerate_pixels() {
        let idx = rgb_to_xterm_256(px[0], px[1], px[2]);
        let (r, g, b) = xterm_256_to_rgb(idx);
        out.put_pixel(x, y, Rgba([r, g, b, px[3]]));
    }

    DynamicImage::ImageRgba8(out)
}

#[cfg(unix)]
fn query_options() -> QueryStdioOptions {
    QueryStdioOptions {
        timeout: Duration::from_millis(PICKER_QUERY_TIMEOUT_MS),
        ..QueryStdioOptions::default()
    }
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
    let to_cube = |v: u8| ((v as u16 * 5) / 255) as u8;
    let ri = to_cube(r);
    let gi = to_cube(g);
    let bi = to_cube(b);
    16 + (36 * ri) + (6 * gi) + bi
}

fn xterm_256_to_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0 => (0, 0, 0),
        1 => (205, 0, 0),
        2 => (0, 205, 0),
        3 => (205, 205, 0),
        4 => (0, 0, 238),
        5 => (205, 0, 205),
        6 => (0, 205, 205),
        7 => (229, 229, 229),
        8 => (127, 127, 127),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (92, 92, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            let i = i - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            let to_val = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            let gray = 8 + (i - 232) * 10;
            (gray, gray, gray)
        }
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

    #[test]
    fn test_supports_truecolor_from_env_detects_24bit() {
        assert!(supports_truecolor_from_env(
            Some("truecolor"),
            Some("xterm-256color")
        ));
        assert!(supports_truecolor_from_env(Some("24BIT"), Some("screen")));
    }

    #[test]
    fn test_supports_truecolor_from_env_detects_non_truecolor() {
        assert!(!supports_truecolor_from_env(None, Some("xterm-256color")));
    }

    #[test]
    fn test_quantize_to_ansi256_preserves_alpha() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([12, 34, 56, 77])));
        let quantized = quantize_to_ansi256(&image).to_rgba8();
        assert_eq!(quantized.get_pixel(0, 0)[3], 77);
    }
}
