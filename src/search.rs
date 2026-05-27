use ratatui::text::Line;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line_index: usize,
}

/// Search rendered lines for `query` (case-insensitive).
/// Returns one result per matching line (first match per line).
pub fn search_lines(lines: &[Line], query: &str) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }
    let q = query.to_lowercase();
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            let text: String = line
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
                .to_lowercase();
            if text.contains(q.as_str()) {
                Some(SearchResult { line_index: i })
            } else {
                None
            }
        })
        .collect()
}
