use vellum::parser::parse;
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

#[test]
fn test_further_reading_renders_nine_link_lines() {
    // Each list item must render as a separate visible line containing the link text.
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
        "  `netwise-ai-release` repo for distribution\n",
    );
    let elements = parse(md);
    let lines = render_elements(&elements);

    // Collect non-empty line texts
    let visible: Vec<String> = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .filter(|s: &String| !s.trim().is_empty())
        .collect();

    // Count lines that contain a bullet + a link label
    let bullet_lines: Vec<&String> = visible.iter().filter(|s| s.contains('•')).collect();
    assert_eq!(
        bullet_lines.len(), 9,
        "expected 9 bulleted item lines; got {}. Lines:\n{:#?}",
        bullet_lines.len(), visible
    );
}

#[test]
fn test_full_readme_further_reading_nine_bullets() {
    use vellum::parser::parse;
    // Parse the FULL README and render it — then count bullet lines for Further reading
    let md = include_str!("/home/nikos/Projects/netwise-ai/README.md");
    let elements = parse(md);

    // Find Further reading List
    let fr_heading_pos = elements.iter().position(|e| {
        matches!(e, vellum::parser::Element::Heading { text, .. } if text.contains("Further reading"))
    }).expect("Further reading heading not found");

    let list_el = elements.get(fr_heading_pos + 1)
        .expect("No element after Further reading heading");

    assert!(
        matches!(list_el, vellum::parser::Element::List { .. }),
        "Expected List after heading, got: {list_el:?}"
    );

    let list_lines = render_elements(std::slice::from_ref(list_el));
    let bullet_lines: Vec<_> = list_lines.iter()
        .filter(|l| l.spans.iter().any(|s| s.content.contains('•')))
        .collect();

    assert_eq!(
        bullet_lines.len(), 9,
        "Expected 9 bullet lines from full-README parse; got {}.\nAll lines: {:#?}",
        bullet_lines.len(),
        list_lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>()
    );
}

#[test]
fn test_full_readme_total_display_lines() {
    // Report total display lines and location of Further reading section
    // to diagnose scroll/viewport issues
    let md = include_str!("/home/nikos/Projects/netwise-ai/README.md");
    let elements = vellum::parser::parse(md);
    
    let mut total_lines = 0usize;
    let mut further_reading_start = None;
    let mut further_reading_end = None;
    let mut found_fr_heading = false;
    
    for el in &elements {
        if matches!(el, vellum::parser::Element::Heading { text, .. } if text.contains("Further reading")) {
            further_reading_start = Some(total_lines);
            found_fr_heading = true;
        }
        if found_fr_heading && further_reading_end.is_none() && matches!(el, vellum::parser::Element::List { .. }) {
            let list_lines = render_elements(std::slice::from_ref(el));
            further_reading_end = Some(total_lines + list_lines.len());
            total_lines += list_lines.len();
            continue;
        }
        let rendered = render_elements(std::slice::from_ref(el));
        total_lines += rendered.len();
    }
    
    println!("Total display lines: {total_lines}");
    println!("Further reading heading at line: {:?}", further_reading_start);
    println!("Further reading list ends at line: {:?}", further_reading_end);
    println!("Lines in Further reading section: {:?}", 
        further_reading_end.zip(further_reading_start).map(|(e, s)| e - s + 2));
    
    // The section must be reachable (not beyond max-scroll) for a 40-row terminal
    // max_scroll = total_lines - 40
    let viewport_height = 40usize;
    let max_scroll = total_lines.saturating_sub(viewport_height);
    if let Some(start) = further_reading_start {
        let visible_from_bottom = total_lines.saturating_sub(max_scroll);
        println!("At goto_bottom (scroll={max_scroll}), last {visible_from_bottom} lines visible");
        println!("Further reading heading at line {start}, visible from bottom: {}", start >= max_scroll);
    }
    
    // Pass always — this is a diagnostic test
    assert!(total_lines > 0);
}

#[test]
fn test_full_readme_elements_near_further_reading() {
    let md = include_str!("/home/nikos/Projects/netwise-ai/README.md");
    let elements = vellum::parser::parse(md);
    
    // Find Further reading heading
    let fr_pos = elements.iter().position(|e| {
        matches!(e, vellum::parser::Element::Heading { text, .. } if text.contains("Further reading"))
    }).expect("Further reading heading not found");
    
    println!("Total elements: {}", elements.len());
    println!("Further reading at index {fr_pos}");
    
    // Print 3 elements before + all after
    for i in fr_pos.saturating_sub(3)..elements.len() {
        match &elements[i] {
            vellum::parser::Element::Heading { level, text } => 
                println!("[{i}] Heading H{level}: {text:?}"),
            vellum::parser::Element::Paragraph(spans) => {
                let txt: String = spans.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ");
                println!("[{i}] Paragraph ({} spans): {}", spans.len(), &txt[..txt.len().min(80)]);
            }
            vellum::parser::Element::List { ordered, items } => 
                println!("[{i}] List (ordered={ordered}, {} items)", items.len()),
            vellum::parser::Element::CodeBlock { lang, .. } => 
                println!("[{i}] CodeBlock (lang={lang:?})"),
            vellum::parser::Element::Image { alt, src } => 
                println!("[{i}] Image alt={alt:?} src={src:?}"),
            e => println!("[{i}] {:?}", std::mem::discriminant(e)),
        }
    }
    
    assert!(true);
}

#[test]
fn test_full_readme_bullet_line_content() {
    let md = include_str!("/home/nikos/Projects/netwise-ai/README.md");
    let elements = vellum::parser::parse(md);
    
    let fr_pos = elements.iter().position(|e| {
        matches!(e, vellum::parser::Element::Heading { text, .. } if text.contains("Further reading"))
    }).expect("FR heading not found");
    
    let list_el = elements.get(fr_pos + 1).expect("no list after heading");
    let list_lines = render_elements(std::slice::from_ref(list_el));
    
    println!("All lines from Further reading list:");
    for (i, line) in list_lines.iter().enumerate() {
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        println!("  [{i}] {:?}", content);
    }
    assert!(true);
}
