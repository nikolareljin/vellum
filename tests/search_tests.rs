use ratatui::text::Line;
use vellum::search::search_lines;

#[test]
fn test_search_finds_match() {
    let lines = vec![
        Line::from("Hello world"),
        Line::from("Rust is great"),
        Line::from("Hello again"),
    ];
    let results = search_lines(&lines, "hello");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].line_index, 0);
    assert_eq!(results[1].line_index, 2);
}

#[test]
fn test_search_case_insensitive() {
    let lines = vec![
        Line::from("UPPER"),
        Line::from("lower"),
        Line::from("Mixed"),
    ];
    let results = search_lines(&lines, "upper");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_index, 0);
}

#[test]
fn test_search_no_match() {
    let lines = vec![Line::from("nothing here")];
    let results = search_lines(&lines, "xyz");
    assert!(results.is_empty());
}

#[test]
fn test_search_empty_query_returns_nothing() {
    let lines = vec![Line::from("some text")];
    let results = search_lines(&lines, "");
    assert!(results.is_empty());
}
