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
pub use protocol::detect_protocol;

use std::path::Path;
use std::time::Duration;

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
#[cfg(unix)]
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::config::ImageMode;

const PICKER_QUERY_TIMEOUT_MS: u64 = 250;

/// Map an [`ImageMode`] to the corresponding [`ProtocolType`].
const fn image_mode_to_protocol_type(mode: ImageMode) -> ProtocolType {
    match mode {
        ImageMode::Kitty => ProtocolType::Kitty,
        ImageMode::Sixel => ProtocolType::Sixel,
        ImageMode::ITerm2 => ProtocolType::Iterm2,
        ImageMode::Halfblock => ProtocolType::Halfblocks,
    }
}

/// Query stdio for font size, then override the protocol type.
///
/// The stdio query gives us accurate font/cell dimensions for image scaling,
/// while we override the detected protocol with one we trust more (from env
/// vars or CLI flags).
#[cfg(unix)]
fn picker_with_protocol(protocol_type: ProtocolType, reason: &str) -> Picker {
    let mut picker = Picker::from_query_stdio_with_options(query_options())
        .unwrap_or_else(|_| Picker::halfblocks());
    picker.set_protocol_type(protocol_type);
    crate::perf::log_event(
        "image.create_picker",
        format!("{reason} protocol={protocol_type:?}"),
    );
    picker
}

/// Create a picker for terminal image rendering.
///
/// When `image_mode` is `Some`, the picker is forced to use that protocol.
/// Otherwise, terminal capabilities are auto-detected.
///
/// When `query_stdio` is `false`, the terminal-capability stdio query is
/// skipped entirely. This must be used whenever stdin is piped input (e.g.
/// `markless -`): the query reads from stdin and would otherwise hang
/// forever reading EOF from an already-consumed pipe, and the terminal's
/// response to the query would arrive on `/dev/tty` rather than the piped
/// stdin anyway, so the query could never succeed in that case.
///
/// Without the stdio query, `image_mode` is honored directly when `Some`,
/// and when `None` the protocol is still auto-detected from environment
/// variables (see [`detect_protocol`]) so that Kitty/iTerm2/WezTerm/Ghostty
/// users piping markdown keep a real image protocol instead of always
/// falling back to half-blocks. In all cases the returned picker uses the
/// hardcoded half-blocks font size (10x20) rather than the terminal's real
/// cell metrics, since that information can only come from the stdio query.
/// This can cause image scaling misalignment for piped content with images
/// even when a pixel protocol (kitty/sixel/iterm2) is in use.
pub fn create_picker(image_mode: Option<ImageMode>, query_stdio: bool) -> Option<Picker> {
    if !query_stdio {
        let picker = image_mode.map_or_else(
            || {
                let detected = detect_protocol();
                let mut picker = Picker::halfblocks();
                if detected == ImageMode::Halfblock {
                    crate::perf::log_event(
                        "image.create_picker",
                        "stdio query skipped protocol=Halfblocks",
                    );
                } else {
                    let protocol_type = image_mode_to_protocol_type(detected);
                    picker.set_protocol_type(protocol_type);
                    crate::perf::log_event(
                        "image.create_picker",
                        format!("stdio query skipped env-detected protocol={protocol_type:?}"),
                    );
                }
                picker
            },
            |mode| {
                let protocol_type = image_mode_to_protocol_type(mode);
                let mut picker = Picker::halfblocks();
                if protocol_type != ProtocolType::Halfblocks {
                    picker.set_protocol_type(protocol_type);
                }
                crate::perf::log_event(
                    "image.create_picker",
                    format!("stdio query skipped protocol={protocol_type:?}"),
                );
                picker
            },
        );
        return Some(picker);
    }

    if let Some(mode) = image_mode {
        let protocol_type = image_mode_to_protocol_type(mode);
        if protocol_type == ProtocolType::Halfblocks {
            crate::perf::log_event("image.create_picker", "forced protocol=Halfblocks");
            return Some(Picker::halfblocks());
        }

        // For non-halfblock forced modes, we still need the terminal's real
        // font/cell size for correct image scaling.  Query stdio first so that
        // `picker.font_size()` returns the true dimensions, then override the
        // protocol type to the one the user requested.
        #[cfg(unix)]
        {
            return Some(picker_with_protocol(protocol_type, "forced"));
        }

        #[cfg(not(unix))]
        {
            let mut picker = Picker::halfblocks();
            picker.set_protocol_type(protocol_type);
            crate::perf::log_event(
                "image.create_picker",
                format!("forced protocol={:?}", protocol_type),
            );
            return Some(picker);
        }
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

    // Trust environment variables first (TERM_PROGRAM, KITTY_WINDOW_ID, etc.)
    // since they are set by the terminal and are reliable. Only fall back to
    // the stdio capability query for unknown terminals.
    #[cfg(unix)]
    {
        let env_detected = detect_protocol();
        if env_detected != ImageMode::Halfblock {
            let protocol_type = image_mode_to_protocol_type(env_detected);
            return Some(picker_with_protocol(protocol_type, "env"));
        }

        let picker = Picker::from_query_stdio_with_options(query_options()).ok()?;
        crate::perf::log_event(
            "image.create_picker",
            format!(
                "stdio term_program={} term={} colorterm={} protocol={:?}",
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
    let mut options = QueryStdioOptions::default();
    options.timeout = Duration::from_millis(PICKER_QUERY_TIMEOUT_MS);
    options
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
    // High byte of 16-bit terminal color
    #[allow(clippy::cast_possible_truncation)]
    let to_cube = |v: u8| ((u16::from(v) * 5) / 255) as u8;
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

    // `create_picker(_, false)` never performs the stdio capability query, so
    // the returned picker always uses the hardcoded half-blocks font size
    // regardless of which protocol ends up selected.
    #[test]
    fn test_create_picker_no_query_stdio_uses_halfblocks_font_size() {
        let picker = create_picker(None, false).expect("picker should be created");
        assert_eq!(picker.font_size(), (10, 20));
        // Auto-detect now consults `detect_protocol()`, which reads
        // environment variables that vary by CI/test environment, so only
        // assert the protocol is one of the known variants (env-independent).
        assert!(matches!(
            picker.protocol_type(),
            ProtocolType::Halfblocks
                | ProtocolType::Kitty
                | ProtocolType::Sixel
                | ProtocolType::Iterm2
        ));
    }

    #[test]
    fn test_create_picker_no_query_stdio_forced_kitty() {
        let picker =
            create_picker(Some(ImageMode::Kitty), false).expect("picker should be created");
        assert_eq!(picker.protocol_type(), ProtocolType::Kitty);
        assert_eq!(picker.font_size(), (10, 20));
    }

    #[test]
    fn test_create_picker_no_query_stdio_forced_iterm2() {
        let picker =
            create_picker(Some(ImageMode::ITerm2), false).expect("picker should be created");
        assert_eq!(picker.protocol_type(), ProtocolType::Iterm2);
        assert_eq!(picker.font_size(), (10, 20));
    }

    #[test]
    fn test_create_picker_no_query_stdio_forced_sixel() {
        let picker =
            create_picker(Some(ImageMode::Sixel), false).expect("picker should be created");
        assert_eq!(picker.protocol_type(), ProtocolType::Sixel);
        assert_eq!(picker.font_size(), (10, 20));
    }

    #[test]
    fn test_create_picker_no_query_stdio_forced_halfblock() {
        let picker =
            create_picker(Some(ImageMode::Halfblock), false).expect("picker should be created");
        assert_eq!(picker.protocol_type(), ProtocolType::Halfblocks);
        assert_eq!(picker.font_size(), (10, 20));
    }
}
