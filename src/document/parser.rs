//! Markdown parsing with comrak.

use std::collections::HashMap;
use std::hash::BuildHasher;

use anyhow::Result;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::types::{
    CodeBlockRef, Document, HeadingRef, ImageRef, InlineSpan, InlineStyle, LineType, LinkRef,
    ParsedDocument, RenderedLine,
};

/// Parse markdown source into a Document.
///
/// # Example
///
/// ```
/// use markless::document::Document;
///
/// let doc = Document::parse("# Hello\n\nWorld").unwrap();
/// assert!(doc.line_count() >= 3); // heading + empty + paragraph + trailing empty
/// ```
impl Document {
    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse(source: &str) -> Result<Self> {
        parse(source)
    }

    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse_with_layout(source: &str, width: u16) -> Result<Self> {
        parse_with_layout(source, width, &HashMap::new())
    }

    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse_with_image_heights(
        source: &str,
        image_heights: &HashMap<String, usize>,
    ) -> Result<Self> {
        parse_with_image_heights(source, image_heights)
    }

    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse_with_layout_and_image_heights(
        source: &str,
        width: u16,
        image_heights: &HashMap<String, usize>,
    ) -> Result<Self> {
        parse_with_layout(source, width, image_heights)
    }

    /// Parse markdown, rendering mermaid code blocks as image placeholders.
    ///
    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse_with_mermaid_images(source: &str, width: u16) -> Result<Self> {
        Ok(parse_with_all_options(source, width, &HashMap::new(), true))
    }

    /// Parse with all options: layout width, image heights, and mermaid-as-images flag.
    ///
    /// # Errors
    /// Returns an error if markdown parsing fails.
    pub fn parse_with_all_options(
        source: &str,
        width: u16,
        image_heights: &HashMap<String, usize>,
        mermaid_as_images: bool,
    ) -> Result<Self> {
        Ok(parse_with_all_options(
            source,
            width,
            image_heights,
            mermaid_as_images,
        ))
    }
}

/// Parse markdown source into a `Document`.
///
/// # Errors
/// Returns an error if markdown parsing fails.
pub fn parse(source: &str) -> Result<Document> {
    parse_with_layout(source, 80, &HashMap::new())
}

/// Parse markdown with known image heights (in terminal rows).
///
/// # Errors
/// Returns an error if markdown parsing fails.
pub fn parse_with_image_heights<S: BuildHasher>(
    source: &str,
    image_heights: &HashMap<String, usize, S>,
) -> Result<Document> {
    parse_with_layout(source, 80, image_heights)
}

/// Parse markdown with layout width and image heights.
///
/// # Errors
/// Returns an error if markdown parsing fails.
pub fn parse_with_layout<S: BuildHasher>(
    source: &str,
    width: u16,
    image_heights: &HashMap<String, usize, S>,
) -> Result<Document> {
    Ok(parse_with_all_options(source, width, image_heights, false))
}

/// Parse markdown with all options including mermaid-as-images flag.
fn parse_with_all_options<S: BuildHasher>(
    source: &str,
    width: u16,
    image_heights: &HashMap<String, usize, S>,
    mermaid_as_images: bool,
) -> Document {
    let arena = Arena::new();
    let options = create_options();
    let root = parse_document(&arena, source, &options);

    let wrap_width = width.max(1) as usize;
    let mut ctx = ParseContext {
        lines: Vec::new(),
        headings: Vec::new(),
        images: Vec::new(),
        link_refs: Vec::new(),
        footnotes: HashMap::new(),
        code_blocks: Vec::new(),
        mermaid_sources: HashMap::new(),
        image_heights,
        wrap_width,
        mermaid_as_images,
    };
    process_node(root, &mut ctx, 0, None);

    Document::from_parsed(
        source.to_string(),
        ParsedDocument {
            lines: ctx.lines,
            headings: ctx.headings,
            images: ctx.images,
            links: ctx.link_refs,
            footnotes: ctx.footnotes,
            code_blocks: ctx.code_blocks,
            mermaid_sources: ctx.mermaid_sources,
        },
    )
}

fn create_options() -> Options {
    let mut options = Options::default();

    // Enable GFM extensions
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.superscript = true;
    options.extension.subscript = true;

    // Enable other useful extensions
    options.extension.header_ids = Some(String::new());
    options.extension.description_lists = true;

    options
}

/// Mutable context threaded through recursive node processing.
struct ParseContext<'h, S: BuildHasher = std::collections::hash_map::RandomState> {
    lines: Vec<RenderedLine>,
    headings: Vec<HeadingRef>,
    images: Vec<ImageRef>,
    link_refs: Vec<LinkRef>,
    footnotes: HashMap<String, usize>,
    code_blocks: Vec<CodeBlockRef>,
    mermaid_sources: HashMap<String, String>,
    image_heights: &'h HashMap<String, usize, S>,
    wrap_width: usize,
    mermaid_as_images: bool,
}

fn process_node<'a, S: BuildHasher>(
    node: &'a AstNode<'a>,
    ctx: &mut ParseContext<'_, S>,
    depth: usize,
    list_marker: Option<String>,
) {
    match &node.data.borrow().value {
        NodeValue::Document => {
            for child in node.children() {
                process_node(child, ctx, depth, list_marker.clone());
            }
        }

        NodeValue::Heading(heading) => {
            let text = extract_text(node);

            // Keep headings visually separated with two rows above.
            ensure_trailing_empty_lines(&mut ctx.lines, 2);
            let line_num = ctx.lines.len();

            ctx.headings.push(HeadingRef {
                level: heading.level,
                text: text.clone(),
                line: line_num,
                id: None, // TODO: Extract from header_ids
            });

            collect_inline_elements(node, line_num, &mut ctx.images, &mut ctx.link_refs);

            let prefix = "#".repeat(heading.level as usize);
            ctx.lines.push(RenderedLine::new(
                format!("{prefix} {text}"),
                LineType::Heading(heading.level),
            ));
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Paragraph => {
            // Check if paragraph contains only an image (common case)
            let child_images = collect_paragraph_images(node);

            if child_images.is_empty() {
                // Regular paragraph text with inline styling and wrapping
                let spans = collect_inline_spans(node);
                // Collect links with a placeholder line number (will be fixed up after wrapping)
                let link_start = ctx.link_refs.len();
                collect_inline_elements(node, 0, &mut ctx.images, &mut ctx.link_refs);

                let base_line = ctx.lines.len();
                let wrapped = wrap_spans(&spans, ctx.wrap_width, "", "");
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    ctx.lines.push(RenderedLine::with_spans(
                        content,
                        LineType::Paragraph,
                        line_spans,
                    ));
                }

                // Fix up link line numbers: find which wrapped line contains each link's text
                fixup_link_lines(
                    &mut ctx.link_refs[link_start..],
                    &ctx.lines[base_line..],
                    base_line,
                );
            } else {
                for (alt, src) in child_images {
                    let height_lines = ctx.image_heights.get(&src).copied().unwrap_or(1).max(1);
                    let has_caption = ctx.image_heights.contains_key(&src) && !alt.is_empty();
                    let start_line = ctx.lines.len();
                    let label = format!("[Image: {}]", if alt.is_empty() { &src } else { &alt });

                    if has_caption {
                        ctx.lines
                            .push(RenderedLine::new(format!("    {alt}"), LineType::Image));
                    }

                    // First line shows the image placeholder/alt text
                    ctx.lines
                        .push(RenderedLine::new(label.clone(), LineType::Image));
                    ctx.link_refs.push(LinkRef {
                        text: label,
                        url: src.clone(),
                        line: start_line + usize::from(has_caption),
                    });

                    // Reserve additional lines for image content (empty Image lines)
                    for _ in 1..height_lines {
                        ctx.lines
                            .push(RenderedLine::new(String::new(), LineType::Image));
                    }

                    let end_line = ctx.lines.len();
                    ctx.images.push(ImageRef {
                        alt: alt.clone(),
                        src: src.clone(),
                        line_range: start_line + usize::from(has_caption)..end_line,
                    });
                }
            }
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::CodeBlock(code_block) => {
            const CODE_RIGHT_PADDING: usize = 3;
            let info = code_block.info.clone();
            let literal = code_block.literal.clone();
            let language = info.split_whitespace().next().filter(|s| !s.is_empty());

            // Store mermaid diagram sources for optional image rendering.
            if language == Some("mermaid") {
                let key = format!("mermaid://{}", ctx.mermaid_sources.len());
                ctx.mermaid_sources
                    .insert(key.clone(), literal.trim_end().to_string());

                if ctx.mermaid_as_images {
                    // Emit as an image placeholder instead of a code block.
                    let height_lines = ctx.image_heights.get(&key).copied().unwrap_or(1).max(1);
                    let has_caption = ctx.image_heights.contains_key(&key);
                    let start_line = ctx.lines.len();
                    let label = "[Image: mermaid diagram]".to_string();

                    if has_caption {
                        ctx.lines.push(RenderedLine::new(
                            "    mermaid diagram".to_string(),
                            LineType::Image,
                        ));
                    }

                    ctx.lines
                        .push(RenderedLine::new(label.clone(), LineType::Image));
                    ctx.link_refs.push(LinkRef {
                        text: label,
                        url: key.clone(),
                        line: start_line + usize::from(has_caption),
                    });

                    for _ in 1..height_lines {
                        ctx.lines
                            .push(RenderedLine::new(String::new(), LineType::Image));
                    }

                    let end_line = ctx.lines.len();
                    ctx.images.push(ImageRef {
                        alt: "mermaid diagram".to_string(),
                        src: key,
                        line_range: start_line + usize::from(has_caption)..end_line,
                    });
                    ctx.lines
                        .push(RenderedLine::new(String::new(), LineType::Empty));
                    // Skip the normal code block rendering below.
                    return;
                }
            }

            // Render CSV code blocks as tables instead of code blocks
            if language == Some("csv") {
                let csv_lines = render_csv_as_table(&literal);
                if !csv_lines.is_empty() {
                    ctx.lines.extend(csv_lines);
                    ctx.lines
                        .push(RenderedLine::new(String::new(), LineType::Empty));
                    return;
                }
                // Fall through to normal code block rendering if CSV parsing fails
            }
            let content_width = literal
                .lines()
                .map(UnicodeWidthStr::width)
                .max()
                .unwrap_or(0)
                .min(ctx.wrap_width.saturating_sub(4).max(1));
            let title = language.unwrap_or("code");
            let label = format!(" {title} ");
            let frame_inner_width = content_width + 2 + CODE_RIGHT_PADDING;
            let top_label_width = frame_inner_width.min(UnicodeWidthStr::width(label.as_str()));
            let visible_label: String = label.chars().take(top_label_width).collect();
            let top = format!(
                "┌{}{}┐",
                visible_label,
                "─".repeat(
                    frame_inner_width
                        .saturating_sub(UnicodeWidthStr::width(visible_label.as_str()))
                )
            );
            ctx.lines.push(RenderedLine::new(top, LineType::CodeBlock));

            let body_start = ctx.lines.len();
            let raw_lines: Vec<String> = literal.lines().map(ToString::to_string).collect();
            for raw_line in &raw_lines {
                let plain_style = InlineStyle {
                    code: true,
                    ..InlineStyle::default()
                };
                let spans = vec![InlineSpan::new(raw_line.clone(), plain_style)];
                let trimmed_spans = truncate_spans(&spans, content_width);
                let trimmed_len = UnicodeWidthStr::width(spans_to_string(&trimmed_spans).as_str());
                let padding =
                    " ".repeat(content_width.saturating_sub(trimmed_len) + CODE_RIGHT_PADDING);

                let mut line_spans = Vec::new();
                line_spans.push(InlineSpan::new("│ ".to_string(), InlineStyle::default()));
                line_spans.extend(trimmed_spans);
                line_spans.push(InlineSpan::new(
                    format!("{padding} │"),
                    InlineStyle::default(),
                ));
                let content = spans_to_string(&line_spans);
                ctx.lines.push(RenderedLine::with_spans(
                    content,
                    LineType::CodeBlock,
                    line_spans,
                ));
            }
            let body_end = ctx.lines.len();

            ctx.code_blocks.push(CodeBlockRef {
                line_range: body_start..body_end,
                language: language.map(ToString::to_string),
                raw_lines,
                highlighted: false,
                content_width,
                right_padding: CODE_RIGHT_PADDING,
            });

            ctx.lines.push(RenderedLine::new(
                format!("└{}┘", "─".repeat(frame_inner_width)),
                LineType::CodeBlock,
            ));
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::List(list) => {
            let list_depth = depth + 1;
            let start = list.start;
            let delimiter = match list.delimiter {
                comrak::nodes::ListDelimType::Paren => ')',
                comrak::nodes::ListDelimType::Period => '.',
            };
            let list_len = node.children().count();
            let max_number = start + list_len.saturating_sub(1);
            let number_width = max_number.to_string().len();

            for (index, child) in node.children().enumerate() {
                let base_marker = match list.list_type {
                    comrak::nodes::ListType::Bullet => "•".to_string(),
                    comrak::nodes::ListType::Ordered => {
                        let number = start + index;
                        format!("{number:>number_width$}{delimiter}")
                    }
                };
                let marker = format!("{base_marker} ");
                process_node(child, ctx, list_depth, Some(marker));
            }
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::TaskItem(symbol) => {
            let indent = "  ".repeat(depth.saturating_sub(1));
            let task_marker = if symbol.is_some() { "✓" } else { "□" };
            let marker = format!("{task_marker} ");
            let prefix_first = format!("{indent}{marker}");
            let prefix_next = format!("{}{}", indent, " ".repeat(marker.len()));

            let spans = collect_inline_spans(node);
            let link_start = ctx.link_refs.len();
            collect_inline_elements(node, 0, &mut ctx.images, &mut ctx.link_refs);
            let base_line = ctx.lines.len();
            let wrapped = wrap_spans(&spans, ctx.wrap_width, &prefix_first, &prefix_next);
            for line_spans in wrapped {
                let content = spans_to_string(&line_spans);
                ctx.lines.push(RenderedLine::with_spans(
                    content,
                    LineType::ListItem(depth),
                    line_spans,
                ));
            }
            fixup_link_lines(
                &mut ctx.link_refs[link_start..],
                &ctx.lines[base_line..],
                base_line,
            );

            for child in node.children() {
                if matches!(child.data.borrow().value, NodeValue::List(_)) {
                    process_node(child, ctx, depth, None);
                }
            }
        }

        NodeValue::Item(_) => {
            let indent = "  ".repeat(depth.saturating_sub(1));
            let base_marker = list_marker.unwrap_or_else(|| "- ".to_string());
            let task_marker = find_task_marker(node);
            let marker = task_marker.map_or(base_marker, |tm| format!("{tm} "));
            let prefix_first = format!("{indent}{marker}");
            let prefix_next = format!("{}{}", indent, " ".repeat(marker.len()));
            let mut rendered_any = false;
            let mut rendered_paragraphs = 0usize;

            for child in node.children() {
                match &child.data.borrow().value {
                    NodeValue::Paragraph | NodeValue::TaskItem(_) => {
                        if rendered_paragraphs > 0 {
                            ctx.lines
                                .push(RenderedLine::new(String::new(), LineType::ListItem(depth)));
                        }
                        let spans = collect_inline_spans(child);
                        let link_start = ctx.link_refs.len();
                        collect_inline_elements(child, 0, &mut ctx.images, &mut ctx.link_refs);
                        let prefix = if rendered_any {
                            &prefix_next
                        } else {
                            &prefix_first
                        };
                        let base_line = ctx.lines.len();
                        let wrapped = wrap_spans(&spans, ctx.wrap_width, prefix, &prefix_next);

                        for line_spans in wrapped {
                            let content = spans_to_string(&line_spans);
                            ctx.lines.push(RenderedLine::with_spans(
                                content,
                                LineType::ListItem(depth),
                                line_spans,
                            ));
                        }
                        fixup_link_lines(
                            &mut ctx.link_refs[link_start..],
                            &ctx.lines[base_line..],
                            base_line,
                        );
                        rendered_any = true;
                        rendered_paragraphs += 1;
                    }
                    _ => {
                        process_node(child, ctx, depth, None);
                    }
                }
            }

            if !rendered_any {
                let spans = collect_inline_spans(node);
                let link_start = ctx.link_refs.len();
                collect_inline_elements(node, 0, &mut ctx.images, &mut ctx.link_refs);
                let base_line = ctx.lines.len();
                let wrapped = wrap_spans(&spans, ctx.wrap_width, &prefix_first, &prefix_next);
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    ctx.lines.push(RenderedLine::with_spans(
                        content,
                        LineType::ListItem(depth),
                        line_spans,
                    ));
                }
                fixup_link_lines(
                    &mut ctx.link_refs[link_start..],
                    &ctx.lines[base_line..],
                    base_line,
                );
            }
        }

        NodeValue::BlockQuote => {
            render_blockquote(
                node,
                &mut ctx.lines,
                &mut ctx.link_refs,
                &mut ctx.images,
                ctx.wrap_width,
                1,
            );
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::ThematicBreak => {
            ctx.lines.push(RenderedLine::new(
                "─────".to_string(),
                LineType::HorizontalRule,
            ));
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Table(_) => {
            let base_line = ctx.lines.len();
            let link_start = ctx.link_refs.len();
            let table_lines = render_table(node, &mut ctx.link_refs, &mut ctx.images);
            ctx.lines.extend(table_lines);
            // Fix up link lines relative to where the table was placed
            for link in &mut ctx.link_refs[link_start..] {
                link.line += base_line;
            }
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::FootnoteDefinition(def) => {
            let line_num = ctx.lines.len();
            ctx.footnotes.insert(def.name.clone(), line_num);
            let label = format!("{} ", render_footnote_reference(&def.name));
            let continuation = " ".repeat(label.len());
            let spans = collect_inline_spans(node);
            let link_start = ctx.link_refs.len();
            collect_inline_elements(node, 0, &mut ctx.images, &mut ctx.link_refs);
            let base_line = ctx.lines.len();
            let wrapped = wrap_spans(&spans, ctx.wrap_width, &label, &continuation);
            if wrapped.is_empty() {
                ctx.lines
                    .push(RenderedLine::new(label, LineType::Paragraph));
            } else {
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    ctx.lines.push(RenderedLine::with_spans(
                        content,
                        LineType::Paragraph,
                        line_spans,
                    ));
                }
            }
            fixup_link_lines(
                &mut ctx.link_refs[link_start..],
                &ctx.lines[base_line..],
                base_line,
            );
            ctx.lines
                .push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Image(image) => {
            let alt = extract_text(node);
            let src = image.url.clone();
            let line_num = ctx.lines.len();
            let label = format!("[Image: {}]", if alt.is_empty() { &src } else { &alt });
            let height_lines = ctx.image_heights.get(&src).copied().unwrap_or(1).max(1);
            let has_caption = ctx.image_heights.contains_key(&src) && !alt.is_empty();

            ctx.images.push(ImageRef {
                alt: alt.clone(),
                src: src.clone(),
                line_range: line_num + usize::from(has_caption)
                    ..line_num + usize::from(has_caption) + height_lines,
            });

            ctx.link_refs.push(LinkRef {
                text: label.clone(),
                url: src,
                line: line_num + usize::from(has_caption),
            });

            if has_caption {
                ctx.lines
                    .push(RenderedLine::new(format!("    {alt}"), LineType::Image));
            }
            ctx.lines.push(RenderedLine::new(label, LineType::Image));

            for _ in 1..height_lines {
                ctx.lines
                    .push(RenderedLine::new(String::new(), LineType::Image));
            }
        }

        _ => {
            // Process children for unhandled nodes
            for child in node.children() {
                process_node(child, ctx, depth, list_marker.clone());
            }
        }
    }
}

fn ensure_trailing_empty_lines(lines: &mut Vec<RenderedLine>, count: usize) {
    let existing = lines
        .iter()
        .rev()
        .take_while(|line| matches!(line.line_type(), LineType::Empty))
        .count();
    for _ in existing..count {
        lines.push(RenderedLine::new(String::new(), LineType::Empty));
    }
}

fn render_blockquote<'a>(
    node: &'a AstNode<'a>,
    lines: &mut Vec<RenderedLine>,
    link_refs: &mut Vec<LinkRef>,
    images: &mut Vec<ImageRef>,
    wrap_width: usize,
    quote_depth: usize,
) {
    let prefix = quote_prefix(quote_depth);

    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Paragraph => {
                let spans = collect_inline_spans(child);
                let link_start = link_refs.len();
                collect_inline_elements(child, 0, images, link_refs);
                let base_line = lines.len();
                let wrapped = wrap_spans(&spans, wrap_width, &prefix, &prefix);
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    lines.push(RenderedLine::with_spans(
                        content,
                        LineType::BlockQuote,
                        line_spans,
                    ));
                }
                fixup_link_lines(&mut link_refs[link_start..], &lines[base_line..], base_line);
            }
            NodeValue::BlockQuote => {
                render_blockquote(child, lines, link_refs, images, wrap_width, quote_depth + 1);
            }
            _ => {
                let link_start = link_refs.len();
                collect_inline_elements(child, 0, images, link_refs);
                let text = extract_text(child);
                let base_line = lines.len();
                for raw_line in text.lines() {
                    let spans = vec![InlineSpan::new(
                        raw_line.to_string(),
                        InlineStyle::default(),
                    )];
                    let wrapped = wrap_spans(&spans, wrap_width, &prefix, &prefix);
                    for line_spans in wrapped {
                        let content = spans_to_string(&line_spans);
                        lines.push(RenderedLine::with_spans(
                            content,
                            LineType::BlockQuote,
                            line_spans,
                        ));
                    }
                }
                fixup_link_lines(&mut link_refs[link_start..], &lines[base_line..], base_line);
            }
        }
    }
}

fn quote_prefix(depth: usize) -> String {
    let mut prefix = String::from("  ");
    for _ in 0..depth {
        prefix.push('│');
        prefix.push(' ');
    }
    prefix
}

#[derive(Debug, Clone)]
struct TableCellRender {
    text: String,
    spans: Vec<InlineSpan>,
}

fn render_table<'a>(
    table_node: &'a AstNode<'a>,
    link_refs: &mut Vec<LinkRef>,
    images: &mut Vec<ImageRef>,
) -> Vec<RenderedLine> {
    let CollectedTableRows {
        alignments,
        mut rows,
        has_header,
        row_link_indices: row_links,
    } = collect_table_rows(table_node, link_refs, images);
    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows.iter().map(std::vec::Vec::len).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }

    for row in &mut rows {
        while row.len() < num_cols {
            row.push(TableCellRender {
                text: String::new(),
                spans: Vec::new(),
            });
        }
    }

    let mut col_widths = vec![1_usize; num_cols];
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            col_widths[idx] = col_widths[idx].max(display_width(&cell.text));
        }
    }

    let mid = render_table_inner_divider(&col_widths);

    let mut lines = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let output_line = lines.len();
        lines.push(render_table_row(row, &col_widths, &alignments));
        // Assign the correct output line to links collected from this row
        for link_idx in &row_links[idx] {
            link_refs[*link_idx].line = output_line;
        }
        if has_header && idx == 0 {
            lines.push(RenderedLine::new(mid.clone(), LineType::Table));
        }
    }
    lines
}

struct CollectedTableRows {
    alignments: Vec<TableAlignment>,
    rows: Vec<Vec<TableCellRender>>,
    has_header: bool,
    /// For each row, the indices into `link_refs` that belong to that row.
    row_link_indices: Vec<Vec<usize>>,
}

fn collect_table_rows<'a>(
    table_node: &'a AstNode<'a>,
    link_refs: &mut Vec<LinkRef>,
    images: &mut Vec<ImageRef>,
) -> CollectedTableRows {
    let alignments = match &table_node.data.borrow().value {
        NodeValue::Table(table) => table.alignments.clone(),
        _ => Vec::new(),
    };

    let mut rows = Vec::new();
    let mut has_header = false;
    let mut row_links: Vec<Vec<usize>> = Vec::new();
    for row_node in table_node.children() {
        let is_header_row = matches!(row_node.data.borrow().value, NodeValue::TableRow(true));
        if is_header_row {
            has_header = true;
        }
        if !matches!(row_node.data.borrow().value, NodeValue::TableRow(_)) {
            continue;
        }

        let mut row_cells = Vec::new();
        let mut this_row_link_indices = Vec::new();
        for cell_node in row_node.children() {
            if !matches!(cell_node.data.borrow().value, NodeValue::TableCell) {
                continue;
            }
            let spans = normalize_inline_whitespace(&collect_inline_spans(cell_node));
            let text = spans_to_string(&spans);
            // Collect links from this cell; line will be fixed up later
            let link_start = link_refs.len();
            collect_inline_elements(cell_node, 0, images, link_refs);
            for idx in link_start..link_refs.len() {
                this_row_link_indices.push(idx);
            }
            row_cells.push(TableCellRender { text, spans });
        }
        rows.push(row_cells);
        row_links.push(this_row_link_indices);
    }

    CollectedTableRows {
        alignments,
        rows,
        has_header,
        row_link_indices: row_links,
    }
}

fn render_table_inner_divider(widths: &[usize]) -> String {
    let mut out = String::new();
    for (idx, width) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(width + 2));
        if idx + 1 < widths.len() {
            out.push('┼');
        }
    }
    out
}

fn render_table_row(
    cells: &[TableCellRender],
    widths: &[usize],
    alignments: &[TableAlignment],
) -> RenderedLine {
    let mut spans = Vec::new();
    for idx in 0..widths.len() {
        let cell = cells.get(idx).cloned().unwrap_or(TableCellRender {
            text: String::new(),
            spans: Vec::new(),
        });
        let mut content_spans = truncate_spans_by_display_width(&cell.spans, widths[idx]);
        let content_width = display_width(&spans_to_string(&content_spans));
        let padding = widths[idx].saturating_sub(content_width);

        spans.push(InlineSpan::new(" ".to_string(), InlineStyle::default()));
        match alignments.get(idx).copied().unwrap_or(TableAlignment::None) {
            TableAlignment::Right => {
                spans.push(InlineSpan::new(" ".repeat(padding), InlineStyle::default()));
                spans.append(&mut content_spans);
            }
            TableAlignment::Center => {
                let left = padding / 2;
                let right = padding - left;
                spans.push(InlineSpan::new(" ".repeat(left), InlineStyle::default()));
                spans.append(&mut content_spans);
                spans.push(InlineSpan::new(" ".repeat(right), InlineStyle::default()));
            }
            TableAlignment::Left | TableAlignment::None => {
                spans.append(&mut content_spans);
                spans.push(InlineSpan::new(" ".repeat(padding), InlineStyle::default()));
            }
        }
        if idx + 1 < widths.len() {
            spans.push(InlineSpan::new(" │".to_string(), InlineStyle::default()));
        }
    }
    let content = spans_to_string(&spans);
    RenderedLine::with_spans(content, LineType::Table, spans)
}

/// Render CSV content as table lines, reusing the existing table rendering infrastructure.
/// All rows are rendered; the TUI viewport handles paging/scrolling.
fn render_csv_as_table(csv_content: &str) -> Vec<RenderedLine> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_content.as_bytes());

    let mut rows: Vec<Vec<TableCellRender>> = Vec::new();
    for result in reader.records() {
        let Ok(record) = result else { continue };
        let cells: Vec<TableCellRender> = record
            .iter()
            .map(|field| {
                let text = field.trim().to_string();
                let spans = vec![InlineSpan::new(text.clone(), InlineStyle::default())];
                TableCellRender { text, spans }
            })
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }

    // Pad rows to have equal column counts
    for row in &mut rows {
        while row.len() < num_cols {
            row.push(TableCellRender {
                text: String::new(),
                spans: Vec::new(),
            });
        }
    }

    // Calculate column widths
    let mut col_widths = vec![1_usize; num_cols];
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            col_widths[idx] = col_widths[idx].max(display_width(&cell.text));
        }
    }

    // All columns left-aligned (CSV has no alignment info)
    let alignments = vec![comrak::nodes::TableAlignment::None; num_cols];
    let mid = render_table_inner_divider(&col_widths);

    let mut lines = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        lines.push(render_table_row(row, &col_widths, &alignments));
        // First row is treated as header
        if idx == 0 {
            lines.push(RenderedLine::new(mid.clone(), LineType::Table));
        }
    }

    lines
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn normalize_inline_whitespace(spans: &[InlineSpan]) -> Vec<InlineSpan> {
    let text = spans_to_string(spans);
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut first_word = true;
    for token in split_tokens_preserve_whitespace(spans) {
        let is_ws = token.text().chars().all(char::is_whitespace);
        if is_ws {
            continue;
        }
        if !first_word {
            result.push(InlineSpan::new(" ".to_string(), InlineStyle::default()));
        }
        result.push(token);
        first_word = false;
    }
    result
}

fn split_tokens_preserve_whitespace(spans: &[InlineSpan]) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    for span in spans {
        out.extend(split_inline_tokens(span));
    }
    out
}

fn truncate_spans_by_display_width(spans: &[InlineSpan], max_width: usize) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let mut used = 0usize;

    for span in spans {
        if used >= max_width {
            break;
        }
        let mut taken = String::new();
        for ch in span.text().chars() {
            let w = ch.width().unwrap_or(0);
            if used + w > max_width {
                break;
            }
            taken.push(ch);
            used += w;
        }
        if !taken.is_empty() {
            out.push(InlineSpan::new(taken, span.style()));
        }
    }
    out
}

fn render_superscript_text(text: &str) -> String {
    render_script_text(text, true)
}

fn render_subscript_text(text: &str) -> String {
    render_script_text(text, false)
}

fn render_script_text(text: &str, superscript: bool) -> String {
    let Some(mapped) = map_script_chars(text, superscript) else {
        return if superscript {
            format!("^({text})")
        } else {
            format!("_({text})")
        };
    };
    mapped
}

fn render_footnote_reference(name: &str) -> String {
    // Reuse the same superscript renderer as inline superscript, but only for
    // numeric footnote labels to avoid uneven glyph spacing in some fonts.
    if name.chars().all(|c| c.is_ascii_digit()) {
        return render_superscript_text(name);
    }
    format!("[^{name}]")
}

fn map_script_chars(text: &str, superscript: bool) -> Option<String> {
    let mut mapped = String::new();
    for ch in text.chars() {
        let mapped_char = if superscript {
            superscript_char(ch)
        } else {
            subscript_char(ch)
        }?;
        mapped.push(mapped_char);
    }
    Some(mapped)
}

const fn superscript_char(ch: char) -> Option<char> {
    match ch {
        'a' => Some('ᵃ'),
        'b' => Some('ᵇ'),
        'c' => Some('ᶜ'),
        'd' => Some('ᵈ'),
        'e' => Some('ᵉ'),
        'f' => Some('ᶠ'),
        'g' => Some('ᵍ'),
        'h' => Some('ʰ'),
        '0' => Some('⁰'),
        '1' => Some('¹'),
        '2' => Some('²'),
        '3' => Some('³'),
        '4' => Some('⁴'),
        '5' => Some('⁵'),
        '6' => Some('⁶'),
        '7' => Some('⁷'),
        '8' => Some('⁸'),
        '9' => Some('⁹'),
        'j' => Some('ʲ'),
        'k' => Some('ᵏ'),
        'l' => Some('ˡ'),
        'm' => Some('ᵐ'),
        'o' => Some('ᵒ'),
        'p' => Some('ᵖ'),
        'r' => Some('ʳ'),
        's' => Some('ˢ'),
        't' => Some('ᵗ'),
        'u' => Some('ᵘ'),
        'v' => Some('ᵛ'),
        'w' => Some('ʷ'),
        'x' => Some('ˣ'),
        'y' => Some('ʸ'),
        'z' => Some('ᶻ'),
        '+' => Some('⁺'),
        '-' => Some('⁻'),
        '=' => Some('⁼'),
        '(' => Some('⁽'),
        ')' => Some('⁾'),
        'n' => Some('ⁿ'),
        'i' => Some('ⁱ'),
        _ => None,
    }
}

const fn subscript_char(ch: char) -> Option<char> {
    match ch {
        '0' => Some('₀'),
        '1' => Some('₁'),
        '2' => Some('₂'),
        '3' => Some('₃'),
        '4' => Some('₄'),
        '5' => Some('₅'),
        '6' => Some('₆'),
        '7' => Some('₇'),
        '8' => Some('₈'),
        '9' => Some('₉'),
        '+' => Some('₊'),
        '-' => Some('₋'),
        '=' => Some('₌'),
        '(' => Some('₍'),
        ')' => Some('₎'),
        'a' => Some('ₐ'),
        'e' => Some('ₑ'),
        'h' => Some('ₕ'),
        'i' => Some('ᵢ'),
        'j' => Some('ⱼ'),
        'k' => Some('ₖ'),
        'l' => Some('ₗ'),
        'm' => Some('ₘ'),
        'n' => Some('ₙ'),
        'o' => Some('ₒ'),
        'p' => Some('ₚ'),
        'r' => Some('ᵣ'),
        's' => Some('ₛ'),
        't' => Some('ₜ'),
        'u' => Some('ᵤ'),
        'v' => Some('ᵥ'),
        'x' => Some('ₓ'),
        _ => None,
    }
}

fn extract_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    extract_text_recursive(node, &mut text);
    text
}

fn collect_inline_spans<'a>(node: &'a AstNode<'a>) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    collect_inline_spans_recursive(node, InlineStyle::default(), &mut spans);
    spans
}

fn collect_inline_spans_recursive<'a>(
    node: &'a AstNode<'a>,
    style: InlineStyle,
    spans: &mut Vec<InlineSpan>,
) {
    match &node.data.borrow().value {
        NodeValue::List(_) | NodeValue::Item(_) => {}

        NodeValue::Text(t) => {
            push_text_with_footnote_fallback(spans, t, style);
        }
        NodeValue::Code(code) => {
            let mut code_style = style;
            code_style.code = true;
            code_style.emphasis = false;
            code_style.strong = false;
            code_style.strikethrough = false;
            spans.push(InlineSpan::new(code.literal.clone(), code_style));
        }
        NodeValue::Emph => {
            let mut next = style;
            next.emphasis = true;
            for child in node.children() {
                collect_inline_spans_recursive(child, next, spans);
            }
        }
        NodeValue::Strong => {
            let mut next = style;
            next.strong = true;
            for child in node.children() {
                collect_inline_spans_recursive(child, next, spans);
            }
        }
        NodeValue::Strikethrough => {
            let mut next = style;
            next.strikethrough = true;
            for child in node.children() {
                collect_inline_spans_recursive(child, next, spans);
            }
        }
        NodeValue::Superscript => {
            let mut inner = String::new();
            for child in node.children() {
                inner.push_str(&extract_text(child));
            }
            spans.push(InlineSpan::new(render_superscript_text(&inner), style));
        }
        NodeValue::Subscript => {
            let mut inner = String::new();
            for child in node.children() {
                inner.push_str(&extract_text(child));
            }
            spans.push(InlineSpan::new(render_subscript_text(&inner), style));
        }
        NodeValue::Link(_) => {
            let mut next = style;
            next.link = true;
            for child in node.children() {
                collect_inline_spans_recursive(child, next, spans);
            }
        }
        NodeValue::FootnoteReference(reference) => {
            spans.push(InlineSpan::new(
                render_footnote_reference(&reference.name),
                style,
            ));
        }
        NodeValue::SoftBreak | NodeValue::LineBreak => {
            spans.push(InlineSpan::new(" ".to_string(), style));
        }
        _ => {
            for child in node.children() {
                collect_inline_spans_recursive(child, style, spans);
            }
        }
    }
}

fn find_task_marker<'a>(node: &'a AstNode<'a>) -> Option<&'static str> {
    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::TaskItem(symbol) => {
                return Some(if symbol.is_some() { "✓" } else { "□" });
            }
            _ => {
                if let Some(found) = find_task_marker(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn extract_text_recursive<'a>(node: &'a AstNode<'a>, text: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(t) => {
            text.push_str(&render_text_with_footnote_fallback(t));
        }
        NodeValue::Code(c) => {
            text.push('`');
            text.push_str(&c.literal);
            text.push('`');
        }
        NodeValue::Superscript => {
            let mut inner = String::new();
            for child in node.children() {
                extract_text_recursive(child, &mut inner);
            }
            text.push_str(&render_superscript_text(&inner));
        }
        NodeValue::Subscript => {
            let mut inner = String::new();
            for child in node.children() {
                extract_text_recursive(child, &mut inner);
            }
            text.push_str(&render_subscript_text(&inner));
        }
        NodeValue::FootnoteReference(reference) => {
            text.push_str(&render_footnote_reference(&reference.name));
        }
        NodeValue::SoftBreak | NodeValue::LineBreak => {
            text.push('\n');
        }
        _ => {
            for child in node.children() {
                extract_text_recursive(child, text);
            }
        }
    }
}

fn wrap_spans(
    spans: &[InlineSpan],
    width: usize,
    prefix_first: &str,
    prefix_next: &str,
) -> Vec<Vec<InlineSpan>> {
    let mut tokens: Vec<InlineSpan> = Vec::new();
    for span in spans {
        tokens.extend(split_inline_tokens(span));
    }

    let mut lines: Vec<Vec<InlineSpan>> = Vec::new();
    let mut current: Vec<InlineSpan> = Vec::new();
    let mut current_len = 0usize;
    let mut has_word = false;

    let start_new_line = |prefix: &str,
                          current: &mut Vec<InlineSpan>,
                          current_len: &mut usize,
                          has_word: &mut bool| {
        current.clear();
        if prefix.is_empty() {
            *current_len = 0;
        } else {
            current.push(InlineSpan::new(prefix.to_string(), InlineStyle::default()));
            *current_len = UnicodeWidthStr::width(prefix);
        }
        *has_word = false;
    };

    start_new_line(prefix_first, &mut current, &mut current_len, &mut has_word);

    for token in tokens {
        let token_len = UnicodeWidthStr::width(token.text());
        let token_is_ws = token.text().chars().all(char::is_whitespace);

        if current_len + token_len > width && has_word {
            lines.push(current.clone());
            start_new_line(prefix_next, &mut current, &mut current_len, &mut has_word);
        }

        if token_is_ws && !has_word {
            // Drop leading whitespace at wrapped line starts.
            continue;
        }

        current_len += token_len;
        current.push(token);
        has_word = token_is_ws || has_word;
        if !token_is_ws {
            has_word = true;
        }
    }

    if current.is_empty() && !prefix_first.is_empty() {
        current.push(InlineSpan::new(
            prefix_first.to_string(),
            InlineStyle::default(),
        ));
    }

    lines.push(current);
    lines
}

fn push_text_with_footnote_fallback(spans: &mut Vec<InlineSpan>, text: &str, style: InlineStyle) {
    let rendered = render_text_with_footnote_fallback(text);
    if !rendered.is_empty() {
        spans.push(InlineSpan::new(rendered, style));
    }
}

fn render_text_with_footnote_fallback(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '[' && i + 3 < chars.len() && chars[i + 1] == '^' {
            let mut j = i + 2;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 2 && j < chars.len() && chars[j] == ']' {
                let digits: String = chars[i + 2..j].iter().collect();
                out.push_str(&render_superscript_text(&digits));
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn split_inline_tokens(span: &InlineSpan) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut ws_state: Option<bool> = None;

    for ch in span.text().chars() {
        let is_ws = ch.is_whitespace();
        match ws_state {
            Some(state) if state == is_ws => {
                buf.push(ch);
            }
            Some(_) => {
                out.push(InlineSpan::new(std::mem::take(&mut buf), span.style()));
                buf.push(ch);
                ws_state = Some(is_ws);
            }
            None => {
                buf.push(ch);
                ws_state = Some(is_ws);
            }
        }
    }

    if !buf.is_empty() {
        out.push(InlineSpan::new(buf, span.style()));
    }

    out
}

fn spans_to_string(spans: &[InlineSpan]) -> String {
    let mut content = String::new();
    for span in spans {
        content.push_str(span.text());
    }
    content
}

fn truncate_spans(spans: &[InlineSpan], max_len: usize) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let mut remaining = max_len;
    for span in spans {
        if remaining == 0 {
            break;
        }
        let mut taken = String::new();
        let mut used = 0usize;
        for ch in span.text().chars() {
            let w = ch.width().unwrap_or(0);
            if used + w > remaining {
                break;
            }
            taken.push(ch);
            used += w;
        }
        if used > 0 {
            out.push(InlineSpan::new(taken, span.style()));
            remaining = remaining.saturating_sub(used);
        }
    }
    out
}

/// Collect images from a paragraph node, returning (alt, src) pairs.
fn collect_paragraph_images<'a>(node: &'a AstNode<'a>) -> Vec<(String, String)> {
    let mut images = Vec::new();
    collect_paragraph_images_recursive(node, &mut images);
    images
}

fn collect_paragraph_images_recursive<'a>(
    node: &'a AstNode<'a>,
    images: &mut Vec<(String, String)>,
) {
    match &node.data.borrow().value {
        NodeValue::Image(image) => {
            let alt = extract_text(node);
            images.push((alt, image.url.clone()));
        }
        _ => {
            for child in node.children() {
                collect_paragraph_images_recursive(child, images);
            }
        }
    }
}

/// After wrapping, fix up `LinkRef.line` values so each link points to
/// the actual rendered line that contains its text, not the first line
/// of the paragraph.
fn fixup_link_lines(links: &mut [LinkRef], wrapped_lines: &[RenderedLine], base_line: usize) {
    // Track which (line_index, byte_offset) occurrences have been claimed
    // so duplicate link text (e.g., two links both labelled "here") each
    // match a distinct occurrence in the rendered output.
    let mut claimed: Vec<(usize, usize)> = Vec::new();

    for link in links.iter_mut() {
        if link.text.is_empty() {
            continue;
        }
        let mut found = false;
        for (i, line) in wrapped_lines.iter().enumerate() {
            let content = line.content();
            let mut search = 0usize;
            while let Some(pos) = content[search..].find(&link.text) {
                let byte_offset = search + pos;
                if !claimed.contains(&(i, byte_offset)) {
                    link.line = base_line + i;
                    claimed.push((i, byte_offset));
                    found = true;
                    break;
                }
                search = byte_offset + link.text.len();
            }
            if found {
                break;
            }
        }
    }
}

fn collect_inline_elements<'a>(
    node: &'a AstNode<'a>,
    base_line: usize,
    images: &mut Vec<ImageRef>,
    links: &mut Vec<LinkRef>,
) {
    match &node.data.borrow().value {
        NodeValue::Image(image) => {
            let alt = extract_text(node);
            let src = image.url.clone();
            images.push(ImageRef {
                alt,
                src,
                line_range: base_line..base_line + 1,
            });
        }
        NodeValue::Link(link) => {
            let text = extract_text(node);
            let url = link.url.clone();
            links.push(LinkRef {
                text,
                url,
                line: base_line,
            });
        }
        NodeValue::FootnoteReference(reference) => {
            links.push(LinkRef {
                text: render_footnote_reference(&reference.name),
                url: format!("footnote:{}", reference.name),
                line: base_line,
            });
        }
        NodeValue::Text(t) => {
            // When comrak emits [^N] as plain text (no matching definition),
            // render_text_with_footnote_fallback converts it to superscript.
            // We need to create LinkRefs for these too.
            collect_text_footnote_links(t, base_line, links);
        }
        _ => {
            for child in node.children() {
                collect_inline_elements(child, base_line, images, links);
            }
        }
    }
}

/// Scan a plain text string for `[^N]` footnote patterns (the same ones that
/// `render_text_with_footnote_fallback` converts to superscript) and create
/// `LinkRef` entries so they are clickable.
fn collect_text_footnote_links(text: &str, base_line: usize, links: &mut Vec<LinkRef>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '[' && i + 3 < chars.len() && chars[i + 1] == '^' {
            let mut j = i + 2;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 2 && j < chars.len() && chars[j] == ']' {
                let digits: String = chars[i + 2..j].iter().collect();
                links.push(LinkRef {
                    text: render_superscript_text(&digits),
                    url: format!("footnote:{digits}"),
                    line: base_line,
                });
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_document() {
        let doc = parse("").unwrap();
        assert_eq!(doc.line_count(), 0);
    }

    #[test]
    fn test_parse_simple_paragraph() {
        let doc = parse("Hello world").unwrap();
        assert!(doc.line_count() >= 1);
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| l.content().contains("Hello")));
    }

    #[test]
    fn test_parse_heading() {
        let doc = parse("# Title").unwrap();
        assert_eq!(doc.headings().len(), 1);
        assert_eq!(doc.headings()[0].text, "Title");
        assert_eq!(doc.headings()[0].level, 1);
    }

    #[test]
    fn test_parse_multiple_headings() {
        let doc = parse("# One\n\n## Two\n\n### Three").unwrap();
        assert_eq!(doc.headings().len(), 3);
        assert_eq!(doc.headings()[0].level, 1);
        assert_eq!(doc.headings()[1].level, 2);
        assert_eq!(doc.headings()[2].level, 3);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse("```rust\nfn main() {}\n```").unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| *l.line_type() == LineType::CodeBlock));
    }

    #[test]
    fn test_parse_list() {
        let doc = parse("- Item 1\n- Item 2").unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| l.content().contains("Item 1")));
    }

    #[test]
    fn test_parse_link() {
        let doc = parse("[Click here](https://example.com)").unwrap();
        assert_eq!(doc.links().len(), 1);
        assert_eq!(doc.links()[0].url, "https://example.com");
    }

    #[test]
    fn test_parse_image() {
        let doc = parse("![Alt text](image.png)").unwrap();
        assert_eq!(doc.images().len(), 1);
        assert_eq!(doc.images()[0].alt, "Alt text");
        assert_eq!(doc.images()[0].src, "image.png");
    }

    #[test]
    fn test_parse_blockquote() {
        let doc = parse("> This is a quote").unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| *l.line_type() == LineType::BlockQuote));
        assert!(lines.iter().any(|l| l.content().starts_with("  │ ")));
        assert!(!lines.iter().any(|l| l.content().starts_with("> ")));
    }

    #[test]
    fn test_blockquote_wraps_with_quote_prefix() {
        let md = "> This is a long block quote line that should wrap and keep the quote prefix.";
        let doc = Document::parse_with_layout(md, 30).unwrap();
        let lines = doc.visible_lines(0, 20);
        let quote_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::BlockQuote)
            .collect();
        assert!(quote_lines.len() > 1);
        for line in quote_lines {
            assert!(line.content().starts_with("  │ "));
            assert!(unicode_width::UnicodeWidthStr::width(line.content()) <= 30);
        }
    }

    #[test]
    fn test_heading_line_numbers() {
        let doc = parse("# First\n\nParagraph\n\n# Second").unwrap();
        assert_eq!(doc.headings().len(), 2);
        // Headings have two rows above them.
        assert_eq!(doc.headings()[0].line, 2);
        // Second heading should be after the first heading + empty + paragraph + empty
        assert!(doc.headings()[1].line > doc.headings()[0].line);
    }

    #[test]
    fn test_heading_has_two_rows_above() {
        let doc = Document::parse_with_layout("Paragraph\n\n## Heading", 80).unwrap();
        let heading_line = doc.headings().first().expect("heading missing").line;
        let lines = doc.visible_lines(0, heading_line + 1);
        assert!(heading_line >= 2);
        assert_eq!(*lines[heading_line - 1].line_type(), LineType::Empty);
        assert_eq!(*lines[heading_line - 2].line_type(), LineType::Empty);
    }

    #[test]
    fn test_gfm_strikethrough() {
        let doc = parse("~~deleted~~").unwrap();
        // Should parse without error (content check would need styled spans)
        assert!(doc.line_count() > 0);
    }

    #[test]
    fn test_gfm_tasklist() {
        let doc = parse("- [x] Done\n- [ ] Todo").unwrap();
        assert!(doc.line_count() > 0);
    }

    #[test]
    fn test_gfm_table() {
        let doc = parse("| A | B |\n|---|---|\n| 1 | 2 |").unwrap();
        let lines = doc.visible_lines(0, 10);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(!table_lines.is_empty());
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("A") && l.content().contains("B"))
        );
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("1") && l.content().contains("2"))
        );
        assert!(table_lines.iter().any(|l| l.content().contains('│')));
        assert!(table_lines.iter().any(|l| l.content().contains("───┼───")));
    }

    #[test]
    fn test_gfm_table_preserves_cell_content() {
        let md = "| Very long heading | Value |\n|---|---:|\n| some really long content | 12345 |";
        let doc = Document::parse_with_layout(md, 24).unwrap();
        let lines = doc.visible_lines(0, 20);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("Very long heading"))
        );
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("some really long content"))
        );
    }

    #[test]
    fn test_gfm_table_with_emoji_preserves_cell_content() {
        let md =
            "| Feature | Status |\n|---|---|\n| Bold | ✅ Supported |\n| Italic | ✅ Supported |";
        let doc = Document::parse_with_layout(md, 28).unwrap();
        let lines = doc.visible_lines(0, 20);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("✅ Supported"))
        );
    }

    #[test]
    fn test_gfm_table_mixed_content_renders_each_row_separately() {
        let md = "| Feature | Status | Notes |\n|---------|--------|-------|\n| **Bold** | ✅ Supported | Works well |\n| *Italic* | ✅ Supported | Works well |\n| `Code` | ✅ Supported | Inline only |\n| ~~Strike~~ | ✅ Supported | GFM extension |\n| [Links](/) | ✅ Supported | Full support |";
        let doc = Document::parse_with_layout(md, 120).unwrap();
        let table_lines: Vec<_> = doc
            .visible_lines(0, 100)
            .into_iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .map(|l| l.content().to_string())
            .collect();

        assert_eq!(table_lines.len(), 7);
        assert!(table_lines.iter().all(|line| !line.contains('\n')));
        assert!(table_lines.iter().any(|line| line.contains("Bold")));
        assert!(table_lines.iter().any(|line| line.contains("Italic")));
        assert!(table_lines.iter().any(|line| line.contains("Code")));
        assert!(table_lines.iter().any(|line| line.contains("Strike")));
        assert!(table_lines.iter().any(|line| line.contains("Links")));
    }

    #[test]
    fn test_gfm_table_mixed_content_preserves_inline_styles_in_cells() {
        let md = "| Feature | Type |\n|---|---|\n| **Bold** | a |\n| *Italic* | b |\n| `Code` | c |\n| ~~Strike~~ | d |\n| [Links](/) | e |";
        let doc = Document::parse_with_layout(md, 120).unwrap();
        let table_rows: Vec<_> = doc
            .visible_lines(0, 50)
            .into_iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .filter(|l| !l.content().contains('┼'))
            .collect();

        assert!(table_rows.iter().any(|row| {
            row.spans()
                .is_some_and(|spans| spans.iter().any(|s| s.style().strong))
        }));
        assert!(table_rows.iter().any(|row| {
            row.spans()
                .is_some_and(|spans| spans.iter().any(|s| s.style().emphasis))
        }));
        assert!(table_rows.iter().any(|row| {
            row.spans()
                .is_some_and(|spans| spans.iter().any(|s| s.style().code))
        }));
        assert!(table_rows.iter().any(|row| {
            row.spans()
                .is_some_and(|spans| spans.iter().any(|s| s.style().strikethrough))
        }));
        assert!(table_rows.iter().any(|row| {
            row.spans()
                .is_some_and(|spans| spans.iter().any(|s| s.style().link))
        }));
    }

    #[test]
    fn test_paragraph_wraps_to_width() {
        let md = "This is a long paragraph that should wrap at the specified width.";
        let doc = Document::parse_with_layout(md, 20).unwrap();
        let lines = doc.visible_lines(0, 100);

        let paragraph_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Paragraph)
            .collect();

        assert!(paragraph_lines.len() > 1);
        for line in paragraph_lines {
            assert!(unicode_width::UnicodeWidthStr::width(line.content()) <= 20);
        }
    }

    #[test]
    fn test_inline_styles_create_spans() {
        let md = "*em* **strong** `code` [link](https://example.com) ~~strike~~";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let paragraph = lines
            .iter()
            .find(|l| *l.line_type() == LineType::Paragraph)
            .expect("Paragraph line missing");
        let spans = paragraph.spans().expect("Inline spans missing");

        assert!(spans.iter().any(|s| s.style().emphasis));
        assert!(spans.iter().any(|s| s.style().strong));
        assert!(spans.iter().any(|s| s.style().code));
        assert!(spans.iter().any(|s| s.style().link));
        assert!(spans.iter().any(|s| s.style().strikethrough));
    }

    #[test]
    fn test_superscript_renders_with_unicode_digits() {
        let md = "E = mc^2^";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let paragraph = lines
            .iter()
            .find(|l| *l.line_type() == LineType::Paragraph)
            .expect("Paragraph line missing");
        assert!(paragraph.content().contains("²"));
    }

    #[test]
    fn test_subscript_renders_with_unicode_digits() {
        let md = "H~2~O";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let paragraph = lines
            .iter()
            .find(|l| *l.line_type() == LineType::Paragraph)
            .expect("Paragraph line missing");
        assert!(paragraph.content().contains("₂"));
    }

    #[test]
    fn test_subscript_falls_back_when_glyph_missing() {
        let md = "x~q~";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let paragraph = lines
            .iter()
            .find(|l| *l.line_type() == LineType::Paragraph)
            .expect("Paragraph line missing");
        assert!(paragraph.content().contains("_(q)"));
    }

    #[test]
    fn test_superscript_letters_and_symbols_render_unicode() {
        let md = "x^abc+()^";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let paragraph = lines
            .iter()
            .find(|l| *l.line_type() == LineType::Paragraph)
            .expect("Paragraph line missing");
        assert!(paragraph.content().contains("ᵃᵇᶜ⁺⁽⁾"));
    }

    #[test]
    fn test_footnote_reference_and_definition_render() {
        let md = "Alpha[^12]\n\n[^12]: Footnote text";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        assert!(lines.iter().any(|l| l.content().contains("Alpha¹²")));
        assert!(lines.iter().any(|l| l.content().contains("¹² ")));
        assert!(lines.iter().any(|l| l.content().contains("Footnote text")));
    }

    #[test]
    fn test_footnote_reference_falls_back_when_superscript_missing() {
        let md = "Alpha[^q]\n\n[^q]: Footnote text";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        assert!(lines.iter().any(|l| l.content().contains("[^q]")));
        assert!(
            lines
                .iter()
                .any(|l| l.content().contains("[^q] Footnote text"))
        );
    }

    #[test]
    fn test_footnote_reference_alpha_label_uses_plain_fallback() {
        let md = "Alpha[^n]\n\n[^n]: Footnote text";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        assert!(lines.iter().any(|l| l.content().contains("[^n]")));
        assert!(
            lines
                .iter()
                .any(|l| l.content().contains("[^n] Footnote text"))
        );
    }

    #[test]
    fn test_numeric_footnote_reference_renders_without_definition() {
        let md = "Here is a sentence with a footnote[^123].";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| l.content().contains("footnote¹²³.")));
    }

    #[test]
    fn test_footnote_without_definition_still_creates_link_ref() {
        // When comrak emits [^1] as plain text (no definition),
        // the text fallback renders it as superscript but should still create a LinkRef
        let md = "Here is a sentence with a footnote[^1].";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let footnote_links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url.starts_with("footnote:"))
            .collect();
        assert_eq!(
            footnote_links.len(),
            1,
            "text fallback should create a LinkRef for [^1]"
        );
        assert_eq!(footnote_links[0].text, "¹");
    }

    #[test]
    fn test_horizontal_rule_renders_subtle_line() {
        let md = "Alpha\n\n---\n\nBeta";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(lines.iter().any(|l| l.content() == "─────"));
    }

    #[test]
    fn test_image_caption_renders_only_when_image_height_known() {
        let md = "![Alt text](image.png)";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        assert!(!lines.iter().any(|l| l.content() == "Alt text"));

        let mut heights = HashMap::new();
        heights.insert("image.png".to_string(), 2);
        let doc = Document::parse_with_layout_and_image_heights(md, 80, &heights).unwrap();
        let lines = doc.visible_lines(0, 10);
        let image_idx = lines
            .iter()
            .position(|l| l.content().starts_with("[Image:"))
            .expect("image placeholder missing");
        assert!(image_idx > 0, "caption should be above image");
        assert_eq!(lines[image_idx - 1].content(), "    Alt text");
    }

    #[test]
    fn test_code_block_highlights_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let mut doc = Document::parse_with_layout(md, 80).unwrap();
        doc.ensure_highlight_for_range(0..doc.line_count());
        let lines = doc.visible_lines(0, 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("fn main"))
            .expect("Code line missing");
        let spans = code_line.spans().expect("Expected code line spans");
        assert!(
            spans.iter().any(|s| s.style().fg.is_some()),
            "Expected highlighted code spans"
        );
    }

    #[test]
    fn test_code_block_is_plain_until_range_is_highlighted() {
        let md = "```rust\nfn main() {}\n```";
        let mut doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("fn main"))
            .expect("Code line missing");
        let spans = code_line.spans().expect("Expected code line spans");
        assert!(
            spans.iter().all(|s| s.style().fg.is_none()),
            "Expected plain code before lazy highlighting"
        );

        doc.ensure_highlight_for_range(0..doc.line_count());
        let lines = doc.visible_lines(0, 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("fn main"))
            .expect("Code line missing");
        let spans = code_line.spans().expect("Expected code line spans");
        assert!(spans.iter().any(|s| s.style().fg.is_some()));
    }

    #[test]
    fn test_code_block_renders_without_fence_markers() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);

        assert!(!lines.iter().any(|l| l.content().starts_with("```")));
        assert!(lines.iter().any(|l| l.content().contains(" rust ")));
    }

    #[test]
    fn test_code_block_renders_ascii_box() {
        let md = "```rust\nfn main() {}\n```";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let code_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::CodeBlock)
            .collect();

        assert!(code_lines.first().unwrap().content().starts_with('┌'));
        assert!(code_lines.first().unwrap().content().ends_with('┐'));
        assert!(code_lines.last().unwrap().content().starts_with('└'));
        assert!(code_lines.last().unwrap().content().ends_with('┘'));
        assert!(code_lines.iter().any(|l| l.content().starts_with("│ ")));
        let top_width =
            unicode_width::UnicodeWidthStr::width(code_lines.first().unwrap().content());
        for line in &code_lines {
            assert_eq!(
                unicode_width::UnicodeWidthStr::width(line.content()),
                top_width
            );
        }
    }

    #[test]
    fn test_code_block_has_right_padding_inside_frame() {
        let md = "```rust\nlet x = 1;\n```";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("let x = 1;"))
            .expect("code line missing");
        assert!(
            code_line.content().contains("   │"),
            "expected at least a few spaces of right padding before border"
        );
    }

    #[test]
    fn test_ordered_list_marker() {
        let md = "1. First item\n2. Second item";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::ListItem(1))
            .collect();

        assert!(list_lines[0].content().starts_with("1. "));
        assert!(list_lines[1].content().starts_with("2. "));
    }

    #[test]
    fn test_list_wraps_with_hanging_indent() {
        let md = "1. This is a long list item that should wrap to the next line.";
        let doc = Document::parse_with_layout(md, 20).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::ListItem(1))
            .collect();

        assert!(list_lines.len() > 1);
        assert!(list_lines[0].content().starts_with("1. "));
        assert!(list_lines[1].content().starts_with("   "));
    }

    #[test]
    fn test_unordered_list_uses_bullet_character() {
        let md = "* Item";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_line = lines
            .iter()
            .find(|l| *l.line_type() == LineType::ListItem(1))
            .expect("List line missing");

        assert!(list_line.content().starts_with("• "));
    }

    #[test]
    fn test_nested_list_indents_children() {
        let md = "- Parent\n  - Child";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| matches!(l.line_type(), LineType::ListItem(_)))
            .collect();

        assert!(list_lines[0].content().starts_with("• "));
        assert!(list_lines[1].content().starts_with("  • "));
    }

    #[test]
    fn test_list_item_with_multiple_paragraphs_has_blank_line() {
        let md = "- First paragraph\n\n  Second paragraph";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::ListItem(1))
            .collect();

        assert!(list_lines.len() >= 3);
        assert_eq!(list_lines[1].content(), "");
    }

    #[test]
    fn test_list_has_trailing_blank_line() {
        let md = "- One\n- Two\n\nAfter";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        let after_idx = lines
            .iter()
            .position(|l| l.content().contains("After"))
            .expect("After line missing");
        assert!(after_idx > 0);
        assert_eq!(lines[after_idx - 1].content(), "");
    }

    #[test]
    fn test_task_list_marker() {
        let md = "- [x] Done\n- [ ] Todo";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::ListItem(1))
            .collect();

        assert!(list_lines[0].content().starts_with("✓ "));
        assert!(list_lines[1].content().starts_with("□ "));
    }

    #[test]
    fn test_ordered_list_alignment_for_two_digits() {
        let md = "9. Ninth\n10. Tenth";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::ListItem(1))
            .collect();

        assert!(list_lines[0].content().starts_with(" 9. "));
        assert!(list_lines[1].content().starts_with("10. "));
    }

    #[test]
    fn test_nested_task_list_markers_indent() {
        let md = "- [x] Parent\n  - [ ] Child";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| matches!(l.line_type(), LineType::ListItem(_)))
            .collect();

        assert!(list_lines[0].content().starts_with("✓ "));
        assert!(list_lines[1].content().starts_with("  □ "));
    }

    #[test]
    fn test_task_list_parent_does_not_inline_children() {
        let md = "- [x] Main task completed\n  - [x] Subtask 1 done\n  - [ ] Subtask 2 pending";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 10);
        let list_lines: Vec<_> = lines
            .iter()
            .filter(|l| matches!(l.line_type(), LineType::ListItem(_)))
            .collect();

        assert!(list_lines[0].content().contains("Main task completed"));
        assert!(!list_lines[0].content().contains("Subtask"));
    }

    #[test]
    fn test_csv_code_block_renders_as_table() {
        let md = "```csv\nName,Age,City\nAlice,30,NYC\nBob,25,LA\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 20);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            !table_lines.is_empty(),
            "CSV block should render as Table lines"
        );
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("Name") && l.content().contains("Age")),
            "Header row should contain column names"
        );
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("Alice") && l.content().contains("30")),
            "Data rows should be present"
        );
    }

    #[test]
    fn test_csv_code_block_has_header_divider() {
        let md = "```csv\nA,B\n1,2\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 10);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines.iter().any(|l| l.content().contains("┼")),
            "CSV table should have a header divider with ┼"
        );
    }

    #[test]
    fn test_csv_code_block_not_rendered_as_code_block() {
        let md = "```csv\nX,Y\n1,2\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 10);
        let code_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::CodeBlock)
            .collect();
        assert!(
            code_lines.is_empty(),
            "CSV block should not render as CodeBlock lines"
        );
    }

    #[test]
    fn test_csv_code_block_preserves_cell_content() {
        let md =
            "```csv\nVery Long Column Name,Another Long Name\nsome really long content,12345\n```";
        let doc = Document::parse_with_layout(md, 30).unwrap();
        let lines = doc.visible_lines(0, 20);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("Very Long Column Name"))
        );
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("some really long content"))
        );
    }

    #[test]
    fn test_csv_with_quoted_fields() {
        let md = "```csv\nName,Description\nAlice,\"Has a, comma\"\nBob,\"Simple\"\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 20);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines
                .iter()
                .any(|l| l.content().contains("Has a, comma")),
            "Quoted CSV fields with commas should be handled correctly"
        );
    }

    #[test]
    fn test_csv_empty_block_renders_nothing() {
        let md = "```csv\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 10);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            table_lines.is_empty(),
            "Empty CSV block should not render table lines"
        );
    }

    #[test]
    fn test_csv_single_column() {
        let md = "```csv\nName\nAlice\nBob\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 10);
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table)
            .collect();
        assert!(
            !table_lines.is_empty(),
            "Single-column CSV should render as table"
        );
        assert!(
            table_lines.iter().any(|l| l.content().contains("Name")),
            "Header should be present"
        );
        assert!(
            table_lines.iter().any(|l| l.content().contains("Alice")),
            "Data rows should be present"
        );
    }

    #[test]
    fn test_csv_large_file_renders_all_rows() {
        let mut md = String::from("```csv\nID,Value\n");
        for i in 0..200 {
            md.push_str(&format!("{},{}\n", i, i * 10));
        }
        md.push_str("```");
        let doc = parse(&md).unwrap();
        let lines = doc.visible_lines(0, 500);
        let data_rows: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::Table && !l.content().contains('┼'))
            .collect();
        // 1 header + 200 data rows = 201 total non-divider table lines
        assert_eq!(data_rows.len(), 201, "All CSV rows should be rendered");
    }

    #[test]
    fn test_csv_large_file_pages_via_viewport() {
        let mut md = String::from("```csv\nID,Value\n");
        for i in 0..200 {
            md.push_str(&format!("{},{}\n", i, i * 10));
        }
        md.push_str("```");
        let doc = parse(&md).unwrap();
        // Simulate viewport paging: first page
        let page1 = doc.visible_lines(0, 10);
        assert_eq!(page1.len(), 10);
        // Later page
        let page2 = doc.visible_lines(100, 10);
        assert_eq!(page2.len(), 10);
        assert_ne!(page1[0].content(), page2[0].content());
    }

    #[test]
    fn test_non_csv_code_block_unchanged() {
        let md = "```rust\nfn main() {}\n```";
        let doc = parse(md).unwrap();
        let lines = doc.visible_lines(0, 10);
        let code_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == LineType::CodeBlock)
            .collect();
        assert!(
            !code_lines.is_empty(),
            "Non-CSV code blocks should still render as CodeBlock"
        );
    }

    #[test]
    fn test_mermaid_code_block_stored_as_mermaid_source() {
        let md = "```mermaid\ngraph TD\n    A --> B\n```";
        let doc = parse(md).unwrap();
        assert_eq!(doc.mermaid_sources().len(), 1);
        let source = doc.mermaid_sources().values().next().unwrap();
        assert!(source.contains("graph TD"));
        assert!(source.contains("A --> B"));
    }

    #[test]
    fn test_non_mermaid_code_block_not_in_mermaid_sources() {
        let md = "```rust\nfn main() {}\n```";
        let doc = parse(md).unwrap();
        assert!(doc.mermaid_sources().is_empty());
    }

    #[test]
    fn test_mermaid_block_renders_as_image_when_flag_set() {
        let md = "```mermaid\ngraph TD\n    A --> B\n```";
        let doc = Document::parse_with_mermaid_images(md, 80).unwrap();
        assert_eq!(doc.images().len(), 1);
        assert!(doc.images()[0].src.starts_with("mermaid://"));
        // Should still have the source stored
        assert_eq!(doc.mermaid_sources().len(), 1);
    }

    #[test]
    fn test_mermaid_block_stays_as_code_without_flag() {
        let md = "```mermaid\ngraph TD\n    A --> B\n```";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        // No image entries for mermaid without the flag
        assert!(doc.images().is_empty());
        // Still stored as mermaid source
        assert_eq!(doc.mermaid_sources().len(), 1);
        // Rendered as code block lines
        let lines = doc.visible_lines(0, 20);
        assert!(lines.iter().any(|l| *l.line_type() == LineType::CodeBlock));
    }

    #[test]
    fn test_mermaid_image_placeholder_text() {
        let md = "```mermaid\ngraph TD\n    A --> B\n```";
        let doc = Document::parse_with_mermaid_images(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        assert!(
            lines
                .iter()
                .any(|l| l.content().contains("[Image: mermaid diagram]"))
        );
    }

    #[test]
    fn test_numeric_footnote_reference_link_uses_rendered_text() {
        let md = "See this[^1].\n\n[^1]: A footnote.";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        // The rendered text uses superscript "¹", so the LinkRef text must match
        let footnote_links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url.starts_with("footnote:"))
            .collect();
        assert!(!footnote_links.is_empty(), "should have a footnote link");
        let link = &footnote_links[0];
        assert_eq!(
            link.text, "¹",
            "LinkRef text should be the rendered superscript, not [^1]"
        );
    }

    #[test]
    fn test_footnote_definition_contains_link_refs() {
        let md = "Alpha[^1]\n\n[^1]: See [example](https://example.com) for details.";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let url_links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://example.com")
            .collect();
        assert_eq!(
            url_links.len(),
            1,
            "link inside footnote definition should be registered"
        );
        assert_eq!(url_links[0].text, "example");
    }

    #[test]
    fn test_list_item_contains_link_refs() {
        let md = "- Visit [Rust](https://rust-lang.org) for more\n- No link here";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://rust-lang.org")
            .collect();
        assert_eq!(links.len(), 1, "link inside list item should be registered");
        assert_eq!(links[0].text, "Rust");
        // The link line should match a line that actually contains "Rust"
        let line_content = doc.line_at(links[0].line).unwrap().content();
        assert!(
            line_content.contains("Rust"),
            "link line {} should contain 'Rust', got: {line_content}",
            links[0].line
        );
    }

    #[test]
    fn test_blockquote_contains_link_refs() {
        let md = "> Check [docs](https://docs.rs) for details";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://docs.rs")
            .collect();
        assert_eq!(
            links.len(),
            1,
            "link inside blockquote should be registered"
        );
        assert_eq!(links[0].text, "docs");
        let line_content = doc.line_at(links[0].line).unwrap().content();
        assert!(
            line_content.contains("docs"),
            "link line {} should contain 'docs', got: {line_content}",
            links[0].line
        );
    }

    #[test]
    fn test_table_contains_link_refs() {
        let md = "| Name | Link |\n|------|------|\n| Foo | [Bar](https://bar.com) |";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://bar.com")
            .collect();
        assert_eq!(
            links.len(),
            1,
            "link inside table cell should be registered"
        );
        assert_eq!(links[0].text, "Bar");
        let line_content = doc.line_at(links[0].line).unwrap().content();
        assert!(
            line_content.contains("Bar"),
            "link line {} should contain 'Bar', got: {line_content}",
            links[0].line
        );
    }

    #[test]
    fn test_task_item_contains_link_refs() {
        let md = "- [ ] Read [guide](https://guide.com)\n- [x] Done";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://guide.com")
            .collect();
        assert_eq!(links.len(), 1, "link inside task item should be registered");
        assert_eq!(links[0].text, "guide");
        let line_content = doc.line_at(links[0].line).unwrap().content();
        assert!(
            line_content.contains("guide"),
            "link line {} should contain 'guide', got: {line_content}",
            links[0].line
        );
    }

    #[test]
    fn test_heading_contains_link_refs() {
        let md = "# Title with [link](https://heading.com)";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://heading.com")
            .collect();
        assert_eq!(links.len(), 1, "link inside heading should be registered");
        assert_eq!(links[0].text, "link");
        let line_content = doc.line_at(links[0].line).unwrap().content();
        assert!(
            line_content.contains("link"),
            "link line {} should contain 'link', got: {line_content}",
            links[0].line
        );
    }

    #[test]
    fn test_nested_blockquote_contains_link_refs() {
        let md = "> > See [inner](https://inner.com)";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://inner.com")
            .collect();
        assert_eq!(
            links.len(),
            1,
            "link inside nested blockquote should be registered"
        );
        assert_eq!(links[0].text, "inner");
    }

    // Issue 2: fixup_link_lines with duplicate link text
    #[test]
    fn test_duplicate_link_text_assigned_to_correct_wrapped_lines() {
        // Two links both labelled "here" in a paragraph, narrow width forces
        // the second "here" onto a different wrapped line.
        let md = "Click [here](https://first.com) for the first thing, then click [here](https://second.com) for second.";
        let doc = Document::parse_with_layout(md, 35).unwrap();
        let first: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://first.com")
            .collect();
        let second: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://second.com")
            .collect();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);

        // Verify they actually ended up on different rendered lines
        // (the wrap at 35 chars should separate them)
        let line_first = doc.line_at(first[0].line).unwrap().content();
        let line_second = doc.line_at(second[0].line).unwrap().content();

        // Both lines must contain the link text
        assert!(
            line_first.contains("here"),
            "first link at line {} should contain 'here': {line_first:?}",
            first[0].line
        );
        assert!(
            line_second.contains("here"),
            "second link at line {} should contain 'here': {line_second:?}",
            second[0].line
        );

        // The second link must NOT be assigned to the same line as the first
        // if the text is actually on a different rendered line
        assert_ne!(
            first[0].line, second[0].line,
            "duplicate 'here' links should be on different lines after wrapping \
             (first line: {line_first:?}, second line: {line_second:?})"
        );
    }

    // Issue 3: links in lists inside blockquotes
    #[test]
    fn test_blockquote_list_contains_link_refs() {
        let md = "> - Visit [Rust](https://rust-lang.org) today";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let links: Vec<_> = doc
            .links()
            .iter()
            .filter(|l| l.url == "https://rust-lang.org")
            .collect();
        assert_eq!(
            links.len(),
            1,
            "link inside list in blockquote should be registered"
        );
        assert_eq!(links[0].text, "Rust");
    }
}
