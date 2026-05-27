use vellum::links::{anchor_from_heading, build_anchor_map};
use vellum::parser::Element;

#[test]
fn test_anchor_from_heading_lowercases_and_hyphens() {
    assert_eq!(anchor_from_heading("Hello World"), "hello-world");
}

#[test]
fn test_anchor_strips_special_chars() {
    assert_eq!(anchor_from_heading("Hello, World!"), "hello-world");
}

#[test]
fn test_build_anchor_map_returns_line_offsets() {
    let elements = vec![
        Element::Heading { level: 1, text: "Introduction".into() },
        Element::Paragraph(vec![]),
        Element::Heading { level: 2, text: "Details".into() },
    ];
    let map = build_anchor_map(&elements);
    assert!(map.contains_key("introduction"), "should have 'introduction' key");
    assert!(map.contains_key("details"), "should have 'details' key");
    assert!(map["introduction"] < map["details"], "introduction should come before details");
}

#[test]
fn test_full_readme_collect_links_count() {
    use vellum::links::collect_links;
    let md = include_str!("/home/nikos/Projects/netwise-ai/README.md");
    let elements = vellum::parser::parse(md);
    let links = collect_links(&elements);
    println!("Total links found by collect_links: {}", links.len());
    for (i, (href, line)) in links.iter().enumerate() {
        println!("  [{i}] line={line} href={href:?}");
    }
    // Should find at least 9 links from Further reading
    assert!(links.len() >= 9, "expected ≥9 links, got {}: {links:?}", links.len());
}
