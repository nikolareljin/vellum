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

use vellum::renderer::render_elements;
use vellum::parser::{Element, Span};
use ratatui::style::Modifier;

#[test]
fn test_heading_h1_is_bold() {
    let el = Element::Heading { level: 1, text: "Title".into() };
    let lines = render_elements(&[el]);
    assert!(!lines.is_empty());
    assert!(
        lines[0].spans.iter().any(|s| s.style.add_modifier.contains(Modifier::BOLD)),
        "h1 should be bold"
    );
}

#[test]
fn test_hrule_contains_line_chars() {
    let el = Element::HRule;
    let lines = render_elements(&[el]);
    assert!(!lines.is_empty());
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains('─'), "hrule should contain box-drawing dashes");
}

#[test]
fn test_paragraph_plain_text() {
    let el = Element::Paragraph(vec![Span::Text("Hello".into())]);
    let lines = render_elements(&[el]);
    let text: String = lines.iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("Hello"));
}
