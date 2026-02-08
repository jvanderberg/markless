//! Core document types.

use std::collections::HashMap;
use std::ops::Range;

/// A parsed and rendered markdown document.
#[derive(Debug, Clone)]
pub struct Document {
    /// Original source text
    source: String,
    /// Rendered lines for display
    lines: Vec<RenderedLine>,
    /// Heading references for TOC
    headings: Vec<HeadingRef>,
    /// Image references
    images: Vec<ImageRef>,
    /// Link references
    links: Vec<LinkRef>,
    /// Footnote definition lines by label
    footnotes: HashMap<String, usize>,
    /// Code blocks for lazy syntax highlighting
    code_blocks: Vec<CodeBlockRef>,
    /// Mermaid diagram sources keyed by synthetic image src (e.g. `mermaid://0`)
    mermaid_sources: HashMap<String, String>,
}

impl Document {
    /// Create an empty document.
    pub fn empty() -> Self {
        Self {
            source: String::new(),
            lines: Vec::new(),
            headings: Vec::new(),
            images: Vec::new(),
            links: Vec::new(),
            footnotes: HashMap::new(),
            code_blocks: Vec::new(),
            mermaid_sources: HashMap::new(),
        }
    }

    /// Create a new document with the given content.
    pub(crate) fn new(
        source: String,
        lines: Vec<RenderedLine>,
        headings: Vec<HeadingRef>,
        images: Vec<ImageRef>,
        links: Vec<LinkRef>,
        footnotes: HashMap<String, usize>,
        code_blocks: Vec<CodeBlockRef>,
        mermaid_sources: HashMap<String, String>,
    ) -> Self {
        Self {
            source,
            lines,
            headings,
            images,
            links,
            footnotes,
            code_blocks,
            mermaid_sources,
        }
    }

    /// Get the total number of rendered lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get all headings for TOC.
    pub fn headings(&self) -> &[HeadingRef] {
        &self.headings
    }

    /// Get all image references.
    pub fn images(&self) -> &[ImageRef] {
        &self.images
    }

    /// Get all link references.
    pub fn links(&self) -> &[LinkRef] {
        &self.links
    }

    /// Get mermaid diagram sources keyed by synthetic image src.
    pub const fn mermaid_sources(&self) -> &HashMap<String, String> {
        &self.mermaid_sources
    }

    pub fn footnote_line(&self, name: &str) -> Option<usize> {
        self.footnotes.get(name).copied()
    }

    pub fn resolve_internal_anchor(&self, anchor: &str) -> Option<usize> {
        let target = anchor.trim();
        if target.is_empty() {
            return None;
        }
        let normalized = normalize_anchor(target);
        self.headings.iter().find_map(|h| {
            if h.id.as_deref().is_some_and(|id| id == target) {
                return Some(h.line);
            }
            let slug = normalize_anchor(&h.text);
            if slug == normalized {
                Some(h.line)
            } else {
                None
            }
        })
    }

    /// Get visible lines for rendering.
    ///
    /// Returns lines from `offset` to `offset + count`.
    pub fn visible_lines(&self, offset: usize, count: usize) -> Vec<&RenderedLine> {
        self.lines.iter().skip(offset).take(count).collect()
    }

    /// Get a specific rendered line by index.
    pub fn line_at(&self, index: usize) -> Option<&RenderedLine> {
        self.lines.get(index)
    }

    /// Get the source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Lazily apply syntax highlighting to code blocks intersecting `range`.
    pub fn ensure_highlight_for_range(&mut self, range: Range<usize>) {
        for block in self.code_blocks.iter_mut() {
            if block.highlighted
                || block.line_range.end <= range.start
                || block.line_range.start >= range.end
            {
                continue;
            }

            let highlighted = crate::highlight::highlight_code(
                block.language.as_deref(),
                &block.raw_lines.join("\n"),
            );

            for (line_idx, spans) in
                (block.line_range.start..block.line_range.end).zip(highlighted.into_iter())
            {
                let trimmed_spans = truncate_spans_to_chars(&spans, block.content_width);
                let trimmed_len = spans_char_len(&trimmed_spans);
                let padding = " "
                    .repeat(block.content_width.saturating_sub(trimmed_len) + block.right_padding);

                let mut line_spans = Vec::new();
                line_spans.push(InlineSpan::new("│ ".to_string(), InlineStyle::default()));
                line_spans.extend(trimmed_spans);
                line_spans.push(InlineSpan::new(
                    format!("{} │", padding),
                    InlineStyle::default(),
                ));
                let content = spans_to_string(&line_spans);
                self.lines[line_idx] =
                    RenderedLine::with_spans(content, LineType::CodeBlock, line_spans);
            }

            block.highlighted = true;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeBlockRef {
    pub line_range: Range<usize>,
    pub language: Option<String>,
    pub raw_lines: Vec<String>,
    pub highlighted: bool,
    pub content_width: usize,
    pub right_padding: usize,
}

/// A single rendered line with styling information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    /// The text content of the line
    content: String,
    /// The type of line (for styling)
    line_type: LineType,
    /// Optional source range in original markdown
    source_range: Option<Range<usize>>,
    /// Optional inline-styled spans for rendering
    spans: Vec<InlineSpan>,
}

impl RenderedLine {
    /// Create a new rendered line.
    pub fn new(content: String, line_type: LineType) -> Self {
        Self {
            content,
            line_type,
            source_range: None,
            spans: Vec::new(),
        }
    }

    /// Create a new rendered line with inline spans.
    pub fn with_spans(content: String, line_type: LineType, spans: Vec<InlineSpan>) -> Self {
        Self {
            content,
            line_type,
            source_range: None,
            spans,
        }
    }

    /// Create with source range.
    pub fn with_source_range(mut self, range: Range<usize>) -> Self {
        self.source_range = Some(range);
        self
    }

    /// Get the text content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the line type.
    pub fn line_type(&self) -> &LineType {
        &self.line_type
    }

    /// Get inline spans, if present.
    pub fn spans(&self) -> Option<&[InlineSpan]> {
        if self.spans.is_empty() {
            None
        } else {
            Some(&self.spans)
        }
    }

    /// Get as string slice.
    pub fn as_str(&self) -> &str {
        &self.content
    }
}

/// Inline style flags for a text span.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub strikethrough: bool,
    pub link: bool,
    pub fg: Option<InlineColor>,
    pub bg: Option<InlineColor>,
}

/// RGB color for inline styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A styled inline span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    text: String,
    style: InlineStyle,
}

impl InlineSpan {
    pub fn new(text: String, style: InlineStyle) -> Self {
        Self { text, style }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(&self) -> InlineStyle {
        self.style
    }
}

/// Type of a rendered line, used for styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    /// Normal paragraph text
    Paragraph,
    /// Heading with level (1-6)
    Heading(u8),
    /// Code block line
    CodeBlock,
    /// Block quote line
    BlockQuote,
    /// List item with nesting level
    ListItem(usize),
    /// Table row
    Table,
    /// Horizontal rule
    HorizontalRule,
    /// Image placeholder
    Image,
    /// Empty line
    Empty,
}

/// Reference to a heading in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingRef {
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text (plain, no formatting)
    pub text: String,
    /// Line number in rendered document
    pub line: usize,
    /// Optional heading ID (for anchors)
    pub id: Option<String>,
}

/// Reference to an image in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// Alt text
    pub alt: String,
    /// Image source (path or URL)
    pub src: String,
    /// Line range in rendered document
    pub line_range: Range<usize>,
}

/// Reference to a link in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRef {
    /// Link text
    pub text: String,
    /// Link URL
    pub url: String,
    /// Line number in rendered document
    pub line: usize,
}

fn spans_to_string(spans: &[InlineSpan]) -> String {
    let mut content = String::new();
    for span in spans {
        content.push_str(span.text());
    }
    content
}

fn spans_char_len(spans: &[InlineSpan]) -> usize {
    spans.iter().map(|s| s.text().chars().count()).sum()
}

fn normalize_anchor(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in s.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn truncate_spans_to_chars(spans: &[InlineSpan], max_len: usize) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let mut remaining = max_len;
    for span in spans {
        if remaining == 0 {
            break;
        }
        let mut taken = String::new();
        for ch in span.text().chars().take(remaining) {
            taken.push(ch);
        }
        let count = taken.chars().count();
        if count > 0 {
            out.push(InlineSpan::new(taken, span.style()));
            remaining -= count;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let doc = Document::empty();
        assert_eq!(doc.line_count(), 0);
        assert!(doc.headings().is_empty());
    }

    #[test]
    fn test_rendered_line_content() {
        let line = RenderedLine::new("Hello".to_string(), LineType::Paragraph);
        assert_eq!(line.content(), "Hello");
        assert_eq!(line.as_str(), "Hello");
    }

    #[test]
    fn test_rendered_line_type() {
        let line = RenderedLine::new("# Heading".to_string(), LineType::Heading(1));
        assert_eq!(line.line_type(), &LineType::Heading(1));
    }

    #[test]
    fn test_visible_lines() {
        let lines = vec![
            RenderedLine::new("Line 1".to_string(), LineType::Paragraph),
            RenderedLine::new("Line 2".to_string(), LineType::Paragraph),
            RenderedLine::new("Line 3".to_string(), LineType::Paragraph),
            RenderedLine::new("Line 4".to_string(), LineType::Paragraph),
            RenderedLine::new("Line 5".to_string(), LineType::Paragraph),
        ];
        let doc = Document::new(
            "source".to_string(),
            lines,
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![],
            HashMap::new(),
        );

        let visible = doc.visible_lines(1, 2);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].content(), "Line 2");
        assert_eq!(visible[1].content(), "Line 3");
    }

    #[test]
    fn test_visible_lines_beyond_end() {
        let lines = vec![
            RenderedLine::new("Line 1".to_string(), LineType::Paragraph),
            RenderedLine::new("Line 2".to_string(), LineType::Paragraph),
        ];
        let doc = Document::new(
            "source".to_string(),
            lines,
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![],
            HashMap::new(),
        );

        let visible = doc.visible_lines(0, 10);
        assert_eq!(visible.len(), 2);
    }
}
