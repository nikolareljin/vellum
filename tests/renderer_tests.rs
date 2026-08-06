use vellum::highlight::highlight_code;
use vellum::parser::parse;
use vellum::theme::{CodeColors, Theme};

// ── Highlight tests ───────────────────────────────────────────────────────────

#[test]
fn test_highlight_returns_lines() {
    let cc = CodeColors::default();
    let lines = highlight_code("fn main() {}", Some("rust"), &cc);
    assert!(!lines.is_empty(), "should return at least one styled line");
    assert!(!lines[0].is_empty());
}

#[test]
fn test_highlight_unknown_lang_falls_back() {
    let cc = CodeColors::default();
    let lines = highlight_code("hello world", Some("nonexistent_lang_xyz"), &cc);
    assert!(!lines.is_empty());
}

#[test]
fn test_highlight_no_lang() {
    let cc = CodeColors::default();
    let lines = highlight_code("plain text", None, &cc);
    assert!(!lines.is_empty());
}

#[test]
fn test_by_language_override_normalizes_info_string() {
    // "Solarized (light)" is bundled with syntect and differs from the default
    // "base16-ocean.dark", so all three info-string forms should produce
    // identical output to each other (same theme selected after normalization)
    // and differ from the default-theme output.
    let mut cc = CodeColors::default();
    cc.by_language
        .insert("rust".to_string(), "Solarized (light)".to_string());

    let plain = highlight_code("let x = 1;", Some("rust"), &cc);
    let spaced = highlight_code("let x = 1;", Some("rust ignore"), &cc);
    let comma = highlight_code("let x = 1;", Some("rust,no_run"), &cc);

    assert!(!plain.is_empty());
    assert_eq!(plain, spaced, "rust ignore should normalize to rust");
    assert_eq!(plain, comma, "rust,no_run should normalize to rust");

    // The override must actually change the output vs default theme.
    let default_out = highlight_code("let x = 1;", Some("rust"), &CodeColors::default());
    assert_ne!(
        plain, default_out,
        "Solarized (light) should differ from base16-ocean.dark"
    );
}

// ── Renderer tests ─────────────────────────────────────────────────────────────

use ratatui::style::Modifier;
use vellum::parser::{Element, Span};
use vellum::renderer::render_elements;

#[test]
fn test_heading_h1_is_bold() {
    let el = Element::Heading {
        level: 1,
        text: "Title".into(),
    };
    let theme = Theme::default();
    let lines = render_elements(&[el], &theme);
    assert!(!lines.is_empty());
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD)),
        "h1 should be bold"
    );
}

#[test]
fn test_hrule_contains_line_chars() {
    let el = Element::HRule;
    let theme = Theme::default();
    let lines = render_elements(&[el], &theme);
    assert!(!lines.is_empty());
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains('─'),
        "hrule should contain box-drawing dashes"
    );
}

#[test]
fn test_paragraph_plain_text() {
    let el = Element::Paragraph(vec![Span::Text("Hello".into())]);
    let theme = Theme::default();
    let lines = render_elements(&[el], &theme);
    let text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
        .collect();
    assert!(text.contains("Hello"));
}

#[test]
fn test_heading_h1_uses_theme_color() {
    use ratatui::style::Color;
    use vellum::theme::{HeadingColors, Rgb};

    // h1 overridden to bright red; everything else default.
    let theme = Theme {
        headings: HeadingColors {
            h1: Rgb(255, 0, 0),
            ..HeadingColors::default()
        },
        ..Theme::default()
    };
    let el = Element::Heading {
        level: 1,
        text: "Test".into(),
    };
    let lines = render_elements(&[el], &theme);
    // At least one span should carry the overridden color
    let has_red = lines.iter().any(|l| {
        l.spans
            .iter()
            .any(|s| s.style.fg == Some(Color::Rgb(255, 0, 0)))
    });
    assert!(has_red, "H1 should use the theme headings.h1 color");
}

#[test]
fn test_further_reading_renders_nine_link_lines() {
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
    let theme = Theme::default();
    let lines = render_elements(&elements, &theme);

    let visible: Vec<String> = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .filter(|s: &String| !s.trim().is_empty())
        .collect();

    let bullet_lines: Vec<&String> = visible.iter().filter(|s| s.contains('•')).collect();
    assert_eq!(
        bullet_lines.len(),
        9,
        "expected 9 bulleted item lines; got {}. Lines:\n{:#?}",
        bullet_lines.len(),
        visible
    );
}

#[test]
fn test_full_readme_further_reading_nine_bullets() {
    let md = include_str!("../TEST.md");
    let elements = parse(md);

    let fr_heading_pos = elements.iter().position(|e| {
        matches!(e, vellum::parser::Element::Heading { text, .. } if text.contains("Further reading"))
    }).expect("Further reading heading not found");

    let list_el = elements
        .get(fr_heading_pos + 1)
        .expect("No element after Further reading heading");

    assert!(
        matches!(list_el, vellum::parser::Element::List { .. }),
        "Expected List after heading, got: {list_el:?}"
    );

    let theme = Theme::default();
    let list_lines = render_elements(std::slice::from_ref(list_el), &theme);
    let bullet_lines: Vec<_> = list_lines
        .iter()
        .filter(|l| l.spans.iter().any(|s| s.content.contains('•')))
        .collect();

    assert_eq!(
        bullet_lines.len(),
        9,
        "Expected 9 bullet lines from TEST.md parse; got {}.\nAll lines: {:#?}",
        bullet_lines.len(),
        list_lines
            .iter()
            .map(|l| l
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>())
            .collect::<Vec<_>>()
    );
}
