//! Search functionality.
//!
//! Provides text search within documents with:
//! - Forward and backward search
//! - Case-insensitive option
//! - Match highlighting

use crate::document::Document;

/// Find matching rendered line indices for a query (case-insensitive).
pub fn find_matches(document: &Document, query: &str) -> Vec<usize> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let needle = trimmed.to_lowercase();
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
}
