use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A block-level document element.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Heading { level: u8, text: String },
    Paragraph(Vec<Span>),
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote(Vec<Element>),
    List { ordered: bool, items: Vec<Vec<Element>> },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Image { alt: String, src: String },
    Video { src: String },
    HRule,
    Break,
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

fn parse_events(events: Vec<Event>) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut i = 0;

    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = heading_level(*level);
                i += 1;
                let mut text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) => text.push_str(t),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(Element::Heading { level: lvl, text });
            }

            Event::Start(Tag::Paragraph) => {
                i += 1;
                let mut spans = Vec::new();
                let mut bold = false;
                let mut italic = false;
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
                            } else {
                                spans.push(Span::Text(s));
                            }
                        }
                        Event::Code(c) => spans.push(Span::Code(c.to_string())),
                        Event::Start(Tag::Strong) => bold = true,
                        Event::End(TagEnd::Strong) => bold = false,
                        Event::Start(Tag::Emphasis) => italic = true,
                        Event::End(TagEnd::Emphasis) => italic = false,
                        Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough) => {}
                        Event::Start(Tag::Link { dest_url, .. }) => {
                            link_href = Some(dest_url.to_string());
                            link_text.clear();
                        }
                        Event::End(TagEnd::Link) => {
                            if let Some(href) = link_href.take() {
                                spans.push(Span::Link { text: link_text.clone(), href });
                                link_text.clear();
                            }
                        }
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
                            spans.push(Span::Text(format!("[img: {}]", alt)));
                            elements.push(Element::Image { alt, src });
                        }
                        Event::SoftBreak | Event::HardBreak => spans.push(Span::Text(" ".into())),
                        Event::End(TagEnd::Paragraph) => break,
                        _ => {}
                    }
                    i += 1;
                }
                if !spans.is_empty() {
                    elements.push(Element::Paragraph(spans));
                }
            }

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

            Event::Rule => elements.push(Element::HRule),

            Event::Start(Tag::BlockQuote(_)) => {
                i += 1;
                let mut inner = Vec::new();
                let mut depth = 1usize;
                while i < events.len() {
                    match &events[i] {
                        Event::Start(Tag::BlockQuote(_)) => { depth += 1; inner.push(events[i].clone()); }
                        Event::End(TagEnd::BlockQuote(_)) => {
                            depth -= 1;
                            if depth == 0 { break; }
                            inner.push(events[i].clone());
                        }
                        _ => inner.push(events[i].clone()),
                    }
                    i += 1;
                }
                elements.push(Element::BlockQuote(parse_events(inner)));
            }

            Event::Start(Tag::List(first_num)) => {
                let ordered = first_num.is_some();
                i += 1;
                let mut items: Vec<Vec<Element>> = Vec::new();
                let mut depth = 1usize;
                let mut item_events: Vec<Event> = Vec::new();

                while i < events.len() {
                    match &events[i] {
                        Event::Start(Tag::List(_)) => { depth += 1; item_events.push(events[i].clone()); }
                        Event::End(TagEnd::List(_)) => {
                            depth -= 1;
                            if depth == 0 {
                                if !item_events.is_empty() {
                                    items.push(parse_events(item_events.drain(..).collect()));
                                }
                                break;
                            }
                            item_events.push(events[i].clone());
                        }
                        Event::Start(Tag::Item) => {
                            if depth == 1 && !item_events.is_empty() {
                                items.push(parse_events(item_events.drain(..).collect()));
                            } else if depth > 1 {
                                item_events.push(events[i].clone());
                            }
                        }
                        Event::End(TagEnd::Item) => {
                            if depth > 1 { item_events.push(events[i].clone()); }
                        }
                        _ => item_events.push(events[i].clone()),
                    }
                    i += 1;
                }
                elements.push(Element::List { ordered, items });
            }

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
                        Event::End(TagEnd::TableRow) => {
                            if !in_header { rows.push(current_row.clone()); }
                        }
                        Event::Start(Tag::TableCell) => cell_text.clear(),
                        Event::End(TagEnd::TableCell) => {
                            if in_header { headers.push(cell_text.clone()); }
                            else { current_row.push(cell_text.clone()); }
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
