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

use vellum::search::find_matches;

#[test]
fn test_find_matches_is_literal() {
    // Regex and glob syntax are plain characters.
    assert_eq!(find_matches("a.c abc", "a.c"), vec![(0, 3)]);
    assert_eq!(find_matches("x*y and xxy", "x*y"), vec![(0, 3)]);
    assert_eq!(find_matches("[copy] (a|b)", "(a|b)"), vec![(7, 5)]);
    assert_eq!(find_matches("$(rm -rf ~)", "$(rm"), vec![(0, 4)]);
}

#[test]
fn test_find_matches_case_insensitive_all_occurrences() {
    assert_eq!(
        find_matches("Foo foo FOO", "foo"),
        vec![(0, 3), (4, 3), (8, 3)]
    );
    assert_eq!(find_matches("aaaa", "aa"), vec![(0, 2), (2, 2)]);
    assert!(find_matches("abc", "").is_empty());
    assert!(find_matches("ab", "abc").is_empty());
}

#[test]
fn test_find_matches_offsets_are_chars_not_bytes() {
    // Multi-byte and multi-char-lowercase text keeps char offsets aligned.
    assert_eq!(find_matches("Ünïcödé café", "CAFÉ"), vec![(8, 4)]);
    assert_eq!(find_matches("İstanbul x", "x"), vec![(9, 1)]);
}
