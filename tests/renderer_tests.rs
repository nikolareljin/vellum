use vellum::highlight::highlight_code;

#[test]
fn test_highlight_returns_lines() {
    let lines = highlight_code("fn main() {}", Some("rust"));
    assert!(!lines.is_empty(), "should return at least one styled line");
    assert!(!lines[0].is_empty());
}

#[test]
fn test_highlight_unknown_lang_falls_back() {
    let lines = highlight_code("hello world", Some("nonexistent_lang_xyz"));
    assert!(!lines.is_empty());
}

#[test]
fn test_highlight_no_lang() {
    let lines = highlight_code("plain text", None);
    assert!(!lines.is_empty());
}
