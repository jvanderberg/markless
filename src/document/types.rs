//! Core document types.

use std::collections::HashMap;
use std::ops::Range;

/// Result of parsing markdown, ready to be assembled into a `Document`.
#[derive(Debug, Clone, Default)]
pub struct ParsedDocument {
    /// Rendered lines for display
    pub lines: Vec<RenderedLine>,
    /// Heading references for TOC
    pub headings: Vec<HeadingRef>,
    /// Image references
    pub images: Vec<ImageRef>,
    /// Link references
    pub links: Vec<LinkRef>,
    /// Footnote definition lines by label
    pub footnotes: HashMap<String, usize>,
    /// Code blocks for lazy syntax highlighting
    pub code_blocks: Vec<CodeBlockRef>,
    /// Mermaid diagram sources keyed by synthetic image src
    pub mermaid_sources: HashMap<String, String>,
}

/// Backing store for lazy hex dump rendering.
#[derive(Debug, Clone)]
pub struct HexData {
    /// Raw file bytes
    bytes: Vec<u8>,
    /// Number of header lines (heading, blank, size, blank)
    header_line_count: usize,
    /// Cached rendered hex lines: (`start_index`, lines)
    cached_range: Option<(usize, Vec<RenderedLine>)>,
}

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
    /// Optional hex data for lazy binary file rendering
    hex_data: Option<HexData>,
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
            hex_data: None,
        }
    }

    /// Create a document from plain text, rendering each line verbatim.
    ///
    /// Used for non-markdown files where line breaks should be preserved
    /// exactly as they appear in the source.
    pub fn from_plain_text(source: &str) -> Self {
        let lines: Vec<RenderedLine> = source
            .lines()
            .map(|line| RenderedLine::new(line.to_string(), LineType::Paragraph))
            .collect();
        Self {
            source: source.to_string(),
            lines,
            headings: Vec::new(),
            images: Vec::new(),
            links: Vec::new(),
            footnotes: HashMap::new(),
            code_blocks: Vec::new(),
            mermaid_sources: HashMap::new(),
            hex_data: None,
        }
    }

    /// Create a new document from parsed results.
    pub(crate) fn from_parsed(source: String, result: ParsedDocument) -> Self {
        Self {
            source,
            lines: result.lines,
            headings: result.headings,
            images: result.images,
            links: result.links,
            footnotes: result.footnotes,
            code_blocks: result.code_blocks,
            mermaid_sources: result.mermaid_sources,
            hex_data: None,
        }
    }

    /// Create a hex document from raw bytes.
    ///
    /// Stores header lines (heading + size info) in `self.lines` and raw bytes
    /// in `hex_data` for lazy hex line generation on demand.
    pub fn from_hex(file_name: &str, bytes: Vec<u8>) -> Self {
        let size = bytes.len();
        let mut lines = Vec::new();
        let headings = vec![HeadingRef {
            level: 1,
            text: file_name.to_string(),
            line: 0,
            id: None,
        }];
        lines.push(RenderedLine::new(
            file_name.to_string(),
            LineType::Heading(1),
        ));
        // Line 1: blank
        lines.push(RenderedLine::new(String::new(), LineType::Empty));
        // Line 2: size info
        lines.push(RenderedLine::new(
            format!("Binary file — {size} bytes"),
            LineType::Paragraph,
        ));
        // Line 3: blank
        lines.push(RenderedLine::new(String::new(), LineType::Empty));

        let header_line_count = lines.len();

        Self {
            source: String::new(),
            lines,
            headings,
            images: Vec::new(),
            links: Vec::new(),
            footnotes: HashMap::new(),
            code_blocks: Vec::new(),
            mermaid_sources: HashMap::new(),
            hex_data: Some(HexData {
                bytes,
                header_line_count,
                cached_range: None,
            }),
        }
    }

    /// Get the total number of rendered lines.
    pub fn line_count(&self) -> usize {
        self.hex_data.as_ref().map_or(self.lines.len(), |hex| {
            hex.header_line_count + hex.bytes.len().div_ceil(16)
        })
    }

    /// Returns true if this document uses lazy hex rendering.
    pub const fn is_hex_mode(&self) -> bool {
        self.hex_data.is_some()
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
        if self.hex_data.is_some() {
            let total = self.line_count();
            (offset..total)
                .take(count)
                .filter_map(|i| self.line_at(i))
                .collect()
        } else {
            self.lines.iter().skip(offset).take(count).collect()
        }
    }

    /// Get a specific rendered line by index.
    pub fn line_at(&self, index: usize) -> Option<&RenderedLine> {
        if let Some(hex) = &self.hex_data {
            if index < hex.header_line_count {
                return self.lines.get(index);
            }
            let hex_idx = index - hex.header_line_count;
            if let Some((cache_start, ref cached)) = hex.cached_range
                && hex_idx >= cache_start
                && hex_idx < cache_start + cached.len()
            {
                return cached.get(hex_idx - cache_start);
            }
            None
        } else {
            self.lines.get(index)
        }
    }

    /// Get the source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Generate the text content for a hex line at the given document index.
    ///
    /// Returns the header line content for header indices, or generates the
    /// hex dump line on the fly for hex indices. Returns `None` if out of range.
    /// Unlike `line_at`, this does not require the cache to be populated.
    pub fn hex_line_content(&self, index: usize) -> Option<String> {
        let hex = self.hex_data.as_ref()?;
        if index < hex.header_line_count {
            return self.lines.get(index).map(|l| l.content().to_string());
        }
        let hex_idx = index - hex.header_line_count;
        let byte_offset = hex_idx * 16;
        if byte_offset >= hex.bytes.len() {
            return None;
        }
        let byte_end = (byte_offset + 16).min(hex.bytes.len());
        let chunk = &hex.bytes[byte_offset..byte_end];
        Some(super::format_single_hex_line(chunk, byte_offset))
    }

    /// Ensure hex lines are cached for the given document line range.
    ///
    /// Generates hex dump lines for the viewport plus a buffer on each side.
    /// The range is in terms of overall document line indices (including headers).
    ///
    pub fn ensure_hex_lines_for_range(&mut self, range: Range<usize>) {
        let Some(hex) = &self.hex_data else { return };
        let header = hex.header_line_count;
        let total_hex_lines = hex.bytes.len().div_ceil(16);
        if total_hex_lines == 0 {
            return;
        }

        // Convert document line range to hex line indices
        let hex_start = range.start.saturating_sub(header);
        let hex_end = range.end.saturating_sub(header).min(total_hex_lines);
        if hex_start >= hex_end {
            return;
        }

        // Add a 200-line buffer on each side
        let buffer = 200;
        let cache_start = hex_start.saturating_sub(buffer);
        let cache_end = (hex_end + buffer).min(total_hex_lines);

        // Check if current cache already covers the requested range
        if let Some((cs, ref cached)) = hex.cached_range
            && cs <= hex_start
            && cs + cached.len() >= hex_end
        {
            return;
        }
        // NLL ends the immutable borrow of `hex` above here.

        // Generate the cached lines
        let mut cached = Vec::with_capacity(cache_end - cache_start);
        let Some(hex) = &self.hex_data else { return };
        for hex_idx in cache_start..cache_end {
            let byte_offset = hex_idx * 16;
            let byte_end = (byte_offset + 16).min(hex.bytes.len());
            let chunk = &hex.bytes[byte_offset..byte_end];
            let text = super::format_single_hex_line(chunk, byte_offset);
            cached.push(RenderedLine::new(text, LineType::CodeBlock));
        }

        if let Some(hex) = &mut self.hex_data {
            hex.cached_range = Some((cache_start, cached));
        }
    }

    /// Lazily apply syntax highlighting to code blocks intersecting `range`.
    pub fn ensure_highlight_for_range(&mut self, range: Range<usize>) {
        for block in &mut self.code_blocks {
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
                if line_idx >= self.lines.len() {
                    break;
                }
                let trimmed_spans = truncate_spans_to_chars(&spans, block.content_width);
                let trimmed_len = spans_char_len(&trimmed_spans);
                let padding = " "
                    .repeat(block.content_width.saturating_sub(trimmed_len) + block.right_padding);

                let mut line_spans = Vec::new();
                line_spans.push(InlineSpan::new("│ ".to_string(), InlineStyle::default()));
                line_spans.extend(trimmed_spans);
                line_spans.push(InlineSpan::new(
                    format!("{padding} │"),
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
pub struct CodeBlockRef {
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
    pub const fn new(content: String, line_type: LineType) -> Self {
        Self {
            content,
            line_type,
            source_range: None,
            spans: Vec::new(),
        }
    }

    /// Create a new rendered line with inline spans.
    pub const fn with_spans(content: String, line_type: LineType, spans: Vec<InlineSpan>) -> Self {
        Self {
            content,
            line_type,
            source_range: None,
            spans,
        }
    }

    /// Create with source range.
    #[must_use]
    pub const fn with_source_range(mut self, range: Range<usize>) -> Self {
        self.source_range = Some(range);
        self
    }

    /// Get the text content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the line type.
    pub const fn line_type(&self) -> &LineType {
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
    pub const fn new(text: String, style: InlineStyle) -> Self {
        Self { text, style }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn style(&self) -> InlineStyle {
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
        let doc = Document::from_parsed(
            "source".to_string(),
            ParsedDocument {
                lines,
                ..ParsedDocument::default()
            },
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
        let doc = Document::from_parsed(
            "source".to_string(),
            ParsedDocument {
                lines,
                ..ParsedDocument::default()
            },
        );

        let visible = doc.visible_lines(0, 10);
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_hex_document_line_count() {
        // 48 bytes = 3 hex lines, plus header lines (heading + blank + size + blank)
        let bytes = vec![0xAB; 48];
        let doc = Document::from_hex("test.bin", bytes);
        // Header: heading, blank, size info, blank = 4 lines
        // Hex: ceil(48/16) = 3 lines
        assert_eq!(doc.line_count(), 4 + 3);
        assert!(doc.is_hex_mode());
    }

    #[test]
    fn test_hex_document_line_count_partial() {
        // 17 bytes = 2 hex lines (16 + 1)
        let bytes = vec![0xAB; 17];
        let doc = Document::from_hex("test.bin", bytes);
        assert_eq!(doc.line_count(), 4 + 2);
    }

    #[test]
    fn test_hex_document_line_at_header() {
        let bytes = vec![0xAB; 16];
        let doc = Document::from_hex("test.bin", bytes);
        // Header line 0 should be the heading
        let line = doc.line_at(0).unwrap();
        assert_eq!(*line.line_type(), LineType::Heading(1));
        assert!(line.content().contains("test.bin"));
    }

    #[test]
    fn test_hex_document_line_at_hex_after_ensure() {
        let bytes = vec![0x41; 32]; // 'A' * 32 = 2 hex lines
        let mut doc = Document::from_hex("test.bin", bytes);
        doc.ensure_hex_lines_for_range(0..10);
        // Hex line at index 4 (first after header)
        let line = doc.line_at(4).unwrap();
        assert!(line.content().contains("41 41 41 41"));
        assert_eq!(*line.line_type(), LineType::CodeBlock);
    }

    #[test]
    fn test_hex_document_visible_lines() {
        let bytes = vec![0x42; 48]; // 3 hex lines
        let mut doc = Document::from_hex("test.bin", bytes);
        doc.ensure_hex_lines_for_range(0..10);
        let visible = doc.visible_lines(0, 7);
        assert_eq!(visible.len(), 7); // 4 header + 3 hex
    }

    #[test]
    fn test_hex_document_is_not_hex_mode_for_normal() {
        let doc = Document::empty();
        assert!(!doc.is_hex_mode());
    }

    #[test]
    fn test_hex_document_empty_bytes() {
        let doc = Document::from_hex("empty.bin", vec![]);
        assert_eq!(doc.line_count(), 4); // header only, 0 hex lines
        assert!(doc.is_hex_mode());
    }

    #[test]
    fn test_hex_document_paging_with_ensure() {
        // Simulate paging: 1600 bytes = 100 hex lines + 4 header = 104 total
        let bytes = vec![0xAB; 1600];
        let mut doc = Document::from_hex("test.bin", bytes);
        assert_eq!(doc.line_count(), 104);

        // Page 1: offset 0, height 24
        doc.ensure_hex_lines_for_range(0..72);
        let v = doc.visible_lines(0, 24);
        assert_eq!(v.len(), 24, "first page should have 24 visible lines");

        // Page down: offset 24
        doc.ensure_hex_lines_for_range(0..96);
        let v = doc.visible_lines(24, 24);
        assert_eq!(v.len(), 24, "second page should have 24 visible lines");

        // Page far down: offset 80
        doc.ensure_hex_lines_for_range(32..152);
        let v = doc.visible_lines(80, 24);
        assert_eq!(v.len(), 24, "later page should have 24 visible lines");
    }

    #[test]
    fn test_hex_reflow_is_noop() {
        // Reflow (re-parsing source) should not crash for hex documents.
        // Hex documents have no meaningful source markdown.
        let doc = Document::from_hex("test.bin", vec![0x42; 16]);
        assert!(doc.is_hex_mode());
        // line_count should be stable
        assert_eq!(doc.line_count(), 5); // 4 header + 1 hex
    }
}
