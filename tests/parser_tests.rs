use vellum::parser::{parse, Element, Span};

#[test]
fn test_heading_level_1() {
    let elements = parse("# Hello World");
    assert_eq!(elements, vec![Element::Heading { level: 1, text: "Hello World".into() }]);
}

#[test]
fn test_heading_level_3() {
    let elements = parse("### Deep heading");
    assert_eq!(elements, vec![Element::Heading { level: 3, text: "Deep heading".into() }]);
}

#[test]
fn test_plain_paragraph() {
    let elements = parse("Hello world.");
    assert_eq!(elements, vec![Element::Paragraph(vec![Span::Text("Hello world.".into())])]);
}

#[test]
fn test_bold_span() {
    let elements = parse("**bold text**");
    assert!(matches!(
        elements.first(),
        Some(Element::Paragraph(spans)) if spans.iter().any(|s| matches!(s, Span::Bold(_)))
    ));
}

#[test]
fn test_strikethrough_span() {
    let elements = parse("~~crossed out~~");
    assert!(
        matches!(
            elements.first(),
            Some(Element::Paragraph(spans)) if spans.iter().any(|s| matches!(s, Span::Strikethrough(_)))
        ),
        "expected Strikethrough span in paragraph, got: {elements:?}"
    );
}

#[test]
fn test_code_block_with_lang() {
    let md = "```rust\nfn main() {}\n```";
    let elements = parse(md);
    assert_eq!(elements, vec![Element::CodeBlock {
        lang: Some("rust".into()),
        code: "fn main() {}\n".into(),
    }]);
}

#[test]
fn test_code_block_no_lang() {
    let md = "```\nplain code\n```";
    let elements = parse(md);
    assert_eq!(elements, vec![Element::CodeBlock { lang: None, code: "plain code\n".into() }]);
}

#[test]
fn test_hrule() {
    let elements = parse("---");
    assert!(elements.contains(&Element::HRule));
}

#[test]
fn test_inline_code_span() {
    let elements = parse("`inline_code`");
    assert!(matches!(
        elements.first(),
        Some(Element::Paragraph(spans)) if spans.iter().any(|s| matches!(s, Span::Code(_)))
    ));
}

#[test]
fn test_link_span() {
    let elements = parse("[visit](https://example.com)");
    let Some(Element::Paragraph(spans)) = elements.first() else { panic!("no paragraph") };
    assert!(spans.iter().any(|s| matches!(s, Span::Link { href, .. } if href == "https://example.com")));
}

#[test]
fn test_image_element() {
    let elements = parse("![alt text](image.png)");
    assert!(
        elements.iter().any(|e| matches!(e, Element::Image { alt, src }
            if alt == "alt text" && src == "image.png")),
        "expected Element::Image, got: {elements:?}"
    );
}

#[test]
fn test_video_element_classified_by_parser() {
    // Parser should emit Element::Video (not Element::Image) for video extensions
    for ext in &["mp4", "webm", "mov", "avi", "mkv"] {
        let md = format!("![clip](video.{ext})");
        let elements = parse(&md);
        assert!(
            elements.iter().any(|e| matches!(e, Element::Video { .. })),
            "extension .{ext} should produce Element::Video, got: {elements:?}"
        );
    }
}

#[test]
fn test_image_not_classified_as_video() {
    let elements = parse("![photo](photo.png)");
    assert!(
        !elements.iter().any(|e| matches!(e, Element::Video { .. })),
        "png should not produce Element::Video"
    );
}
