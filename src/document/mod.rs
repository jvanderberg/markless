//! Markdown document parsing and rendering.
//!
//! This module handles:
//! - Parsing markdown with comrak
//! - Extracting document structure (headings, links, images)
//! - Rendering to styled lines for display

mod parser;
mod types;

pub use parser::{parse, parse_with_image_heights, parse_with_layout};
pub use types::{
    Document, HeadingRef, ImageRef, InlineColor, InlineSpan, InlineStyle, LineType, LinkRef,
    RenderedLine,
};

/// Image file extensions that should be rendered inline.
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "ico", "svg", "avif",
];

/// Prepare file content for rendering based on its extension.
///
/// If the file has a recognized code extension, wrap content in a fenced code
/// block so it renders with syntax highlighting. Image files are wrapped as
/// markdown image references for inline rendering. Markdown and unrecognized
/// files pass through unchanged.
pub fn prepare_content(file_path: &std::path::Path, content: String) -> String {
    if is_image_file(file_path) {
        return image_markdown(file_path);
    }
    if is_csv_file(file_path) {
        return format!("```csv\n{content}\n```");
    }
    let Some(language) = crate::highlight::language_for_file(file_path) else {
        return content;
    };
    format!("```{language}\n{content}\n```")
}

/// Returns true if the file extension is `.csv`.
fn is_csv_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
}

/// Returns true if the file extension is a recognized image format.
pub fn is_image_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Generate markdown content that displays an image file inline.
///
/// Uses angle brackets around the URL so filenames with spaces or
/// parentheses are parsed correctly by `CommonMark`.
pub fn image_markdown(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    format!("![{name}](<{name}>)")
}

/// Prepare file content from raw bytes, detecting binary vs text.
///
/// Image files are rendered as markdown images. Text files (valid UTF-8
/// without null bytes) are passed to [`prepare_content`]. Binary files
/// are displayed as a hex dump with a heading showing the file name and size.
pub fn prepare_content_from_bytes(file_path: &std::path::Path, bytes: Vec<u8>) -> String {
    if is_image_file(file_path) {
        return image_markdown(file_path);
    }
    if !is_binary(&bytes) {
        match String::from_utf8(bytes) {
            Ok(content) => return prepare_content(file_path, content),
            Err(e) => return prepare_content(file_path, e.to_string()),
        }
    }
    let name = file_path
        .file_name()
        .map_or_else(|| "binary".to_string(), |n| n.to_string_lossy().to_string());
    let size = bytes.len();
    let hex = format_hex_dump(&bytes);
    format!("# {name}\n\n*Binary file — {size} bytes*\n\n```\n{hex}\n```")
}

/// Prepare a [`Document`] directly from raw file bytes.
///
/// Binary files use lazy hex rendering ([`Document::from_hex`]) and skip
/// comrak entirely. Image files and text files use the existing markdown
/// parsing pipeline.
pub fn prepare_document_from_bytes(
    file_path: &std::path::Path,
    bytes: Vec<u8>,
    layout_width: u16,
) -> Document {
    if is_image_file(file_path) {
        let md = image_markdown(file_path);
        return Document::parse_with_layout(&md, layout_width)
            .unwrap_or_else(|_| Document::empty());
    }
    if !is_binary(&bytes) {
        let content = match String::from_utf8(bytes) {
            Ok(s) => prepare_content(file_path, s),
            Err(e) => prepare_content(file_path, e.to_string()),
        };
        return Document::parse_with_layout(&content, layout_width)
            .unwrap_or_else(|_| Document::empty());
    }
    let name = file_path
        .file_name()
        .map_or_else(|| "binary".to_string(), |n| n.to_string_lossy().to_string());
    Document::from_hex(&name, bytes)
}

/// Returns true if the byte slice appears to be binary (non-text) content.
///
/// Checks for null bytes and invalid UTF-8. Text files with valid UTF-8
/// multibyte sequences are not considered binary.
pub fn is_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.contains(&0) {
        return true;
    }
    std::str::from_utf8(bytes).is_err()
}

/// Format a single 16-byte chunk as one hex dump line.
///
/// Takes a chunk of up to 16 bytes and a byte offset, returning a line
/// in `hexdump -C` style: 8-digit hex offset, 16 bytes in two groups of 8,
/// and an ASCII representation where non-printable bytes appear as `.`.
pub fn format_single_hex_line(chunk: &[u8], offset: usize) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    // Offset
    let _ = write!(output, "{offset:08x}  ");

    // Hex bytes: first group of 8
    for i in 0..8 {
        if i < chunk.len() {
            let _ = write!(output, "{:02x} ", chunk[i]);
        } else {
            output.push_str("   ");
        }
    }
    output.push(' ');

    // Hex bytes: second group of 8
    for i in 8..16 {
        if i < chunk.len() {
            let _ = write!(output, "{:02x} ", chunk[i]);
        } else {
            output.push_str("   ");
        }
    }

    // ASCII representation
    output.push('|');
    for &byte in chunk {
        if byte.is_ascii_graphic() || byte == b' ' {
            output.push(byte as char);
        } else {
            output.push('.');
        }
    }
    output.push('|');

    output
}

/// Format raw bytes as a hex dump in the classic `hexdump -C` style.
///
/// Each line shows: 8-digit hex offset, 16 bytes in two groups of 8,
/// and an ASCII representation where non-printable bytes appear as `.`.
pub fn format_hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for (chunk_idx, chunk) in bytes.chunks(16).enumerate() {
        let offset = chunk_idx * 16;
        output.push_str(&format_single_hex_line(chunk, offset));
        output.push('\n');
    }

    // Remove trailing newline
    output.truncate(output.len() - 1);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_prepare_content_wraps_rust_file() {
        let content = "fn main() {}".to_string();
        let result = prepare_content(Path::new("main.rs"), content);
        assert!(
            result.starts_with("```Rust\n"),
            "should start with Rust fence"
        );
        assert!(result.ends_with("\n```"), "should end with closing fence");
        assert!(
            result.contains("fn main() {}"),
            "should contain original code"
        );
    }

    #[test]
    fn test_prepare_content_passes_markdown_through() {
        let content = "# Hello\nworld".to_string();
        let result = prepare_content(Path::new("README.md"), content.clone());
        assert_eq!(
            result, content,
            "Markdown content should pass through unchanged"
        );
    }

    #[test]
    fn test_prepare_content_wraps_csv_file() {
        let content = "Name,Age,City\nAlice,30,NYC\nBob,25,LA".to_string();
        let result = prepare_content(Path::new("data.csv"), content);
        assert!(
            result.starts_with("```csv\n"),
            "CSV file should be wrapped in csv fence, got: {result}"
        );
        assert!(result.ends_with("\n```"), "should end with closing fence");
    }

    #[test]
    fn test_prepare_document_from_bytes_csv_renders_as_table() {
        let bytes = b"Name,Age,City\nAlice,30,NYC\nBob,25,LA".to_vec();
        let doc = prepare_document_from_bytes(Path::new("data.csv"), bytes, 80);
        assert!(!doc.is_hex_mode());
        // Should contain table lines, not paragraph text
        let has_table = (0..doc.line_count()).any(|i| {
            doc.line_at(i)
                .is_some_and(|l| *l.line_type() == LineType::Table)
        });
        assert!(has_table, "CSV file should render with Table line types");
    }

    #[test]
    fn test_prepare_content_passes_unknown_through() {
        let content = "some data".to_string();
        let result = prepare_content(Path::new("data.xyz"), content.clone());
        assert_eq!(
            result, content,
            "Unknown extension should pass through unchanged"
        );
    }

    #[test]
    fn test_prepare_content_wraps_png_as_image() {
        let content = "binary data".to_string();
        let result = prepare_content(Path::new("photo.png"), content);
        assert!(result.contains("![photo.png](<photo.png>)"));
    }

    #[test]
    fn test_prepare_content_wraps_jpg_as_image() {
        let result = prepare_content(Path::new("pic.jpg"), "data".to_string());
        assert!(result.contains("![pic.jpg](<pic.jpg>)"));
    }

    #[test]
    fn test_prepare_content_wraps_jpeg_as_image() {
        let result = prepare_content(Path::new("pic.jpeg"), "data".to_string());
        assert!(result.contains("![pic.jpeg](<pic.jpeg>)"));
    }

    #[test]
    fn test_prepare_content_wraps_gif_as_image() {
        let result = prepare_content(Path::new("anim.gif"), "data".to_string());
        assert!(result.contains("![anim.gif](<anim.gif>)"));
    }

    #[test]
    fn test_prepare_content_wraps_webp_as_image() {
        let result = prepare_content(Path::new("photo.webp"), "data".to_string());
        assert!(result.contains("![photo.webp](<photo.webp>)"));
    }

    #[test]
    fn test_prepare_content_wraps_bmp_as_image() {
        let result = prepare_content(Path::new("icon.bmp"), "data".to_string());
        assert!(result.contains("![icon.bmp](<icon.bmp>)"));
    }

    #[test]
    fn test_prepare_content_wraps_svg_as_image() {
        let result = prepare_content(Path::new("logo.svg"), "data".to_string());
        assert!(result.contains("![logo.svg](<logo.svg>)"));
    }

    #[test]
    fn test_prepare_content_image_extension_case_insensitive() {
        let result = prepare_content(Path::new("photo.PNG"), "data".to_string());
        assert!(result.contains("![photo.PNG]"));
    }

    #[test]
    fn test_image_markdown_with_spaces_parses_as_image() {
        let md = image_markdown(Path::new("image support.png"));
        let doc = Document::parse(&md).unwrap();
        assert!(
            !doc.images().is_empty(),
            "Filename with spaces must parse as image, got markdown: {md}"
        );
    }

    #[test]
    fn test_image_markdown_with_parens_parses_as_image() {
        let md = image_markdown(Path::new("photo (1).jpg"));
        let doc = Document::parse(&md).unwrap();
        assert!(
            !doc.images().is_empty(),
            "Filename with parens must parse as image, got markdown: {md}"
        );
    }

    #[test]
    fn test_format_hex_dump_basic() {
        let bytes = b"Hello World\x00\xff\xfe";
        let result = format_hex_dump(bytes);
        let lines: Vec<&str> = result.lines().collect();
        // Should have offset, hex bytes, and ASCII representation
        assert!(lines[0].starts_with("00000000"));
        assert!(lines[0].contains("48 65 6c 6c 6f 20 57 6f"));
        assert!(lines[0].contains("72 6c 64 00 ff fe"));
        assert!(lines[0].contains("|Hello World...|"));
    }

    #[test]
    fn test_format_hex_dump_full_line_16_bytes() {
        let bytes: Vec<u8> = (0x41..=0x50).collect(); // A through P
        let result = format_hex_dump(&bytes);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("00000000"));
        assert!(lines[0].contains("41 42 43 44 45 46 47 48"));
        assert!(lines[0].contains("49 4a 4b 4c 4d 4e 4f 50"));
        assert!(lines[0].contains("|ABCDEFGHIJKLMNOP|"));
    }

    #[test]
    fn test_format_hex_dump_multiple_lines() {
        let bytes = vec![0x61u8; 32]; // 32 'a' bytes -> 2 lines
        let result = format_hex_dump(&bytes);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("00000000"));
        assert!(lines[1].starts_with("00000010"));
    }

    #[test]
    fn test_format_hex_dump_empty() {
        let result = format_hex_dump(b"");
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_hex_dump_non_printable_shows_dot() {
        let bytes = &[0x01, 0x02, 0x7f, 0x80];
        let result = format_hex_dump(bytes);
        assert!(result.contains("|....|"));
    }

    #[test]
    fn test_is_binary_with_null_bytes() {
        assert!(is_binary(b"hello\x00world"));
    }

    #[test]
    fn test_is_binary_with_valid_utf8() {
        assert!(!is_binary(b"hello world"));
    }

    #[test]
    fn test_is_binary_with_invalid_utf8() {
        assert!(is_binary(&[0xff, 0xfe, 0x00]));
    }

    #[test]
    fn test_is_binary_empty() {
        assert!(!is_binary(b""));
    }

    #[test]
    fn test_is_binary_with_utf8_multibyte() {
        assert!(!is_binary("héllo wörld".as_bytes()));
    }

    #[test]
    fn test_prepare_content_from_bytes_text_delegates_to_prepare_content() {
        let bytes = b"fn main() {}".to_vec();
        let result = prepare_content_from_bytes(Path::new("main.rs"), bytes);
        // Should wrap in Rust code block just like prepare_content
        assert!(result.starts_with("```Rust\n"));
        assert!(result.contains("fn main() {}"));
    }

    #[test]
    fn test_prepare_content_from_bytes_binary_shows_hex_dump() {
        let bytes = vec![0x00, 0x01, 0x02, 0xff];
        let result = prepare_content_from_bytes(Path::new("data.bin"), bytes);
        // Should have a heading with file name
        assert!(result.contains("data.bin"));
        // Should have hex dump wrapped in a code block
        assert!(result.contains("```"));
        assert!(result.contains("00 01 02 ff"));
    }

    #[test]
    fn test_prepare_content_from_bytes_binary_shows_file_size() {
        let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let result = prepare_content_from_bytes(Path::new("test.bin"), bytes);
        assert!(result.contains("4 bytes"));
    }

    #[test]
    fn test_format_single_hex_line_basic() {
        let chunk = b"Hello World\x00\xff\xfe";
        let line = format_single_hex_line(chunk, 0);
        assert!(line.starts_with("00000000"));
        assert!(line.contains("48 65 6c 6c 6f 20 57 6f"));
        assert!(line.contains("72 6c 64 00 ff fe"));
        assert!(line.contains("|Hello World...|"));
    }

    #[test]
    fn test_format_single_hex_line_with_offset() {
        let chunk: Vec<u8> = (0x41..=0x50).collect(); // A through P
        let line = format_single_hex_line(&chunk, 0x100);
        assert!(line.starts_with("00000100"));
        assert!(line.contains("|ABCDEFGHIJKLMNOP|"));
    }

    #[test]
    fn test_format_single_hex_line_partial_chunk() {
        let chunk = &[0x41, 0x42, 0x43]; // ABC, only 3 bytes
        let line = format_single_hex_line(chunk, 0);
        assert!(line.contains("41 42 43"));
        assert!(line.contains("|ABC|"));
    }

    #[test]
    fn test_prepare_content_from_bytes_image_file_shows_image() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic
        let result = prepare_content_from_bytes(Path::new("photo.png"), bytes);
        // Image files should still be handled as images, not hex
        assert!(result.contains("![photo.png]"));
    }

    #[test]
    fn test_prepare_document_from_bytes_binary() {
        let bytes = vec![0x00, 0x01, 0x02, 0xff];
        let doc = prepare_document_from_bytes(Path::new("data.bin"), bytes, 80);
        assert!(doc.is_hex_mode());
        // Header (4) + 1 hex line
        assert_eq!(doc.line_count(), 5);
        // Heading should contain the filename
        let heading = doc.line_at(0).unwrap();
        assert!(heading.content().contains("data.bin"));
    }

    #[test]
    fn test_prepare_document_from_bytes_text() {
        let bytes = b"# Hello\n\nWorld".to_vec();
        let doc = prepare_document_from_bytes(Path::new("README.md"), bytes, 80);
        assert!(!doc.is_hex_mode());
        assert!(doc.line_count() > 0);
    }

    #[test]
    fn test_prepare_document_from_bytes_image() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic
        let doc = prepare_document_from_bytes(Path::new("photo.png"), bytes, 80);
        assert!(!doc.is_hex_mode());
        assert!(!doc.images().is_empty());
    }

    #[test]
    fn test_large_binary_file_loads_fast() {
        // 10MB binary file should create a hex document without hanging
        let bytes = vec![0xABu8; 10 * 1024 * 1024];
        let start = std::time::Instant::now();
        let doc = prepare_document_from_bytes(Path::new("large.bin"), bytes, 80);
        let elapsed = start.elapsed();
        assert!(doc.is_hex_mode());
        // 10MB / 16 = 655360 hex lines + 4 header
        assert_eq!(doc.line_count(), 4 + 655360);
        // Should complete in well under 1 second (no hex string generation)
        assert!(
            elapsed.as_millis() < 1000,
            "Loading 10MB binary took {}ms, expected < 1000ms",
            elapsed.as_millis()
        );
    }
}
