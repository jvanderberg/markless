//! Markdown parsing with comrak.

use std::collections::HashMap;

use anyhow::Result;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::types::{
    CodeBlockRef, Document, HeadingRef, ImageRef, InlineSpan, InlineStyle, LineType, LinkRef,
    RenderedLine,
};

/// Parse markdown source into a Document.
///
/// # Example
///
/// ```
/// use gander::document::Document;
///
/// let doc = Document::parse("# Hello\n\nWorld").unwrap();
/// assert!(doc.line_count() >= 3); // heading + empty + paragraph + trailing empty
/// ```
impl Document {
    pub fn parse(source: &str) -> Result<Self> {
        parse(source)
    }

    pub fn parse_with_layout(source: &str, width: u16) -> Result<Self> {
        parse_with_layout(source, width, &HashMap::new())
    }

    pub fn parse_with_image_heights(
        source: &str,
        image_heights: &HashMap<String, usize>,
    ) -> Result<Self> {
        parse_with_image_heights(source, image_heights)
    }

    pub fn parse_with_layout_and_image_heights(
        source: &str,
        width: u16,
        image_heights: &HashMap<String, usize>,
    ) -> Result<Self> {
        parse_with_layout(source, width, image_heights)
    }
}

/// Parse markdown source into a Document.
pub fn parse(source: &str) -> Result<Document> {
    parse_with_layout(source, 80, &HashMap::new())
}

/// Parse markdown source into a Document with known image heights (in terminal rows).
pub fn parse_with_image_heights(
    source: &str,
    image_heights: &HashMap<String, usize>,
) -> Result<Document> {
    parse_with_layout(source, 80, image_heights)
}

/// Parse markdown source into a Document with layout and wrapping.
pub fn parse_with_layout(
    source: &str,
    width: u16,
    image_heights: &HashMap<String, usize>,
) -> Result<Document> {
    let arena = Arena::new();
    let options = create_options();
    let root = parse_document(&arena, source, &options);

    let mut lines = Vec::new();
    let mut headings = Vec::new();
    let mut images = Vec::new();
    let mut links = Vec::new();
    let mut code_blocks = Vec::new();

    let wrap_width = width.max(1) as usize;
    process_node(
        root,
        &mut lines,
        &mut headings,
        &mut images,
        &mut links,
        &mut code_blocks,
        0,
        image_heights,
        wrap_width,
        None,
    );

    Ok(Document::new(
        source.to_string(),
        lines,
        headings,
        images,
        links,
        code_blocks,
    ))
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
    options.extension.header_ids = Some("".to_string());
    options.extension.description_lists = true;

    options
}

fn process_node<'a>(
    node: &'a AstNode<'a>,
    lines: &mut Vec<RenderedLine>,
    headings: &mut Vec<HeadingRef>,
    images: &mut Vec<ImageRef>,
    links: &mut Vec<LinkRef>,
    code_blocks: &mut Vec<CodeBlockRef>,
    depth: usize,
    image_heights: &HashMap<String, usize>,
    wrap_width: usize,
    list_marker: Option<String>,
) {
    match &node.data.borrow().value {
        NodeValue::Document => {
            for child in node.children() {
                process_node(
                    child,
                    lines,
                    headings,
                    images,
                    links,
                    code_blocks,
                    depth,
                    image_heights,
                    wrap_width,
                    list_marker.clone(),
                );
            }
        }

        NodeValue::Heading(heading) => {
            let text = extract_text(node);

            // Keep headings visually separated with two rows above.
            ensure_trailing_empty_lines(lines, 2);
            let line_num = lines.len();

            headings.push(HeadingRef {
                level: heading.level,
                text: text.clone(),
                line: line_num,
                id: None, // TODO: Extract from header_ids
            });

            let prefix = "#".repeat(heading.level as usize);
            lines.push(RenderedLine::new(
                format!("{} {}", prefix, text),
                LineType::Heading(heading.level),
            ));
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Paragraph => {
            // Check if paragraph contains only an image (common case)
            let child_images = collect_paragraph_images(node);

            if !child_images.is_empty() {
                for (alt, src) in child_images {
                    let height_lines = image_heights
                        .get(&src)
                        .copied()
                        .unwrap_or(1)
                        .max(1);
                    let start_line = lines.len();

                    // First line shows the image placeholder/alt text
                    lines.push(RenderedLine::new(
                        format!("[Image: {}]", if alt.is_empty() { &src } else { &alt }),
                        LineType::Image,
                    ));

                    // Reserve additional lines for image content (empty Image lines)
                    for _ in 1..height_lines {
                        lines.push(RenderedLine::new(String::new(), LineType::Image));
                    }

                    let end_line = lines.len();
                    images.push(ImageRef {
                        alt: alt.clone(),
                        src: src.clone(),
                        line_range: start_line..end_line,
                    });
                }
                lines.push(RenderedLine::new(String::new(), LineType::Empty));
            } else {
                // Regular paragraph text with inline styling and wrapping
                let spans = collect_inline_spans(node);
                // Collect links from paragraph
                collect_inline_elements(node, lines.len(), images, links);

                let wrapped = wrap_spans(&spans, wrap_width, "", "");
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    lines.push(RenderedLine::with_spans(
                        content,
                        LineType::Paragraph,
                        line_spans,
                    ));
                }
                lines.push(RenderedLine::new(String::new(), LineType::Empty));
            }
        }

        NodeValue::CodeBlock(code_block) => {
            const CODE_RIGHT_PADDING: usize = 3;
            let info = code_block.info.clone();
            let literal = code_block.literal.clone();
            let language = info.split_whitespace().next().filter(|s| !s.is_empty());
            let content_width = literal
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0)
                .min(wrap_width.saturating_sub(4).max(1));
            let title = language.unwrap_or("code");
            let label = format!(" {} ", title);
            let frame_inner_width = content_width + 2 + CODE_RIGHT_PADDING;
            let top_label_width = frame_inner_width.min(label.chars().count());
            let visible_label: String = label.chars().take(top_label_width).collect();
            let top = format!(
                "┌{}{}┐",
                visible_label,
                "─".repeat(frame_inner_width.saturating_sub(visible_label.chars().count()))
            );
            lines.push(RenderedLine::new(top, LineType::CodeBlock));

            let body_start = lines.len();
            let raw_lines: Vec<String> = literal.lines().map(ToString::to_string).collect();
            for raw_line in &raw_lines {
                let mut plain_style = InlineStyle::default();
                plain_style.code = true;
                let spans = vec![InlineSpan::new(raw_line.clone(), plain_style)];
                let trimmed_spans = truncate_spans(&spans, content_width);
                let trimmed_len = spans_to_string(&trimmed_spans).chars().count();
                let padding =
                    " ".repeat(content_width.saturating_sub(trimmed_len) + CODE_RIGHT_PADDING);

                let mut line_spans = Vec::new();
                line_spans.push(InlineSpan::new("│ ".to_string(), InlineStyle::default()));
                line_spans.extend(trimmed_spans);
                line_spans.push(InlineSpan::new(
                    format!("{} │", padding),
                    InlineStyle::default(),
                ));
                let content = spans_to_string(&line_spans);
                lines.push(RenderedLine::with_spans(
                    content,
                    LineType::CodeBlock,
                    line_spans,
                ));
            }
            let body_end = lines.len();

            code_blocks.push(CodeBlockRef {
                line_range: body_start..body_end,
                language: language.map(ToString::to_string),
                raw_lines,
                highlighted: false,
                content_width,
                right_padding: CODE_RIGHT_PADDING,
            });

            lines.push(RenderedLine::new(
                format!("└{}┘", "─".repeat(frame_inner_width)),
                LineType::CodeBlock,
            ));
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
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
                        format!("{:>width$}{}", number, delimiter, width = number_width)
                    }
                };
                let marker = format!("{} ", base_marker);
                process_node(
                    child,
                    lines,
                    headings,
                    images,
                    links,
                    code_blocks,
                    list_depth,
                    image_heights,
                    wrap_width,
                    Some(marker),
                );
            }
        }

        NodeValue::TaskItem(symbol) => {
            let indent = "  ".repeat(depth.saturating_sub(1));
            let task_marker = if symbol.is_some() { "✓" } else { "□" };
            let marker = format!("{} ", task_marker);
            let prefix_first = format!("{}{}", indent, marker);
            let prefix_next = format!("{}{}", indent, " ".repeat(marker.len()));

            let spans = collect_inline_spans(node);
            let wrapped = wrap_spans(&spans, wrap_width, &prefix_first, &prefix_next);
            for line_spans in wrapped {
                let content = spans_to_string(&line_spans);
                lines.push(RenderedLine::with_spans(
                    content,
                    LineType::ListItem(depth),
                    line_spans,
                ));
            }

            for child in node.children() {
                if matches!(child.data.borrow().value, NodeValue::List(_)) {
                    process_node(
                        child,
                        lines,
                        headings,
                        images,
                        links,
                        code_blocks,
                        depth,
                        image_heights,
                        wrap_width,
                        None,
                    );
                }
            }
        }

        NodeValue::Item(_) => {
            let indent = "  ".repeat(depth.saturating_sub(1));
            let base_marker = list_marker.clone().unwrap_or_else(|| "- ".to_string());
            let task_marker = find_task_marker(node);
            let marker = if let Some(task_marker) = task_marker {
                format!("{} ", task_marker)
            } else {
                base_marker
            };
            let prefix_first = format!("{}{}", indent, marker);
            let prefix_next = format!("{}{}", indent, " ".repeat(marker.len()));
            let mut rendered_any = false;
            let mut rendered_paragraphs = 0usize;

            for child in node.children() {
                match &child.data.borrow().value {
                    NodeValue::Paragraph => {
                        if rendered_paragraphs > 0 {
                            lines.push(RenderedLine::new(String::new(), LineType::ListItem(depth)));
                        }
                        let spans = collect_inline_spans(child);
                        let prefix = if rendered_any {
                            &prefix_next
                        } else {
                            &prefix_first
                        };
                        let wrapped = wrap_spans(&spans, wrap_width, prefix, &prefix_next);

                        for line_spans in wrapped {
                            let content = spans_to_string(&line_spans);
                            lines.push(RenderedLine::with_spans(
                                content,
                                LineType::ListItem(depth),
                                line_spans,
                            ));
                        }
                        rendered_any = true;
                        rendered_paragraphs += 1;
                    }
                    NodeValue::TaskItem(_) => {
                        if rendered_paragraphs > 0 {
                            lines.push(RenderedLine::new(String::new(), LineType::ListItem(depth)));
                        }
                        let spans = collect_inline_spans(child);
                        let prefix = if rendered_any {
                            &prefix_next
                        } else {
                            &prefix_first
                        };
                        let wrapped = wrap_spans(&spans, wrap_width, prefix, &prefix_next);

                        for line_spans in wrapped {
                            let content = spans_to_string(&line_spans);
                            lines.push(RenderedLine::with_spans(
                                content,
                                LineType::ListItem(depth),
                                line_spans,
                            ));
                        }
                        rendered_any = true;
                        rendered_paragraphs += 1;
                    }
                    NodeValue::List(_) => {
                        process_node(
                            child,
                            lines,
                            headings,
                            images,
                            links,
                            code_blocks,
                            depth,
                            image_heights,
                            wrap_width,
                            None,
                        );
                    }
                    _ => {
                        process_node(
                            child,
                            lines,
                            headings,
                            images,
                            links,
                            code_blocks,
                            depth,
                            image_heights,
                            wrap_width,
                            None,
                        );
                    }
                }
            }

            if !rendered_any {
                let spans = collect_inline_spans(node);
                let wrapped = wrap_spans(&spans, wrap_width, &prefix_first, &prefix_next);
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    lines.push(RenderedLine::with_spans(
                        content,
                        LineType::ListItem(depth),
                        line_spans,
                    ));
                }
            }
        }

        NodeValue::BlockQuote => {
            render_blockquote(node, lines, wrap_width, 1);
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::ThematicBreak => {
            lines.push(RenderedLine::new(
                "---".to_string(),
                LineType::HorizontalRule,
            ));
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Table(_) => {
            for line in render_table(node, wrap_width) {
                lines.push(RenderedLine::new(line, LineType::Table));
            }
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::FootnoteDefinition(def) => {
            let label = format!("[^{}]: ", def.name);
            let continuation = " ".repeat(label.len());
            let spans = collect_inline_spans(node);
            let wrapped = wrap_spans(&spans, wrap_width, &label, &continuation);
            if wrapped.is_empty() {
                lines.push(RenderedLine::new(label, LineType::Paragraph));
            } else {
                for line_spans in wrapped {
                    let content = spans_to_string(&line_spans);
                    lines.push(RenderedLine::with_spans(
                        content,
                        LineType::Paragraph,
                        line_spans,
                    ));
                }
            }
            lines.push(RenderedLine::new(String::new(), LineType::Empty));
        }

        NodeValue::Image(image) => {
            let alt = extract_text(node);
            let src = image.url.clone();
            let line_num = lines.len();
            let height_lines = image_heights
                .get(&src)
                .copied()
                .unwrap_or(1)
                .max(1);

            images.push(ImageRef {
                alt: alt.clone(),
                src: src.clone(),
                line_range: line_num..line_num + height_lines,
            });

            lines.push(RenderedLine::new(
                format!("[Image: {}]", if alt.is_empty() { &src } else { &alt }),
                LineType::Image,
            ));

            for _ in 1..height_lines {
                lines.push(RenderedLine::new(String::new(), LineType::Image));
            }
        }

        _ => {
            // Process children for unhandled nodes
            for child in node.children() {
                process_node(
                    child,
                    lines,
                    headings,
                    images,
                    links,
                    code_blocks,
                    depth,
                    image_heights,
                    wrap_width,
                    list_marker.clone(),
                );
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
    wrap_width: usize,
    quote_depth: usize,
) {
    let prefix = quote_prefix(quote_depth);

    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Paragraph => {
                let spans = collect_inline_spans(child);
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
            NodeValue::BlockQuote => {
                render_blockquote(child, lines, wrap_width, quote_depth + 1);
            }
            _ => {
                let text = extract_text(child);
                for raw_line in text.lines() {
                    let spans = vec![InlineSpan::new(raw_line.to_string(), InlineStyle::default())];
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

fn render_table<'a>(table_node: &'a AstNode<'a>, wrap_width: usize) -> Vec<String> {
    let (alignments, mut rows, has_header) = collect_table_rows(table_node);
    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows.iter().map(std::vec::Vec::len).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }

    for row in &mut rows {
        while row.len() < num_cols {
            row.push(String::new());
        }
    }

    let mut col_widths = vec![1_usize; num_cols];
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            col_widths[idx] = col_widths[idx].max(display_width(cell));
        }
    }

    // Keep the table inside available width.
    // Table row width is: 1 + sum(col_width + 3) for all columns.
    let max_table_width = wrap_width.max(4);
    while 1 + col_widths.iter().sum::<usize>() + (3 * num_cols) > max_table_width {
        if let Some((widest_idx, _)) = col_widths.iter().enumerate().max_by_key(|(_, w)| *w) {
            if col_widths[widest_idx] > 1 {
                col_widths[widest_idx] -= 1;
            } else {
                break;
            }
        }
    }

    let top = render_table_border(&col_widths, '┌', '┬', '┐');
    let mid = render_table_border(&col_widths, '├', '┼', '┤');
    let bottom = render_table_border(&col_widths, '└', '┴', '┘');

    let mut lines = Vec::new();
    lines.push(top);
    for (idx, row) in rows.iter().enumerate() {
        lines.push(render_table_row(row, &col_widths, &alignments));
        if has_header && idx == 0 {
            lines.push(mid.clone());
        }
    }
    lines.push(bottom);
    lines
}

fn collect_table_rows<'a>(table_node: &'a AstNode<'a>) -> (Vec<TableAlignment>, Vec<Vec<String>>, bool) {
    let alignments = match &table_node.data.borrow().value {
        NodeValue::Table(table) => table.alignments.clone(),
        _ => Vec::new(),
    };

    let mut rows = Vec::new();
    let mut has_header = false;
    for row_node in table_node.children() {
        let is_header_row = matches!(row_node.data.borrow().value, NodeValue::TableRow(true));
        if is_header_row {
            has_header = true;
        }
        if !matches!(row_node.data.borrow().value, NodeValue::TableRow(_)) {
            continue;
        }

        let mut row_cells = Vec::new();
        for cell_node in row_node.children() {
            if !matches!(cell_node.data.borrow().value, NodeValue::TableCell) {
                continue;
            }
            let cell = extract_text(cell_node)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            row_cells.push(cell);
        }
        rows.push(row_cells);
    }

    (alignments, rows, has_header)
}

fn render_table_border(widths: &[usize], left: char, middle: char, right: char) -> String {
    let mut out = String::new();
    out.push(left);
    for (idx, width) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(width + 2));
        if idx + 1 < widths.len() {
            out.push(middle);
        }
    }
    out.push(right);
    out
}

fn render_table_row(cells: &[String], widths: &[usize], alignments: &[TableAlignment]) -> String {
    let mut out = String::new();
    out.push('│');
    for idx in 0..widths.len() {
        let content = cells.get(idx).map_or("", std::string::String::as_str);
        let content = truncate_text(content, widths[idx]);
        let padding = widths[idx].saturating_sub(display_width(&content));

        out.push(' ');
        match alignments.get(idx).copied().unwrap_or(TableAlignment::None) {
            TableAlignment::Right => {
                out.push_str(&" ".repeat(padding));
                out.push_str(&content);
            }
            TableAlignment::Center => {
                let left = padding / 2;
                let right = padding - left;
                out.push_str(&" ".repeat(left));
                out.push_str(&content);
                out.push_str(&" ".repeat(right));
            }
            TableAlignment::Left | TableAlignment::None => {
                out.push_str(&content);
                out.push_str(&" ".repeat(padding));
            }
        }
        out.push(' ');
        out.push('│');
    }
    out
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_chars {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn render_superscript_text(text: &str) -> String {
    render_script_text(text, true)
}

fn render_subscript_text(text: &str) -> String {
    render_script_text(text, false)
}

fn render_script_text(text: &str, superscript: bool) -> String {
    let mut mapped = String::new();
    for ch in text.chars() {
        let mapped_char = if superscript {
            superscript_char(ch)
        } else {
            subscript_char(ch)
        };
        let Some(mapped_char) = mapped_char else {
            return if superscript {
                format!("^({text})")
            } else {
                format!("_({text})")
            };
        };
        mapped.push(mapped_char);
    }
    mapped
}

fn superscript_char(ch: char) -> Option<char> {
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

fn subscript_char(ch: char) -> Option<char> {
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
        NodeValue::List(_) | NodeValue::Item(_) => {
            return;
        }
        NodeValue::Text(t) => {
            spans.push(InlineSpan::new(t.clone(), style));
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
            spans.push(InlineSpan::new(format!("[^{}]", reference.name), style));
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
            text.push_str(t);
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
            text.push_str(&format!("[^{}]", reference.name));
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
        if !prefix.is_empty() {
            current.push(InlineSpan::new(prefix.to_string(), InlineStyle::default()));
            *current_len = prefix.len();
        } else {
            *current_len = 0;
        }
        *has_word = false;
    };

    start_new_line(prefix_first, &mut current, &mut current_len, &mut has_word);

    for token in tokens {
        let token_len = token.text().chars().count();
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
        current.push(InlineSpan::new(prefix_first.to_string(), InlineStyle::default()));
    }

    lines.push(current);
    lines
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

/// Collect images from a paragraph node, returning (alt, src) pairs.
fn collect_paragraph_images<'a>(node: &'a AstNode<'a>) -> Vec<(String, String)> {
    let mut images = Vec::new();
    collect_paragraph_images_recursive(node, &mut images);
    images
}

fn collect_paragraph_images_recursive<'a>(node: &'a AstNode<'a>, images: &mut Vec<(String, String)>) {
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
        _ => {
            for child in node.children() {
                collect_inline_elements(child, base_line, images, links);
            }
        }
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
            assert!(line.content().len() <= 30);
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
        assert!(table_lines[0].content().starts_with('┌'));
        assert!(table_lines.iter().any(|l| l.content().starts_with("│ A")));
        assert!(table_lines.iter().any(|l| l.content().contains("│ 1")));
        assert!(table_lines.last().unwrap().content().starts_with('└'));
    }

    #[test]
    fn test_gfm_table_respects_layout_width() {
        let md = "| Very long heading | Value |\n|---|---:|\n| some really long content | 12345 |";
        let doc = Document::parse_with_layout(md, 24).unwrap();
        let lines = doc.visible_lines(0, 20);
        for line in lines.iter().filter(|l| *l.line_type() == LineType::Table) {
            assert!(
                unicode_width::UnicodeWidthStr::width(line.content()) <= 24,
                "table line exceeds width: {}",
                line.content()
            );
        }
    }

    #[test]
    fn test_gfm_table_with_emoji_respects_layout_width() {
        let md = "| Feature | Status |\n|---|---|\n| Bold | ✅ Supported |\n| Italic | ✅ Supported |";
        let doc = Document::parse_with_layout(md, 28).unwrap();
        let lines = doc.visible_lines(0, 20);
        for line in lines.iter().filter(|l| *l.line_type() == LineType::Table) {
            assert!(
                unicode_width::UnicodeWidthStr::width(line.content()) <= 28,
                "emoji table line exceeds width: {}",
                line.content()
            );
        }
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

        assert_eq!(table_lines.len(), 9);
        assert!(table_lines.iter().all(|line| !line.contains('\n')));
        assert!(table_lines.iter().any(|line| line.contains("Bold")));
        assert!(table_lines.iter().any(|line| line.contains("Italic")));
        assert!(table_lines.iter().any(|line| line.contains("Code")));
        assert!(table_lines.iter().any(|line| line.contains("Strike")));
        assert!(table_lines.iter().any(|line| line.contains("Links")));
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
            assert!(line.content().len() <= 20);
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
        let md = "Alpha[^n]\n\n[^n]: Footnote text";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let lines = doc.visible_lines(0, 20);
        assert!(lines.iter().any(|l| l.content().contains("[^n]")));
        assert!(lines.iter().any(|l| l.content().contains("[^n]:")));
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
        let top_width = code_lines.first().unwrap().content().chars().count();
        for line in &code_lines {
            assert_eq!(line.content().chars().count(), top_width);
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
}
