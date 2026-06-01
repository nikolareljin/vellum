use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A block-level document element.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(Vec<Span>),
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    BlockQuote(Vec<Element>),
    List {
        ordered: bool,
        items: Vec<Vec<Element>>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Image {
        alt: String,
        src: String,
    },
    Video {
        src: String,
    },
    HRule,
}

/// An inline span within a paragraph or list item.
#[derive(Debug, Clone, PartialEq)]
pub enum Span {
    Text(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Link { text: String, href: String },
    Strikethrough(String),
}

/// Parse a Markdown string into a list of block elements.
pub fn parse(input: &str) -> Vec<Element> {
    let opts = Options::all();
    let parser = Parser::new_ext(input, opts);
    parse_events(parser.collect())
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Extract the value of an HTML attribute from a raw HTML snippet.
///
/// - Attribute name comparison is case-insensitive (`to_ascii_lowercase`,
///   1-to-1 byte mapping so byte offsets stay valid).
/// - Accepts optional ASCII whitespace (space, tab, `\n`, `\r`) around `=`.
/// - Word-boundary check: the byte before the attribute name must be
///   whitespace or the start of the string, preventing `data-src` from
///   matching when searching for `src`.
fn html_attr(html: &str, attr: &str) -> Option<String> {
    let html_lc = html.to_ascii_lowercase();
    let attr_lc = attr.to_ascii_lowercase();
    let bytes = html_lc.as_bytes();

    let mut i = 0;
    while i + attr_lc.len() <= bytes.len() {
        let rel = html_lc[i..].find(&attr_lc)?;
        let pos = i + rel;

        let boundary = pos == 0 || matches!(bytes[pos - 1], b' ' | b'\t' | b'\n' | b'\r');

        if boundary {
            let mut j = pos + attr_lc.len();
            // Skip optional whitespace before '='
            while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                // Skip optional whitespace after '='
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
                    j += 1;
                }
                if j < bytes.len() && matches!(bytes[j], b'"' | b'\'') {
                    let quote = bytes[j] as char;
                    // Use original html (not html_lc) to preserve value casing
                    let val_rest = &html[j + 1..];
                    if let Some(end) = val_rest.find(quote) {
                        return Some(val_rest[..end].to_owned());
                    }
                }
            }
        }

        i = pos + 1;
    }
    None
}

/// If `html` contains an `<img>` tag, return `(alt, src)`.
/// Attribute extraction is scoped to just the `<img … >` tag so that other
/// tags earlier in the snippet (e.g. `<div src="…">`) cannot shadow the
/// image's real `src`.
///
/// Two correctness invariants:
/// - The byte after `<img` must be ASCII whitespace, `/`, or `>` so that
///   `<imgur>` and similar names are not mistaken for `<img>`.
/// - The closing `>` is found by scanning while skipping over quoted attribute
///   values, so `alt="a > b"` does not terminate the tag prematurely.
fn extract_img_tag(html: &str) -> Option<(String, String)> {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();

    // Find an `<img` whose next byte is a valid tag-name terminator.
    let img_start = {
        let mut search = 0;
        loop {
            let rel = lower[search..].find("<img")?;
            let pos = search + rel;
            match bytes.get(pos + 4).copied() {
                None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/') => break pos,
                _ => search = pos + 1,
            }
        }
    };

    // Scan for the closing `>`, skipping over quoted attribute values so that
    // `alt="a > b"` does not truncate the tag at the inner `>`.
    let tag_end = {
        let mut i = img_start + 4; // start after "<img"
        let mut in_quote: Option<u8> = None;
        loop {
            match (bytes.get(i).copied(), in_quote) {
                (None, _) => break html.len(),
                (Some(b'>'), None) => break i + 1,
                (Some(q @ (b'"' | b'\'')), None) => in_quote = Some(q),
                (Some(q), Some(iq)) if q == iq => in_quote = None,
                _ => {}
            }
            i += 1;
        }
    };

    let img_tag = &html[img_start..tag_end];
    let src = html_attr(img_tag, "src")?;
    let alt = html_attr(img_tag, "alt").unwrap_or_default();
    Some((alt, src))
}

/// Returns `true` when `src` looks like a video file by extension.
fn is_video_src(src: &str) -> bool {
    let lower = src.to_lowercase();
    matches!(
        lower.rsplit('.').next().unwrap_or(""),
        "mp4" | "webm" | "mov" | "avi" | "mkv"
    )
}

/// Tight list items (no blank line between siblings) are emitted by
/// pulldown-cmark WITHOUT Start(Paragraph)/End(Paragraph) wrappers — bare
/// inline events go directly under Start(Item).  `parse_events` only handles
/// block-level openers at the top level, so those bare events fall through to
/// the no-op `_ => {}` arm and produce an empty element list.
///
/// This helper detects the "bare inline" case and wraps the events in a
/// synthetic Paragraph, making them parseable by `parse_events`.
fn wrap_tight_item(events: Vec<Event>) -> Vec<Event> {
    let is_bare = match events.first() {
        Some(
            Event::Start(Tag::Paragraph)
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::List(_))
            | Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::Table(_))
            | Event::Rule,
        )
        | None => false,
        Some(_) => true,
    };
    if is_bare {
        let mut wrapped = vec![Event::Start(Tag::Paragraph)];
        wrapped.extend(events);
        wrapped.push(Event::End(TagEnd::Paragraph));
        wrapped
    } else {
        events
    }
}

fn parse_events(events: Vec<Event>) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut i = 0;

    while i < events.len() {
        match &events[i] {
            // ── Headings ──────────────────────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = heading_level(*level);
                i += 1;
                let mut text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) | Event::Code(t) => text.push_str(t),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(Element::Heading { level: lvl, text });
            }

            // ── Paragraphs (inline spans) ─────────────────────────────────────
            Event::Start(Tag::Paragraph) => {
                i += 1;
                let mut spans = Vec::new();
                let mut bold = false;
                let mut italic = false;
                let mut strikethrough = false;
                let mut link_href: Option<String> = None;
                let mut link_text = String::new();

                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) => {
                            let s = t.to_string();
                            if link_href.is_some() {
                                link_text.push_str(&s);
                            } else if bold && italic {
                                spans.push(Span::BoldItalic(s));
                            } else if bold {
                                spans.push(Span::Bold(s));
                            } else if italic {
                                spans.push(Span::Italic(s));
                            } else if strikethrough {
                                spans.push(Span::Strikethrough(s));
                            } else {
                                spans.push(Span::Text(s));
                            }
                        }
                        Event::Code(c) => {
                            if link_href.is_some() {
                                // e.g. [`PLAN.md`](./PLAN.md) — the backtick text
                                // is the link label, not a standalone code span
                                link_text.push_str(c);
                            } else {
                                spans.push(Span::Code(c.to_string()));
                            }
                        }
                        Event::Start(Tag::Strong) => bold = true,
                        Event::End(TagEnd::Strong) => bold = false,
                        Event::Start(Tag::Emphasis) => italic = true,
                        Event::End(TagEnd::Emphasis) => italic = false,
                        Event::Start(Tag::Strikethrough) => strikethrough = true,
                        Event::End(TagEnd::Strikethrough) => strikethrough = false,
                        Event::Start(Tag::Link { dest_url, .. }) => {
                            link_href = Some(dest_url.to_string());
                            link_text.clear();
                        }
                        Event::End(TagEnd::Link) => {
                            if let Some(href) = link_href.take() {
                                spans.push(Span::Link {
                                    text: link_text.clone(),
                                    href,
                                });
                                link_text.clear();
                            }
                        }
                        // Image (or video) inline — emit a block element and
                        // skip adding any text span to the paragraph
                        Event::Start(Tag::Image { dest_url, .. }) => {
                            let src = dest_url.to_string();
                            i += 1;
                            let mut alt = String::new();
                            while i < events.len() {
                                match &events[i] {
                                    Event::Text(t) => alt.push_str(t),
                                    Event::End(TagEnd::Image) => break,
                                    _ => {}
                                }
                                i += 1;
                            }
                            // Flush any preceding text spans so surrounding text
                            // (e.g. "hello ![x](u) world") appears in document
                            // order: Paragraph("hello ") → Image → Paragraph(" world").
                            if !spans.is_empty() {
                                elements.push(Element::Paragraph(std::mem::take(&mut spans)));
                            }
                            if is_video_src(&src) {
                                elements.push(Element::Video { src });
                            } else {
                                elements.push(Element::Image { alt, src });
                            }
                        }
                        // Inline HTML — pick out <img> tags.
                        // Flush accumulated spans first so that surrounding text
                        // appears before the image in document order.
                        Event::InlineHtml(html) => {
                            if let Some((alt, src)) = extract_img_tag(html) {
                                if !spans.is_empty() {
                                    elements.push(Element::Paragraph(
                                        std::mem::take(&mut spans),
                                    ));
                                }
                                if is_video_src(&src) {
                                    elements.push(Element::Video { src });
                                } else {
                                    elements.push(Element::Image { alt, src });
                                }
                            }
                        }
                        Event::SoftBreak => spans.push(Span::Text(" ".into())),
                        Event::HardBreak
                            // Flush current spans as a paragraph then start fresh
                            if !spans.is_empty() => {
                                elements.push(Element::Paragraph(std::mem::take(&mut spans)));
                            }
                        Event::End(TagEnd::Paragraph) => break,
                        _ => {}
                    }
                    i += 1;
                }
                if !spans.is_empty() {
                    elements.push(Element::Paragraph(spans));
                }
            }

            // ── Code blocks ──────────────────────────────────────────────────
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) if !info.is_empty() => Some(info.to_string()),
                    _ => None,
                };
                i += 1;
                let mut code = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) => code.push_str(t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(Element::CodeBlock { lang, code });
            }

            // ── Raw HTML blocks — pick out <img> tags ─────────────────────────
            Event::Html(html) => {
                if let Some((alt, src)) = extract_img_tag(html) {
                    if is_video_src(&src) {
                        elements.push(Element::Video { src });
                    } else {
                        elements.push(Element::Image { alt, src });
                    }
                }
            }

            // ── Horizontal rule ───────────────────────────────────────────────
            Event::Rule => elements.push(Element::HRule),

            // ── Block quotes ──────────────────────────────────────────────────
            Event::Start(Tag::BlockQuote(_)) => {
                i += 1;
                let mut inner = Vec::new();
                let mut depth = 1usize;
                while i < events.len() {
                    match &events[i] {
                        Event::Start(Tag::BlockQuote(_)) => {
                            depth += 1;
                            inner.push(events[i].clone());
                        }
                        Event::End(TagEnd::BlockQuote(_)) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            inner.push(events[i].clone());
                        }
                        _ => inner.push(events[i].clone()),
                    }
                    i += 1;
                }
                elements.push(Element::BlockQuote(parse_events(inner)));
            }

            // ── Lists ─────────────────────────────────────────────────────────
            Event::Start(Tag::List(first_num)) => {
                let ordered = first_num.is_some();
                i += 1;
                let mut items: Vec<Vec<Element>> = Vec::new();
                let mut depth = 1usize;
                let mut item_events: Vec<Event> = Vec::new();

                while i < events.len() {
                    match &events[i] {
                        Event::Start(Tag::List(_)) => {
                            depth += 1;
                            item_events.push(events[i].clone());
                        }
                        Event::End(TagEnd::List(_)) => {
                            depth -= 1;
                            if depth == 0 {
                                if !item_events.is_empty() {
                                    items.push(parse_events(wrap_tight_item(std::mem::take(
                                        &mut item_events,
                                    ))));
                                }
                                break;
                            }
                            item_events.push(events[i].clone());
                        }
                        Event::Start(Tag::Item) => {
                            if depth == 1 && !item_events.is_empty() {
                                items.push(parse_events(wrap_tight_item(std::mem::take(
                                    &mut item_events,
                                ))));
                            } else if depth > 1 {
                                item_events.push(events[i].clone());
                            }
                        }
                        Event::End(TagEnd::Item) => {
                            if depth > 1 {
                                item_events.push(events[i].clone());
                            }
                        }
                        _ => item_events.push(events[i].clone()),
                    }
                    i += 1;
                }
                elements.push(Element::List { ordered, items });
            }

            // ── Tables ────────────────────────────────────────────────────────
            Event::Start(Tag::Table(_)) => {
                i += 1;
                let mut headers: Vec<String> = Vec::new();
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut in_header = true;
                let mut current_row: Vec<String> = Vec::new();
                let mut cell_text = String::new();

                while i < events.len() {
                    match &events[i] {
                        Event::Start(Tag::TableHead) => in_header = true,
                        Event::End(TagEnd::TableHead) => in_header = false,
                        Event::Start(Tag::TableRow) => current_row.clear(),
                        Event::End(TagEnd::TableRow) if !in_header => {
                            rows.push(current_row.clone());
                        }
                        Event::Start(Tag::TableCell) => cell_text.clear(),
                        Event::End(TagEnd::TableCell) => {
                            if in_header {
                                headers.push(cell_text.clone());
                            } else {
                                current_row.push(cell_text.clone());
                            }
                        }
                        Event::Text(t) => cell_text.push_str(t),
                        Event::End(TagEnd::Table) => break,
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(Element::Table { headers, rows });
            }

            _ => {}
        }
        i += 1;
    }

    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── html_attr ─────────────────────────────────────────────────────────────

    #[test]
    fn html_attr_double_quoted() {
        let html = r#"<img src="https://example.com/x.png" alt="Demo" />"#;
        assert_eq!(
            html_attr(html, "src").as_deref(),
            Some("https://example.com/x.png")
        );
        assert_eq!(html_attr(html, "alt").as_deref(), Some("Demo"));
    }

    #[test]
    fn html_attr_single_quoted() {
        let html = "<img src='https://example.com/x.png' alt='Demo' />";
        assert_eq!(
            html_attr(html, "src").as_deref(),
            Some("https://example.com/x.png")
        );
    }

    #[test]
    fn html_attr_case_insensitive_name() {
        let html = r#"<IMG SRC="https://example.com/x.png" ALT="Demo" />"#;
        assert_eq!(
            html_attr(html, "src").as_deref(),
            Some("https://example.com/x.png")
        );
        assert_eq!(html_attr(html, "alt").as_deref(), Some("Demo"));
    }

    #[test]
    fn html_attr_missing_returns_none() {
        let html = r#"<img alt="Demo" />"#;
        assert!(html_attr(html, "src").is_none());
    }

    #[test]
    fn html_attr_no_false_positive_on_data_src() {
        // data-src should NOT match when searching for "src"
        let html = r#"<img data-src="https://cdn.example.com/x.png" />"#;
        assert!(html_attr(html, "src").is_none());
    }

    #[test]
    fn html_attr_whitespace_around_equals() {
        // HTML allows spaces around '=': src = "..."
        let html = r#"<img src = "https://example.com/x.png" />"#;
        assert_eq!(
            html_attr(html, "src").as_deref(),
            Some("https://example.com/x.png")
        );
        let html2 = r#"<img src ="https://example.com/x.png" />"#;
        assert_eq!(
            html_attr(html2, "src").as_deref(),
            Some("https://example.com/x.png")
        );
    }

    #[test]
    fn html_attr_data_src_itself_is_extractable() {
        let html = r#"<img data-src="https://cdn.example.com/x.png" />"#;
        assert_eq!(
            html_attr(html, "data-src").as_deref(),
            Some("https://cdn.example.com/x.png")
        );
    }

    // ── extract_img_tag ───────────────────────────────────────────────────────

    #[test]
    fn extract_img_tag_full() {
        let html = r#"<img width="1080" height="2220" alt="Screenshot" src="https://github.com/user-attachments/assets/abc" />"#;
        assert_eq!(
            extract_img_tag(html),
            Some((
                "Screenshot".into(),
                "https://github.com/user-attachments/assets/abc".into()
            ))
        );
    }

    #[test]
    fn extract_img_tag_no_alt() {
        let html = r#"<img src="https://example.com/x.png" />"#;
        assert_eq!(
            extract_img_tag(html),
            Some(("".into(), "https://example.com/x.png".into()))
        );
    }

    #[test]
    fn extract_img_tag_non_img_html_returns_none() {
        assert!(extract_img_tag("<div>hello</div>").is_none());
        assert!(extract_img_tag("<!-- comment -->").is_none());
    }

    #[test]
    fn extract_img_tag_no_false_positive_on_imgur() {
        // <imgur> must not match the <img> detector
        assert!(extract_img_tag(r#"<imgur src="https://i.imgur.com/x.png">"#).is_none());
    }

    #[test]
    fn extract_img_tag_gt_in_quoted_attribute() {
        // '>' inside a quoted value must not truncate the tag prematurely
        let html = r#"<img alt="a > b" src="https://example.com/x.png" />"#;
        assert_eq!(
            extract_img_tag(html),
            Some(("a > b".into(), "https://example.com/x.png".into()))
        );
    }

    #[test]
    fn extract_img_tag_no_src_returns_none() {
        assert!(extract_img_tag(r#"<img alt="no-src" />"#).is_none());
    }

    #[test]
    fn extract_img_tag_scoped_to_img_not_surrounding_tags() {
        // A leading tag with src= must not shadow the img's real src
        let html = r#"<div src="https://wrong.example.com/x.png"><img src="https://right.example.com/x.png" /></div>"#;
        assert_eq!(
            extract_img_tag(html),
            Some(("".into(), "https://right.example.com/x.png".into()))
        );
    }

    #[test]
    fn html_attr_newline_whitespace_around_equals() {
        let html = "<img src\n=\n\"https://example.com/x.png\" />";
        assert_eq!(
            html_attr(html, "src").as_deref(),
            Some("https://example.com/x.png")
        );
    }

    // ── parse: block-level <img> ──────────────────────────────────────────────

    #[test]
    fn parse_block_html_img_becomes_image_element() {
        let md = "<img src=\"https://example.com/x.png\" alt=\"Demo\" />\n";
        let elements = parse(md);
        assert_eq!(
            elements,
            vec![Element::Image {
                alt: "Demo".into(),
                src: "https://example.com/x.png".into(),
            }]
        );
    }

    #[test]
    fn parse_block_html_img_no_alt() {
        let md = "<img src=\"https://example.com/x.png\" />\n";
        let elements = parse(md);
        assert_eq!(
            elements,
            vec![Element::Image {
                alt: "".into(),
                src: "https://example.com/x.png".into(),
            }]
        );
    }

    #[test]
    fn parse_block_html_non_img_is_ignored() {
        let elements = parse("<div>hello</div>\n");
        assert!(elements.is_empty());
    }

    #[test]
    fn parse_markdown_img_still_works() {
        let md = "![Alt text](https://example.com/x.png)\n";
        let elements = parse(md);
        assert_eq!(
            elements,
            vec![Element::Image {
                alt: "Alt text".into(),
                src: "https://example.com/x.png".into(),
            }]
        );
    }

    // ── parse: image ordering (both Markdown and HTML) ───────────────────────

    #[test]
    fn parse_markdown_inline_img_flushes_preceding_text() {
        // "hello ![x](url) world" must yield
        // [Paragraph("hello "), Image, Paragraph(" world")]
        // not [Image, Paragraph("hello  world")]
        let md = "hello ![x](https://example.com/x.png) world\n";
        let elements = parse(md);
        assert_eq!(elements.len(), 3);
        assert_eq!(
            elements[0],
            Element::Paragraph(vec![Span::Text("hello ".into())])
        );
        assert_eq!(
            elements[1],
            Element::Image {
                alt: "x".into(),
                src: "https://example.com/x.png".into(),
            }
        );
        assert_eq!(
            elements[2],
            Element::Paragraph(vec![Span::Text(" world".into())])
        );
    }

    #[test]
    fn parse_inline_img_flushes_preceding_text() {
        // "hello <img /> world" must yield [Paragraph("hello "), Image, Paragraph(" world")]
        // not [Image, Paragraph("hello  world")]
        let md = "hello <img src=\"https://example.com/x.png\" alt=\"x\" /> world\n";
        let elements = parse(md);
        assert_eq!(elements.len(), 3);
        assert_eq!(
            elements[0],
            Element::Paragraph(vec![Span::Text("hello ".into())])
        );
        assert_eq!(
            elements[1],
            Element::Image {
                alt: "x".into(),
                src: "https://example.com/x.png".into(),
            }
        );
        assert_eq!(
            elements[2],
            Element::Paragraph(vec![Span::Text(" world".into())])
        );
    }
}
