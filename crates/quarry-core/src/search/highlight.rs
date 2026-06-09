//! Snippet extraction.
//!
//! A result list is far more useful when it shows *why* a document matched.
//! That has to work from the stored original text rather than the index:
//! analysis is lossy, so the term `run` cannot be turned back into the word
//! `running` that the user needs to see highlighted.
//!
//! The approach is to re-analyze the stored text with the same analyzer, which
//! yields byte offsets for each term, then pick the densest window of matches.

use crate::tokenize::Analyzer;

/// How a snippet is built.
#[derive(Debug, Clone)]
pub struct HighlightOptions {
    /// Target snippet width in characters.
    pub width: usize,
    /// Inserted before a matched term.
    pub open: String,
    /// Inserted after a matched term.
    pub close: String,
    /// Inserted where text was cut.
    pub ellipsis: String,
    /// Most snippets to return per field.
    pub max_snippets: usize,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        Self {
            width: 160,
            open: "<mark>".to_string(),
            close: "</mark>".to_string(),
            ellipsis: "…".to_string(),
            max_snippets: 1,
        }
    }
}

impl HighlightOptions {
    /// Options that mark matches with `*asterisks*`, for terminal output.
    #[must_use]
    pub fn terminal() -> Self {
        Self {
            open: "\u{1b}[1;33m".to_string(),
            close: "\u{1b}[0m".to_string(),
            ..Self::default()
        }
    }

    /// Options that wrap matches in plain brackets, for tests and plain text.
    #[must_use]
    pub fn brackets() -> Self {
        Self {
            open: "[".to_string(),
            close: "]".to_string(),
            ..Self::default()
        }
    }
}

/// A byte range in the original text that matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start.
    pub start: usize,
    /// Exclusive end.
    pub end: usize,
}

/// Finds the spans of `text` whose analyzed form is in `terms`.
///
/// `terms` must already be analyzed — the caller passes the same terms the
/// query was turned into, so a query for `running` highlights `Running`.
#[must_use]
pub fn find_spans(analyzer: &Analyzer, text: &str, terms: &[String]) -> Vec<Span> {
    if terms.is_empty() {
        return Vec::new();
    }

    analyzer
        .analyze(text)
        .into_iter()
        .filter(|token| terms.contains(&token.text))
        .map(|token| Span {
            start: token.start,
            end: token.end,
        })
        .collect()
}

/// Builds a highlighted snippet, or `None` when nothing matched.
#[must_use]
pub fn snippet(
    analyzer: &Analyzer,
    text: &str,
    terms: &[String],
    options: &HighlightOptions,
) -> Option<String> {
    let spans = find_spans(analyzer, text, terms);
    if spans.is_empty() {
        return None;
    }

    let window = best_window(text, &spans, options.width);
    Some(render(text, &spans, window, options))
}

/// Returns the leading `width` characters, marking any matches in them.
///
/// Used when a document matched on another field: there is nothing to centre
/// on here, but the start of the text is still worth showing.
#[must_use]
pub fn prefix_snippet(text: &str, options: &HighlightOptions) -> String {
    let mut end = text.len();
    if text.chars().count() > options.width {
        end = text
            .char_indices()
            .nth(options.width)
            .map_or(text.len(), |(i, _)| i);
        return format!("{}{}", &text[..end], options.ellipsis);
    }
    text[..end].to_string()
}

/// The byte window containing the most match spans.
///
/// Walks each span as a candidate start and counts how many fit within `width`
/// of it. Quadratic in the number of spans in the worst case, which is fine:
/// the span count is bounded by the matches in one field of one document, and
/// snippets are only built for the handful of results actually returned.
fn best_window(text: &str, spans: &[Span], width: usize) -> Span {
    let byte_width = width.saturating_mul(2).max(width);

    let mut best = Span {
        start: spans[0].start,
        end: spans[0].end,
    };
    let mut best_count = 0;

    for anchor in spans {
        // Centre the window on the anchor rather than starting at it, so a
        // match in the middle of a sentence keeps its lead-in.
        let half = byte_width / 2;
        let start = anchor.start.saturating_sub(half);
        let end = (start + byte_width).min(text.len());
