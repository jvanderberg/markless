//! Search functionality.
//!
//! Provides text search within documents with:
//! - Forward and backward search
//! - Case-insensitive option
//! - Match highlighting

use crate::document::Document;

/// Find matching rendered line indices for a query (case-insensitive).
///
/// For hex-mode documents, generates each hex line on the fly and matches
/// against offsets, hex bytes, and the ASCII column.
pub fn find_matches(document: &Document, query: &str) -> Vec<usize> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let needle = trimmed.to_lowercase();
    if document.is_hex_mode() {
        return (0..document.line_count())
            .filter(|&idx| {
                document
                    .hex_line_content(idx)
                    .is_some_and(|text| text.to_lowercase().contains(&needle))
            })
            .collect();
    }
    document
        .visible_lines(0, document.line_count())
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let haystack = line.content().to_lowercase();
            haystack.contains(&needle).then_some(idx)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_case_insensitive() {
        let doc = Document::parse("Alpha\n\nbeta\n\nALPHA alpha").unwrap();
        let matches = find_matches(&doc, "alpha");
        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_find_matches_empty_query() {
        let doc = Document::parse("Alpha").unwrap();
        assert!(find_matches(&doc, "").is_empty());
    }

    #[test]
    fn test_find_matches_hex_by_ascii() {
        // "Hello" in bytes followed by nulls
        let bytes = b"Hello\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
        let doc = Document::from_hex("test.bin", bytes);
        let matches = find_matches(&doc, "Hello");
        assert!(!matches.is_empty(), "should find ASCII text in hex dump");
        // Match should be on a hex line (after 4 header lines)
        assert!(matches[0] >= 4);
    }

    #[test]
    fn test_find_matches_hex_by_hex_bytes() {
        let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00];
        let doc = Document::from_hex("test.bin", bytes);
        let matches = find_matches(&doc, "de ad be ef");
        assert!(!matches.is_empty(), "should find hex byte patterns");
    }

    #[test]
    fn test_find_matches_hex_header() {
        let bytes = vec![0x00; 32];
        let doc = Document::from_hex("special.bin", bytes);
        let matches = find_matches(&doc, "special.bin");
        assert!(!matches.is_empty(), "should find filename in header");
        assert_eq!(matches[0], 0, "filename match should be on heading line");
    }
}
