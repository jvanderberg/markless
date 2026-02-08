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
    let Some(language) = crate::highlight::language_for_file(file_path) else {
        return content;
    };
    format!("```{language}\n{content}\n```")
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
}
