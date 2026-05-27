# Vellum Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `vellum` — a rich Markdown viewer that renders text, images, code blocks, and clickable links directly in the terminal using a full TUI, with an optional code-view mode that opens the raw file in `$EDITOR`.

**Architecture:** Parse a Markdown file with `pulldown-cmark` into an internal `Element` enum tree. A renderer maps that tree to `ratatui` widgets drawn each frame. Images are rendered inline via `ratatui-image` (auto-detecting Kitty / Sixel / iTerm2 protocols). Links use OSC 8 escape sequences for external URLs and an anchor map for `#heading` jumps. Video embeds are extracted to a single thumbnail frame via `ffmpeg`.

**Tech Stack:** Rust 2021 · ratatui 0.29 · crossterm 0.28 · pulldown-cmark 0.12 · syntect 5 · ratatui-image 6 · image 0.25 · clap 4 · anyhow 1 · thiserror 2

---

## File Map

| File | Responsibility |
|------|----------------|
| `src/main.rs` | CLI entry point — parse args, dispatch TUI, code-view, or about page |
| `src/app.rs` | `App` state struct + ratatui event loop + keyboard handling |
| `src/parser.rs` | `pulldown-cmark` events → `Vec<Element>` |
| `src/renderer.rs` | `Vec<Element>` → ratatui `Text` / `Paragraph` / `Table` widgets |
| `src/highlight.rs` | `syntect` wrapper — syntax-highlight a code block string |
| `src/image.rs` | `ratatui-image` integration — load & cache inline images *(Phase 2)* |
| `src/video.rs` | `ffmpeg` subprocess → extract first frame as temp PNG *(Phase 4)* |
| `src/links.rs` | OSC 8 external links + heading anchor offset map *(Phase 3)* |
| `src/search.rs` | In-document text search with match highlighting *(Phase 6)* |
| `tests/parser_tests.rs` | Unit tests for `parser.rs` |
| `tests/renderer_tests.rs` | Unit tests for `renderer.rs` |
| `tests/links_tests.rs` | Unit tests for `links.rs` |

---

## Shared Types  *(defined in `src/parser.rs`, used everywhere)*

```rust
/// A block-level document element produced by the parser.
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

/// An inline span inside a paragraph or list item.
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
```

---

## Phase 1 — Core Viewer

**Delivers:** Parse Markdown → render styled text, headings, code blocks with syntax highlighting, horizontal rules, block quotes, lists, tables. Vertical scrolling. Quit with `q`. Code-view mode with `e`.

### Task 1: Project Bootstrap

**Files:**
- Modify: `src/main.rs`
- Create: `src/app.rs`

- [ ] **Step 1: Replace generated `src/main.rs` with CLI skeleton**

```rust
// src/main.rs
use clap::Parser;

mod app;
mod highlight;
mod parser;
mod renderer;

#[derive(Parser, Debug)]
#[command(name = "vellum", about = "Rich Markdown viewer for the terminal")]
pub struct Cli {
    /// Markdown file to open
    pub file: std::path::PathBuf,

    /// Open in code view (spawns $EDITOR / bat / less)
    #[arg(short, long)]
    pub code: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.code {
        app::open_code_view(&cli.file)?;
    } else {
        app::run(&cli.file)?;
    }

    Ok(())
}
```

- [ ] **Step 2: Create `src/app.rs` stub**

```rust
// src/app.rs
use std::path::Path;

pub fn run(_file: &Path) -> anyhow::Result<()> {
    todo!("Phase 1 TUI loop")
}

pub fn open_code_view(file: &Path) -> anyhow::Result<()> {
    todo!("Phase 5 code view")
}
```

- [ ] **Step 3: Verify it compiles (errors about `todo!` are expected)**

```bash
cargo build 2>&1 | grep -v "warning"
```

Expected: `error[E0433]` only for missing modules — fix by creating empty stubs:

```bash
# Create module stubs so the build proceeds
touch src/highlight.rs src/parser.rs src/renderer.rs
# Add empty module markers to each file
echo "// stub" | tee src/highlight.rs src/parser.rs src/renderer.rs
```

- [ ] **Step 4: Verify build succeeds with stubs**

```bash
cargo build 2>&1
```

Expected: `Compiling vellum` → no errors (todo! panics are runtime, not compile-time).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: initial project scaffold"
```

---

### Task 2: Parser — Markdown → `Vec<Element>`

**Files:**
- Create: `src/parser.rs`
- Create: `tests/parser_tests.rs`

- [ ] **Step 1: Define `Element` and `Span` types, add failing test**

Create `src/parser.rs`:

```rust
// src/parser.rs
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
                            if let Some(_) = &link_href {
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
                        Event::Start(Tag::Strikethrough) => {}
                        Event::End(TagEnd::Strikethrough) => {}
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
                        Event::Start(Tag::Image { dest_url, title, .. }) => {
                            let src = dest_url.to_string();
                            // Collect alt text
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
                            // Image inside paragraph: push as inline placeholder
                            spans.push(Span::Text(format!("[img: {}]", alt)));
                            // Also record as block for Phase 2
                            elements.push(Element::Image { alt, src });
                        }
                        Event::SoftBreak | Event::HardBreak => spans.push(Span::Text(" ".to_string())),
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
                use pulldown_cmark::CodeBlockKind;
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
                // Collect inner events until matching end
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

            _ => {}
        }
        i += 1;
    }

    elements
}
```

- [ ] **Step 2: Create `tests/parser_tests.rs` with failing tests**

```rust
// tests/parser_tests.rs
use vellum::parser::{parse, Element, Span};

#[test]
fn test_heading_level_1() {
    let elements = parse("# Hello World");
    assert_eq!(
        elements,
        vec![Element::Heading { level: 1, text: "Hello World".into() }]
    );
}

#[test]
fn test_heading_level_3() {
    let elements = parse("### Deep heading");
    assert_eq!(
        elements,
        vec![Element::Heading { level: 3, text: "Deep heading".into() }]
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
        vec![Element::CodeBlock { lang: None, code: "plain code\n".into() }]
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
    let Some(Element::Paragraph(spans)) = elements.first() else { panic!("no paragraph") };
    assert!(spans.iter().any(|s| matches!(s, Span::Link { href, .. } if href == "https://example.com")));
}
```

- [ ] **Step 3: Expose `parser` module from `lib.rs` so tests can import it**

```bash
# Add lib.rs so integration tests can reference vellum::parser
cat > src/lib.rs << 'RUST'
pub mod highlight;
pub mod parser;
pub mod renderer;
RUST
```

- [ ] **Step 4: Run tests — expect failures about missing items**

```bash
cargo test 2>&1 | head -40
```

Expected: compile errors about `mod app` not being in lib.rs — that's fine, app is binary-only. Adjust `src/main.rs` mods accordingly (app stays in main.rs only, not lib.rs).

- [ ] **Step 5: Run tests until all pass**

```bash
cargo test --test parser_tests -- --nocapture
```

Expected: all 9 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/parser.rs src/lib.rs tests/parser_tests.rs
git commit -m "feat: parser — pulldown-cmark to Element tree"
```

---

### Task 3: Syntax Highlighter

**Files:**
- Create: `src/highlight.rs`

- [ ] **Step 1: Write failing test**

Add to `tests/renderer_tests.rs`:

```rust
// tests/renderer_tests.rs
use vellum::highlight::highlight_code;

#[test]
fn test_highlight_returns_lines() {
    let lines = highlight_code("fn main() {}", Some("rust"));
    assert!(!lines.is_empty(), "should return at least one styled line");
    // Each line is a Vec of (style, text) tuples
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
```

- [ ] **Step 2: Implement `src/highlight.rs`**

```rust
// src/highlight.rs
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// One line = list of (syntect Style, text slice) pairs.
pub type StyledLine = Vec<(Style, String)>;

thread_local! {
    static SS: SyntaxSet = SyntaxSet::load_defaults_nonewlines();
    static TS: ThemeSet = ThemeSet::load_defaults();
}

/// Highlight `code` with the given language hint.
/// Returns one `StyledLine` per source line.
/// Falls back to plain text if the language is unknown.
pub fn highlight_code(code: &str, lang: Option<&str>) -> Vec<StyledLine> {
    SS.with(|ss| {
        TS.with(|ts| {
            let syntax = lang
                .and_then(|l| ss.find_syntax_by_token(l))
                .unwrap_or_else(|| ss.find_syntax_plain_text());
            let theme = &ts.themes["base16-ocean.dark"];
            let mut hl = HighlightLines::new(syntax, theme);
            let mut out = Vec::new();
            for line in LinesWithEndings::from(code) {
                let ranges = hl.highlight_line(line, ss).unwrap_or_default();
                let styled: StyledLine = ranges
                    .into_iter()
                    .map(|(style, text)| (style, text.to_owned()))
                    .collect();
                out.push(styled);
            }
            out
        })
    })
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --test renderer_tests -- --nocapture
```

Expected: all 3 highlight tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/highlight.rs tests/renderer_tests.rs
git commit -m "feat: syntect syntax highlighting"
```

---

### Task 4: Renderer — Elements → Ratatui Widgets

**Files:**
- Create: `src/renderer.rs`

- [ ] **Step 1: Add renderer tests to `tests/renderer_tests.rs`**

Append to the file:

```rust
use vellum::renderer::render_elements;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};
use vellum::parser::{Element, Span};

#[test]
fn test_heading_h1_is_bold_yellow() {
    let el = Element::Heading { level: 1, text: "Title".into() };
    let lines = render_elements(&[el]);
    assert!(!lines.is_empty());
    let first_line = &lines[0];
    // h1 must include bold modifier somewhere
    assert!(
        first_line.spans.iter().any(|s| s.style.add_modifier.contains(Modifier::BOLD)),
        "h1 should be bold"
    );
}

#[test]
fn test_hrule_is_dashes() {
    let el = Element::HRule;
    let lines = render_elements(&[el]);
    assert!(!lines.is_empty());
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains('─') || text.contains('-'), "hrule should be a line");
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
```

- [ ] **Step 2: Implement `src/renderer.rs`**

```rust
// src/renderer.rs
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};

use crate::highlight::highlight_code;
use crate::parser::{Element, Span};

const HEADING_COLORS: [Color; 6] = [
    Color::Yellow,
    Color::Cyan,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::White,
];

/// Convert a list of `Element`s into a flat list of ratatui `Line`s ready for display.
pub fn render_elements(elements: &[Element]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for element in elements {
        render_element(element, &mut lines, 0);
        lines.push(Line::from(""));   // blank line between blocks
    }
    lines
}

fn render_element(element: &Element, out: &mut Vec<Line<'static>>, indent: usize) {
    match element {
        Element::Heading { level, text } => {
            let prefix = "#".repeat(*level as usize) + " ";
            let color = HEADING_COLORS[(*level as usize).saturating_sub(1).min(5)];
            let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            out.push(Line::from(vec![
                RSpan::styled(prefix, style),
                RSpan::styled(text.clone(), style),
            ]));
        }

        Element::Paragraph(spans) => {
            let prefix = " ".repeat(indent);
            let mut rspans: Vec<RSpan<'static>> = Vec::new();
            if !prefix.is_empty() {
                rspans.push(RSpan::raw(prefix));
            }
            for span in spans {
                rspans.extend(render_span(span));
            }
            out.push(Line::from(rspans));
        }

        Element::CodeBlock { lang, code } => {
            let lang_str = lang.as_deref().unwrap_or("");
            let header = format!(" {} ", if lang_str.is_empty() { "code" } else { lang_str });
            out.push(Line::from(vec![RSpan::styled(
                header,
                Style::default().fg(Color::Black).bg(Color::DarkGray).add_modifier(Modifier::BOLD),
            )]));
            let styled_lines = highlight_code(code, lang.as_deref());
            for sl in styled_lines {
                let rspans: Vec<RSpan<'static>> = sl
                    .into_iter()
                    .map(|(style, text)| {
                        let fg = syntect_color_to_ratatui(style.foreground);
                        RSpan::styled(text, Style::default().fg(fg))
                    })
                    .collect();
                out.push(Line::from(rspans));
            }
            out.push(Line::from(vec![RSpan::styled(
                "─".repeat(40),
                Style::default().fg(Color::DarkGray),
            )]));
        }

        Element::HRule => {
            out.push(Line::from(vec![RSpan::styled(
                "─".repeat(60),
                Style::default().fg(Color::DarkGray),
            )]));
        }

        Element::BlockQuote(inner) => {
            let bar = RSpan::styled("│ ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
            let mut inner_lines: Vec<Line<'static>> = Vec::new();
            for el in inner {
                render_element(el, &mut inner_lines, indent + 2);
            }
            for mut line in inner_lines {
                line.spans.insert(0, bar.clone());
                out.push(line);
            }
        }

        Element::List { ordered, items } => {
            for (idx, item_elements) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", idx + 1)
                } else {
                    "• ".to_string()
                };
                let bullet_span = RSpan::styled(
                    " ".repeat(indent) + &bullet,
                    Style::default().fg(Color::Cyan),
                );
                let mut item_lines: Vec<Line<'static>> = Vec::new();
                for el in item_elements {
                    render_element(el, &mut item_lines, indent + 2);
                }
                if let Some(first) = item_lines.first_mut() {
                    first.spans.insert(0, bullet_span);
                }
                out.extend(item_lines);
            }
        }

        Element::Table { headers, rows } => {
            let col_widths: Vec<usize> = headers.iter().enumerate().map(|(i, h)| {
                let max_row = rows.iter().map(|r| r.get(i).map(|c| c.len()).unwrap_or(0)).max().unwrap_or(0);
                h.len().max(max_row).max(3)
            }).collect();

            // Header row
            let header_spans: Vec<RSpan<'static>> = headers.iter().enumerate().map(|(i, h)| {
                RSpan::styled(
                    format!(" {:<width$} │", h, width = col_widths[i]),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )
            }).collect();
            out.push(Line::from(header_spans));

            // Separator
            let sep: String = col_widths.iter().map(|w| "─".repeat(w + 2) + "┼").collect();
            out.push(Line::from(vec![RSpan::styled(sep, Style::default().fg(Color::DarkGray))]));

            // Data rows
            for row in rows {
                let row_spans: Vec<RSpan<'static>> = col_widths.iter().enumerate().map(|(i, w)| {
                    let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    RSpan::raw(format!(" {:<width$} │", cell, width = w))
                }).collect();
                out.push(Line::from(row_spans));
            }
        }

        Element::Image { alt, src: _ } => {
            // Phase 1: placeholder; Phase 2 replaces with actual image widget
            out.push(Line::from(vec![
                RSpan::styled("[🖼  ", Style::default().fg(Color::Magenta)),
                RSpan::raw(alt.clone()),
                RSpan::styled("]", Style::default().fg(Color::Magenta)),
            ]));
        }

        Element::Video { src: _ } => {
            // Phase 4: replaced with thumbnail
            out.push(Line::from(vec![
                RSpan::styled("[▶  video thumbnail]", Style::default().fg(Color::Blue)),
            ]));
        }

        Element::Break => out.push(Line::from("")),
    }
}

fn render_span(span: &Span) -> Vec<RSpan<'static>> {
    match span {
        Span::Text(t) => vec![RSpan::raw(t.clone())],
        Span::Bold(t) => vec![RSpan::styled(t.clone(), Style::default().add_modifier(Modifier::BOLD))],
        Span::Italic(t) => vec![RSpan::styled(t.clone(), Style::default().add_modifier(Modifier::ITALIC))],
        Span::BoldItalic(t) => vec![RSpan::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
        )],
        Span::Code(t) => vec![RSpan::styled(
            format!("`{}`", t),
            Style::default().fg(Color::Green).bg(Color::Black),
        )],
        Span::Link { text, href } => vec![
            RSpan::styled(text.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED)),
            RSpan::styled(
                format!(" ({})", href),
                Style::default().fg(Color::DarkGray),
            ),
        ],
        Span::Strikethrough(t) => vec![RSpan::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::CROSSED_OUT),
        )],
    }
}

fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}
```

- [ ] **Step 3: Run renderer tests**

```bash
cargo test --test renderer_tests -- --nocapture
```

Expected: all 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/renderer.rs
git commit -m "feat: renderer — Element tree to ratatui lines"
```

---

### Task 5: TUI App Loop — Scrolling Viewer

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Implement the full `run` function in `src/app.rs`**

```rust
// src/app.rs
use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use ratatui::Terminal;

use crate::parser;
use crate::renderer::render_elements;

struct App {
    lines: Vec<Line<'static>>,
    scroll: usize,
    viewport_height: usize,
}

impl App {
    fn new(lines: Vec<Line<'static>>) -> Self {
        App { lines, scroll: 0, viewport_height: 0 }
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.lines.len().saturating_sub(self.viewport_height);
        self.scroll = (self.scroll + n).min(max);
    }

    fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    fn page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(2));
    }

    fn page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(2));
    }

    fn goto_top(&mut self) { self.scroll = 0; }

    fn goto_bottom(&mut self) {
        self.scroll = self.lines.len().saturating_sub(self.viewport_height);
    }
}

pub fn run(file: &Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(file)?;
    let elements = parser::parse(&source);
    let lines = render_elements(&elements);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(lines);
    let fname = file.file_name().unwrap_or_default().to_string_lossy().to_string();

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            app.viewport_height = chunks[0].height as usize;

            let visible: Vec<Line<'static>> = app
                .lines
                .iter()
                .skip(app.scroll)
                .take(app.viewport_height)
                .cloned()
                .collect();

            let content = Paragraph::new(Text::from(visible))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false });
            f.render_widget(content, chunks[0]);

            // Status bar
            let status = Line::from(vec![
                ratatui::text::Span::styled(
                    format!(" {} ", fname),
                    Style::default().fg(Color::Black).bg(Color::Cyan),
                ),
                ratatui::text::Span::raw(format!(
                    "  {}/{}  j/k scroll · g/G top/bottom · q quit · e code-view",
                    app.scroll + 1,
                    app.lines.len(),
                )),
            ]);
            f.render_widget(Paragraph::new(status), chunks[1]);

            // Scrollbar
            let mut scrollbar_state = ScrollbarState::new(app.lines.len())
                .position(app.scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                chunks[0],
                &mut scrollbar_state,
            );
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                match (code, modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.scroll_down(1),
                    (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.scroll_up(1),
                    (KeyCode::PageDown, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => app.page_down(),
                    (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => app.page_up(),
                    (KeyCode::Char('g'), _) | (KeyCode::Home, _) => app.goto_top(),
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.goto_bottom(),
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

pub fn open_code_view(file: &Path) -> anyhow::Result<()> {
    // Phase 5: spawn $EDITOR / bat / less
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if which_exists("bat") { "bat".into() }
            else if which_exists("less") { "less".into() }
            else { "cat".into() }
        });

    let status = std::process::Command::new(&editor)
        .arg(file)
        .status()?;

    if !status.success() {
        anyhow::bail!("editor '{}' exited with {}", editor, status);
    }
    Ok(())
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

- [ ] **Step 2: Build and smoke-test against README.md**

```bash
cargo build --release 2>&1
./target/release/vellum README.md
```

Expected: TUI opens, shows README content, `j`/`k` scroll, `q` quits.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: TUI event loop with scrolling and status bar"
```

---

### Task 6: Phase 1 README and CHANGELOG

**Files:**
- Create: `README.md`
- Create: `CHANGELOG.md`

- [ ] **Step 1: Create `README.md`**

```markdown
# vellum

Rich Markdown viewer for the terminal. Renders headings, paragraphs, code blocks (syntax-highlighted), images, tables, and links in a full TUI.

## Requirements

- Rust ≥ 1.75
- `ffmpeg` (for video thumbnail extraction, Phase 4+)
- A terminal supporting one of: Kitty graphics, Sixel, or iTerm2 protocol (for images, Phase 2+)

## Install

```bash
cargo install --path .
```

## Usage

```bash
vellum <file.md>          # rich TUI view (default)
vellum --code <file.md>   # open in $EDITOR / bat / less
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `PgDn` / `Ctrl+F` | Page down |
| `PgUp` / `Ctrl+B` | Page up |
| `g` / `Home` | Top of document |
| `G` / `End` | Bottom of document |
| `q` / `Ctrl+C` | Quit |
| `e` | Open in code view |

## Development

```bash
./setup        # install deps + submodules
./build        # release build
./test         # run test suite
./lint         # fmt + clippy
./lint -f      # auto-fix
./run [file]   # build + run
```
```

- [ ] **Step 2: Create `CHANGELOG.md`**

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- Phase 1: core Markdown viewer — text, headings, code blocks, tables, lists, block quotes
- Syntax highlighting via syntect
- Scrollable TUI with status bar (ratatui + crossterm)
- Code-view mode (`-c` / `--code`) spawning $EDITOR/bat/less
- script-helpers + ci-helpers CI scaffold
```

- [ ] **Step 3: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: README and CHANGELOG for Phase 1"
```

---

## Phase 2 — Inline Images

**Goal:** Replace `[🖼 alt]` placeholders with actual images rendered inline using `ratatui-image`. Auto-detect terminal protocol (Kitty → Sixel → iTerm2 → halfblock fallback).

**Files:**
- Create: `src/image.rs`
- Modify: `src/app.rs` — add `ImageManager` to `App` state
- Modify: `src/renderer.rs` — yield `ImageSlot` markers; app fills them during draw

### Task 7: Image Loading and Protocol Detection

**Files:**
- Create: `src/image.rs`

- [ ] **Step 1: Add failing test**

```rust
// tests/image_tests.rs
use vellum::image::load_image;
use std::path::Path;

#[test]
fn test_load_png_succeeds() {
    // Create a 2x2 white PNG in /tmp for testing
    let path = "/tmp/vellum_test.png";
    image::RgbImage::from_pixel(2, 2, image::Rgb([255u8, 255, 255]))
        .save(path).unwrap();
    let img = load_image(path).unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
}

#[test]
fn test_load_missing_file_errors() {
    let result = load_image("/tmp/vellum_definitely_missing.png");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Implement `src/image.rs`**

```rust
// src/image.rs
use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;
use std::path::Path;

/// Load an image from a local path or return error.
pub fn load_image<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let img = image::open(path.as_ref())?;
    Ok(img)
}

/// Simple LRU-free in-memory cache: path/url → DynamicImage.
#[derive(Default)]
pub struct ImageCache {
    cache: HashMap<String, DynamicImage>,
}

impl ImageCache {
    pub fn get_or_load(&mut self, src: &str) -> Result<&DynamicImage> {
        if !self.cache.contains_key(src) {
            let img = if src.starts_with("http://") || src.starts_with("https://") {
                // Future: fetch via ureq; for now, unsupported
                anyhow::bail!("remote images not yet supported")
            } else {
                load_image(src)?
            };
            self.cache.insert(src.to_owned(), img);
        }
        Ok(self.cache.get(src).unwrap())
    }
}
```

- [ ] **Step 3: Integrate `ratatui-image` into `App`**

In `src/app.rs`, add `ratatui_image` integration. Modify `App` to hold a `ratatui_image::picker::Picker` and a map of pre-encoded images. During draw, replace image placeholder lines with `ratatui_image::StatefulImage` widgets rendered at the correct `Rect`.

> **Note:** `ratatui-image` renders images using absolute `Rect` coordinates, not inside `Line` text. The implementation replaces image placeholder lines with calls to `f.render_stateful_widget(StatefulImage::new(...), image_rect, &mut image_state)`.

Full integration code goes here — implement after reviewing `ratatui-image` 6.x API docs at:
`https://docs.rs/ratatui-image/latest/ratatui_image/`

- [ ] **Step 4: Run image tests**

```bash
cargo test --test image_tests -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/image.rs tests/image_tests.rs src/app.rs
git commit -m "feat: inline image rendering with ratatui-image"
```

---

## Phase 3 — Links and Anchor Navigation

**Goal:** External links open in browser via OSC 8. `#anchor` links jump to the matching heading offset. Tab/Shift+Tab cycle through links; Enter activates.

**Files:**
- Create: `src/links.rs`
- Create: `tests/links_tests.rs`
- Modify: `src/app.rs` — add `link_index`, `links` vec, Tab handler

### Task 8: Anchor Map Builder

**Files:**
- Create: `src/links.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/links_tests.rs
use vellum::links::{build_anchor_map, anchor_from_heading};

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
    use vellum::parser::Element;
    let elements = vec![
        Element::Heading { level: 1, text: "Introduction".into() },
        Element::Paragraph(vec![]),
        Element::Heading { level: 2, text: "Details".into() },
    ];
    let map = build_anchor_map(&elements);
    assert!(map.contains_key("introduction"));
    assert!(map.contains_key("details"));
    assert!(map["introduction"] < map["details"]);
}
```

- [ ] **Step 2: Implement `src/links.rs`**

```rust
// src/links.rs
use std::collections::HashMap;
use crate::parser::Element;

/// Convert a heading string to a GitHub-style anchor slug.
pub fn anchor_from_heading(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Map heading anchor slugs to approximate line offsets in the rendered output.
/// Each heading occupies 2 lines (the heading line + blank gap).
pub fn build_anchor_map(elements: &[Element]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut line = 0usize;
    for el in elements {
        match el {
            Element::Heading { text, .. } => {
                map.insert(anchor_from_heading(text), line);
                line += 2;
            }
            Element::Paragraph(_) | Element::CodeBlock { .. } | Element::HRule | Element::Image { .. } => {
                line += 2;
            }
            Element::Table { rows, .. } => {
                line += rows.len() + 3;
            }
            Element::List { items, .. } => {
                line += items.len() + 1;
            }
            _ => line += 1,
        }
    }
    map
}

/// Open a URL in the system browser.
pub fn open_url(url: &str) -> anyhow::Result<()> {
    let status = if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open").arg(url).status()?
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()?
    } else {
        anyhow::bail!("unsupported platform for open_url")
    };
    if !status.success() {
        anyhow::bail!("open_url failed for {}", url);
    }
    Ok(())
}

/// Write an OSC 8 hyperlink to a writer (for terminals that support it).
pub fn osc8_link(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}
```

- [ ] **Step 3: Add Tab/Enter link navigation to `src/app.rs`**

In the `App` struct, add:

```rust
links: Vec<(String, usize)>,  // (href, rendered-line-offset)
link_cursor: Option<usize>,
anchor_map: std::collections::HashMap<String, usize>,
```

Populate `links` from the `Element` tree by walking spans for `Span::Link`. In the event loop, add:

```rust
(KeyCode::Tab, _) => {
    if let Some(i) = app.link_cursor {
        app.link_cursor = Some((i + 1) % app.links.len().max(1));
    } else if !app.links.is_empty() {
        app.link_cursor = Some(0);
    }
    if let Some(i) = app.link_cursor {
        app.scroll = app.links[i].1.min(
            app.lines.len().saturating_sub(app.viewport_height)
        );
    }
}
(KeyCode::BackTab, _) => { /* reverse */ }
(KeyCode::Enter, _) => {
    if let Some(i) = app.link_cursor {
        let href = &app.links[i].0;
        if href.starts_with('#') {
            let anchor = &href[1..];
            if let Some(&offset) = app.anchor_map.get(anchor) {
                app.scroll = offset;
            }
        } else {
            let _ = crate::links::open_url(href);
        }
    }
}
```

- [ ] **Step 4: Run link tests**

```bash
cargo test --test links_tests -- --nocapture
```

Expected: all 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/links.rs tests/links_tests.rs src/app.rs
git commit -m "feat: link navigation — OSC 8 external, anchor jump internal"
```

---

## Phase 4 — Video Thumbnails

**Goal:** For `<video>` tags or markdown links ending in `.mp4`/`.webm`/`.mov`, extract frame 0 via `ffmpeg` and display as an inline image using the Phase 2 image pipeline.

**Files:**
- Create: `src/video.rs`

### Task 9: FFmpeg Thumbnail Extraction

- [ ] **Step 1: Write failing test**

```rust
// tests/video_tests.rs
use vellum::video::extract_thumbnail;

#[test]
fn test_extract_thumbnail_missing_file_errors() {
    let result = extract_thumbnail("/tmp/vellum_missing.mp4");
    assert!(result.is_err());
}

#[test]
fn test_ffmpeg_unavailable_returns_clear_error() {
    // Force PATH to empty so ffmpeg isn't found
    std::env::set_var("PATH", "");
    let result = extract_thumbnail("/tmp/vellum_missing.mp4");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("ffmpeg") || msg.contains("not found") || msg.contains("No such file"),
        "error should mention ffmpeg or file: {msg}");
}
```

- [ ] **Step 2: Implement `src/video.rs`**

```rust
// src/video.rs
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Extract the first frame of a video file as a PNG using ffmpeg.
/// Returns a `NamedTempFile` that stays alive as long as the caller holds it.
pub fn extract_thumbnail<P: AsRef<Path>>(video_path: P) -> Result<NamedTempFile> {
    let video_path = video_path.as_ref();
    if !video_path.exists() {
        bail!("video file not found: {}", video_path.display());
    }

    let tmp = tempfile::Builder::new()
        .prefix("vellum_thumb_")
        .suffix(".png")
        .tempfile()?;

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", &video_path.to_string_lossy(),
            "-vframes", "1",
            "-q:v", "2",
            &tmp.path().to_string_lossy(),
        ])
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("ffmpeg not found — install ffmpeg to enable video thumbnails")
            } else {
                anyhow::anyhow!("ffmpeg failed: {}", e)
            }
        })?;

    if !status.success() {
        bail!("ffmpeg exited with {}", status);
    }

    Ok(tmp)
}
```

- [ ] **Step 3: Wire into renderer**

In `src/renderer.rs`, change the `Element::Video` arm to call `video::extract_thumbnail` and pass the resulting path to the image cache (same pipeline as Phase 2 images).

- [ ] **Step 4: Run tests**

```bash
cargo test --test video_tests -- --nocapture
```

Expected: `test_extract_thumbnail_missing_file_errors` passes. `test_ffmpeg_unavailable_returns_clear_error` passes.

- [ ] **Step 5: Commit**

```bash
git add src/video.rs tests/video_tests.rs
git commit -m "feat: video thumbnail extraction via ffmpeg"
```

---

## Phase 5 — Code View Mode (polish)

*Already scaffolded in `open_code_view` in `src/app.rs`.* Full implementation in Task 5 Step 1 above. Additional polish:

- Add `--code` to argument docs in README
- Detect `bat` with `--paging=always` for paginated output
- Add `e` keybinding in the TUI that exits the viewer, spawns code view, then re-enters the TUI on return

**Key change in event loop:**

```rust
(KeyCode::Char('e'), _) => {
    // Tear down TUI temporarily
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    // Spawn code view
    let _ = crate::app::open_code_view(&file_path);
    // Re-enter TUI
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
}
```

**Commit:** `feat: e key re-enters TUI after code-view`

---

## Phase 6 — Polish: Mouse, Search, Config

### Task 10: In-Document Search

**Files:**
- Create: `src/search.rs`

- [ ] **Step 1: Write failing test**

```rust
// tests/search_tests.rs
use vellum::search::{search_lines, SearchResult};
use ratatui::text::Line;

#[test]
fn test_search_finds_match() {
    let lines: Vec<Line> = vec![
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
    let lines = vec![Line::from("UPPER"), Line::from("lower"), Line::from("Mixed")];
    let results = search_lines(&lines, "upper");
    assert_eq!(results.len(), 1);
}

#[test]
fn test_search_no_match() {
    let lines = vec![Line::from("nothing here")];
    let results = search_lines(&lines, "xyz");
    assert!(results.is_empty());
}
```

- [ ] **Step 2: Implement `src/search.rs`**

```rust
// src/search.rs
use ratatui::text::Line;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line_index: usize,
    pub byte_offset: usize,
}

/// Search rendered lines for `query` (case-insensitive).
pub fn search_lines(lines: &[Line], query: &str) -> Vec<SearchResult> {
    let q = query.to_lowercase();
    lines.iter().enumerate().filter_map(|(i, line)| {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>().to_lowercase();
        text.find(&q).map(|offset| SearchResult { line_index: i, byte_offset: offset })
    }).collect()
}
```

- [ ] **Step 3: Add `/` keybinding and search overlay to `src/app.rs`**

```rust
// In App struct:
search_query: String,
search_mode: bool,
search_results: Vec<crate::search::SearchResult>,
search_cursor: usize,

// In event loop:
(KeyCode::Char('/'), _) => { app.search_mode = true; app.search_query.clear(); }
// When search_mode == true, printable chars append to search_query
// Enter confirms: run search_lines, jump to first result
// Esc cancels search mode
// n / N cycle search_results
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test search_tests -- --nocapture
```

Expected: all 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/search.rs tests/search_tests.rs src/app.rs
git commit -m "feat: / search with n/N navigation"
```

---

### Task 11: Mouse Support

- [ ] **Step 1: Enable mouse capture in `src/app.rs`**

```rust
// After enable_raw_mode():
crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;

// Before LeaveAlternateScreen:
crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture)?;

// In event loop, add mouse handler:
if let Event::Mouse(mouse) = event::read()? {
    match mouse.kind {
        crossterm::event::MouseEventKind::ScrollDown => app.scroll_down(3),
        crossterm::event::MouseEventKind::ScrollUp => app.scroll_up(3),
        _ => {}
    }
}
```

- [ ] **Step 2: Verify scroll wheel works in supported terminal**

```bash
./run README.md
```

Expected: mouse scroll wheel moves viewport.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: mouse scroll wheel support"
```

---

### Task 12: `--page` About Screen

**Goal:** `vellum --page` prints author/project info (GitHub, LinkedIn) without entering the TUI. Useful as a quick identity card for the binary.

**Files:**
- Modify: `src/main.rs` — add `--page` flag to `Cli`
- Create: `src/about.rs` — render the about page

- [ ] **Step 1: Add `--page` flag to `Cli` in `src/main.rs`**

```rust
#[derive(Parser, Debug)]
#[command(name = "vellum", about = "Rich Markdown viewer for the terminal")]
pub struct Cli {
    /// Markdown file to open
    #[arg(required_unless_present = "page")]
    pub file: Option<std::path::PathBuf>,

    /// Open in code view (spawns $EDITOR / bat / less)
    #[arg(short, long)]
    pub code: bool,

    /// Show author and project info
    #[arg(long)]
    pub page: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.page {
        about::print_page();
        return Ok(());
    }

    let file = cli.file.expect("file required when --page not set");
    if cli.code {
        app::open_code_view(&file)?;
    } else {
        app::run(&file)?;
    }

    Ok(())
}
```

- [ ] **Step 2: Create `src/about.rs`**

```rust
// src/about.rs

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_page() {
    println!();
    println!("  \x1b[1;33mvellum\x1b[0m  v{}  — Rich Markdown viewer for the terminal", VERSION);
    println!();
    println!("  \x1b[1mAuthor\x1b[0m   Nik Reljin");
    println!("  \x1b[1mGitHub\x1b[0m   \x1b[4;36mhttps://github.com/nikolareljin\x1b[0m");
    println!("  \x1b[1mLinkedIn\x1b[0m \x1b[4;36mhttps://www.linkedin.com/in/nikolareljin\x1b[0m");
    println!();
    println!("  Source:  https://github.com/nikolareljin/vellum");
    println!("  License: MIT");
    println!();
}
```

- [ ] **Step 3: Add `mod about;` to `src/main.rs` and `src/lib.rs`**

- [ ] **Step 4: Verify**

```bash
cargo build --release
./target/release/vellum --page
```

Expected: formatted about block printed, no TUI opened.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/about.rs src/lib.rs
git commit -m "feat: --page flag shows author/project info"
```

---

### Task 14: CHANGELOG + 0.1.0 Tag Prep

- [ ] **Step 1: Update CHANGELOG.md** — fill in all Phase 1 entries under `[0.1.0]` with date.

- [ ] **Step 2: Bump `Cargo.toml` version** to `0.1.0` (already set).

- [ ] **Step 3: Commit and push**

```bash
git add CHANGELOG.md Cargo.toml
git commit -m "chore: prepare 0.1.0 release"
git push origin main
```

- [ ] **Step 4: Tag** *(only on user confirmation)*

```bash
git tag 0.1.0
git push origin 0.1.0
```

> **Note:** Tags follow no-v-prefix convention for this repo. Never force-move tags. CI `release.yml` triggers on `*.*.*` tag patterns and builds multi-platform binaries via `rust-release.yml@production`.

---

## Keybinding Summary (Final)

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down 1 line |
| `k` / `↑` | Scroll up 1 line |
| `PgDn` / `Ctrl+F` | Page down |
| `PgUp` / `Ctrl+B` | Page up |
| `g` / `Home` | Top of document |
| `G` / `End` | Bottom of document |
| `Tab` | Next link |
| `Shift+Tab` | Previous link |
| `Enter` | Follow link / jump anchor |
| `e` | Code view (spawns `$EDITOR`/`bat`/`less`) |
| `/` | Search |
| `n` / `N` | Next / previous search match |
| `q` / `Ctrl+C` | Quit |
| Mouse scroll | Scroll 3 lines |

---

## CI / Scripts Reference

| Command | Action |
|---------|--------|
| `./setup` | Install system deps (`ffmpeg`) + `rustfmt`/`clippy` + init submodules |
| `./build` | `cargo build --release` |
| `./test` | `cargo test --verbose` |
| `./lint` | `cargo fmt --check` + `cargo clippy -D warnings` |
| `./lint -f` | Auto-fix formatting and clippy |
| `./run [file]` | Build + run against given file (default: `README.md`) |
| `./update` | `git submodule update --remote --merge` |

Workflows (via `ci-helpers@production`):
- `rust.yml` — CI on push/PR to `main`
- `rust-scan.yml` — `rustfmt` + `clippy` + `cargo audit` on push/PR
- `release.yml` — multi-platform binaries on `*.*.*` tags
