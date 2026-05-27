use vellum::parser::{parse, Element, Span};

#[test]
fn test_heading_level_1() {
    let elements = parse("# Hello World");
    assert_eq!(
        elements,
        vec![Element::Heading {
            level: 1,
            text: "Hello World".into()
        }]
    );
}

#[test]
fn test_heading_level_3() {
    let elements = parse("### Deep heading");
    assert_eq!(
        elements,
        vec![Element::Heading {
            level: 3,
            text: "Deep heading".into()
        }]
    );
}

#[test]
fn test_plain_paragraph() {
    let elements = parse("Hello world.");
    assert_eq!(
        elements,
        vec![Element::Paragraph(vec![Span::Text("Hello world.".into())])]
    );
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
    assert_eq!(
        elements,
        vec![Element::CodeBlock {
            lang: Some("rust".into()),
            code: "fn main() {}\n".into(),
        }]
    );
}

#[test]
fn test_code_block_no_lang() {
    let md = "```\nplain code\n```";
    let elements = parse(md);
    assert_eq!(
        elements,
        vec![Element::CodeBlock {
            lang: None,
            code: "plain code\n".into()
        }]
    );
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
    let Some(Element::Paragraph(spans)) = elements.first() else {
        panic!("no paragraph")
    };
    assert!(spans
        .iter()
        .any(|s| matches!(s, Span::Link { href, .. } if href == "https://example.com")));
}

#[test]
fn test_image_element() {
    let elements = parse("![alt text](image.png)");
    assert!(
        elements
            .iter()
            .any(|e| matches!(e, Element::Image { alt, src }
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

#[test]
fn test_link_with_inline_code_label() {
    // [`PLAN.md`](./PLAN.md) — backtick text is link label, not standalone Code span
    let elements = parse("[`PLAN.md`](./PLAN.md)");
    let Some(Element::Paragraph(spans)) = elements.first() else {
        panic!("no paragraph")
    };
    let link = spans.iter().find_map(|s| {
        if let Span::Link { text, href } = s {
            Some((text.as_str(), href.as_str()))
        } else {
            None
        }
    });
    assert_eq!(
        link,
        Some(("PLAN.md", "./PLAN.md")),
        "backtick link label must be non-empty; got spans: {spans:?}"
    );
}

#[test]
fn test_link_in_list_item_collected() {
    // List items containing links must produce Paragraph spans with Link spans
    let md = "- [`PLAN.md`](./PLAN.md) — description\n";
    let elements = parse(md);
    let list = elements.iter().find(|e| matches!(e, Element::List { .. }));
    assert!(list.is_some(), "expected a List element");
    if let Some(Element::List { items, .. }) = list {
        let has_link = items.iter().flatten().any(|el| {
            if let Element::Paragraph(spans) = el {
                spans
                    .iter()
                    .any(|s| matches!(s, Span::Link { text, .. } if !text.is_empty()))
            } else {
                false
            }
        });
        assert!(
            has_link,
            "list item must contain a non-empty Link span; got: {items:?}"
        );
    }
}

#[test]
fn test_multiline_tight_list_code_links() {
    // Mirrors the real TEST.md fixture "Further reading" section:
    // tight list, each item is a backtick-label link with a continuation line.
    let md = concat!(
        "- [`PLAN.md`](./PLAN.md) — full plan,\n",
        "  scope decisions\n",
        "- [`docs/arch.md`](./docs/arch.md) — components,\n",
        "  trust boundaries\n",
    );
    let elements = parse(md);
    let list = elements.iter().find(|e| matches!(e, Element::List { .. }));
    assert!(list.is_some(), "expected a List element");
    if let Some(Element::List { items, .. }) = list {
        assert_eq!(items.len(), 2, "expected 2 list items; got {items:?}");
        for (idx, item) in items.iter().enumerate() {
            let has_link = item.iter().any(|el| {
                if let Element::Paragraph(spans) = el {
                    spans
                        .iter()
                        .any(|s| matches!(s, Span::Link { text, .. } if !text.is_empty()))
                } else {
                    false
                }
            });
            assert!(
                has_link,
                "item {idx} must have a non-empty Link span; got: {item:?}"
            );
        }
    }
}

#[test]
fn test_further_reading_nine_items() {
    // Full TEST.md fixture "Further reading" — 9 items, each a code-label link
    // with a soft-wrapped continuation line. All 9 must parse with a Link span.
    let md = concat!(
        "## Further reading\n\n",
        "- [`PLAN.md`](./PLAN.md) — full implementation plan, phase breakdown,\n",
        "  acceptance gates, scope decisions\n",
        "- [`docs/architecture.md`](./docs/architecture.md) — components, trust\n",
        "  boundaries, deployment topologies\n",
        "- [`docs/ai-pipeline.md`](./docs/ai-pipeline.md) — provider abstraction,\n",
        "  fallback ladder, the 9 AI functions\n",
        "- [`docs/security-and-safety.md`](./docs/security-and-safety.md) — the\n",
        "  defensive posture policy\n",
        "- [`docs/firewall-integrations.md`](./docs/firewall-integrations.md) —\n",
        "  five firewall providers, the approval workflow\n",
        "- [`docs/api-examples.md`](./docs/api-examples.md) — curl recipes for\n",
        "  every endpoint\n",
        "- [`docs/install-native.md`](./docs/install-native.md) — non-Docker\n",
        "  install\n",
        "- [`docs/kafka-design.md`](./docs/kafka-design.md) — where Kafka fits\n",
        "- [`docs/release-pipeline.md`](./docs/release-pipeline.md) — separate\n",
        "  release notes and distribution details\n",
    );
    let elements = parse(md);
    let list = elements.iter().find(|e| matches!(e, Element::List { .. }));
    assert!(list.is_some(), "expected a List element");
    if let Some(Element::List { items, .. }) = list {
        assert_eq!(items.len(), 9, "expected 9 list items; got {}", items.len());
        for (idx, item) in items.iter().enumerate() {
            let has_link = item.iter().any(|el| {
                if let Element::Paragraph(spans) = el {
                    spans
                        .iter()
                        .any(|s| matches!(s, Span::Link { text, .. } if !text.is_empty()))
                } else {
                    false
                }
            });
            assert!(
                has_link,
                "item {idx} must have a non-empty Link span; got: {item:?}"
            );
        }
    }
}

#[test]
fn test_full_readme_further_reading_nine_items() {
    // Parse the TEST.md fixture — context before Further reading section matters.
    let md = include_str!("../TEST.md");
    let elements = parse(md);

    // Find the Further reading List element by finding the heading index first
    let fr_pos = elements.iter().position(
        |e| matches!(e, Element::Heading { text, .. } if text.contains("Further reading")),
    );
    assert!(
        fr_pos.is_some(),
        "Could not find 'Further reading' heading in parsed elements"
    );
    let fr_pos = fr_pos.unwrap();

    // The list should be the next element after the heading
    let list = elements.get(fr_pos + 1);
    assert!(
        matches!(list, Some(Element::List { .. })),
        "Element after 'Further reading' heading should be a List; got: {:?}",
        list
    );

    if let Some(Element::List { items, .. }) = list {
        assert_eq!(
            items.len(),
            9,
            "expected 9 list items in Further reading; got {}. Items:\n{:#?}",
            items.len(),
            items
        );
    }
}
