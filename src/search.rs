use ratatui::text::Line;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line_index: usize,
}

/// Case fold one character for matching.  Characters whose lowercase form is
/// more than one character (e.g. `İ`) are kept as they are, so a match found
/// in folded text has the same character offsets in the original text.
fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Character ranges `(start, len)` of every non-overlapping, case-insensitive
/// occurrence of `query` in `text`.  Plain text: no pattern syntax, nothing
/// in the query is interpreted.
pub fn find_matches(text: &str, query: &str) -> Vec<(usize, usize)> {
    let q: Vec<char> = query.chars().map(fold).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let t: Vec<char> = text.chars().map(fold).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + q.len() <= t.len() {
        if t[i..i + q.len()] == q[..] {
            out.push((i, q.len()));
            i += q.len();
        } else {
            i += 1;
        }
    }
    out
}

fn line_text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Search rendered lines for `query` (case-insensitive, literal).
/// Returns one result per matching line.
pub fn search_lines(lines: &[Line], query: &str) -> Vec<SearchResult> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !find_matches(&line_text(line), query).is_empty())
        .map(|(i, _)| SearchResult { line_index: i })
        .collect()
}
