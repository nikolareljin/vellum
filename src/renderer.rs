use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};

use crate::highlight::highlight_code;
use crate::parser::{Element, Span};

// Heading colours: H1 → H6
const HEADING_COLORS: [Color; 6] = [
    Color::Rgb(255, 215, 0),   // H1 — gold
    Color::Rgb(100, 200, 255), // H2 — sky blue
    Color::Rgb(100, 220, 120), // H3 — seafoam
    Color::Rgb(220, 130, 255), // H4 — lilac
    Color::Rgb(100, 180, 255), // H5 — steel blue
    Color::Rgb(180, 180, 180), // H6 — silver
];

// Dark background used inside code blocks (close to VSCode's dark theme)
const CODE_BG: Color = Color::Rgb(30, 30, 36);
// Slightly lighter for the language label bar
const CODE_HEADER_BG: Color = Color::Rgb(45, 45, 55);

/// Convert a list of `Element`s into a flat list of ratatui `Line`s ready for display.
pub fn render_elements(elements: &[Element]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for element in elements {
        render_element(element, &mut lines, 0);
        lines.push(Line::from(""));
    }
    lines
}

fn render_element(element: &Element, out: &mut Vec<Line<'static>>, indent: usize) {
    match element {
        Element::Heading { level, text } => render_heading(*level, text, out),

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

        Element::CodeBlock { lang, code } => render_code_block(lang.as_deref(), code, out),

        Element::HRule => {
            out.push(Line::from(vec![RSpan::styled(
                "─".repeat(72),
                Style::default().fg(Color::Rgb(80, 80, 80)),
            )]));
        }

        Element::BlockQuote(inner) => {
            let bar_style = Style::default()
                .fg(Color::Rgb(100, 180, 255))
                .add_modifier(Modifier::BOLD);
            let mut inner_lines: Vec<Line<'static>> = Vec::new();
            for el in inner {
                render_element(el, &mut inner_lines, indent + 2);
            }
            for mut line in inner_lines {
                // Apply italic to all existing spans to give blockquote flavour
                for span in &mut line.spans {
                    span.style = span
                        .style
                        .patch(Style::default().add_modifier(Modifier::ITALIC));
                }
                line.spans.insert(0, RSpan::styled("▌ ", bar_style));
                out.push(line);
            }
        }

        Element::List { ordered, items } => {
            for (idx, item_elements) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("  {}. ", idx + 1)
                } else {
                    "  • ".to_string()
                };
                let bullet_style = Style::default()
                    .fg(Color::Rgb(100, 200, 255))
                    .add_modifier(Modifier::BOLD);
                let bullet_span = RSpan::styled(bullet, bullet_style);

                let mut item_lines: Vec<Line<'static>> = Vec::new();
                for el in item_elements {
                    render_element(el, &mut item_lines, indent + 4);
                }
                if let Some(first) = item_lines.first_mut() {
                    first.spans.insert(0, bullet_span);
                } else {
                    out.push(Line::from(bullet_span));
                    continue;
                }
                out.extend(item_lines);
            }
        }

        Element::Table { headers, rows } => render_table(headers, rows, out),

        Element::Image { alt, .. } => {
            // Phase 1 placeholder — Phase 2 replaces with ratatui-image widget.
            out.push(Line::from(vec![
                RSpan::styled(
                    " 🖼  ",
                    Style::default().fg(Color::Rgb(200, 120, 255)),
                ),
                RSpan::styled(
                    alt.clone(),
                    Style::default()
                        .fg(Color::Rgb(160, 160, 160))
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        Element::Video { .. } => {
            out.push(Line::from(vec![RSpan::styled(
                " ▶  video thumbnail",
                Style::default().fg(Color::Rgb(100, 180, 255)),
            )]));
        }

        Element::Break => out.push(Line::from("")),
    }
}

// ─── Headings ────────────────────────────────────────────────────────────────

fn render_heading(level: u8, text: &str, out: &mut Vec<Line<'static>>) {
    let color = HEADING_COLORS[(level as usize).saturating_sub(1).min(5)];
    let char_count = text.chars().count();

    match level {
        1 => {
            // H1: full bold+underlined title, gold rule beneath
            let style = Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            out.push(Line::from(vec![RSpan::styled(
                format!("  {}", text),
                style,
            )]));
            out.push(Line::from(vec![RSpan::styled(
                format!("  {}", "═".repeat(char_count.min(72))),
                Style::default().fg(color),
            )]));
        }
        2 => {
            // H2: bold title, thin rule beneath, indented
            let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            out.push(Line::from(vec![
                RSpan::styled("  ", Style::default()),
                RSpan::styled(text.to_string(), style),
            ]));
            out.push(Line::from(vec![
                RSpan::styled("  ", Style::default()),
                RSpan::styled(
                    "─".repeat(char_count.min(68)),
                    Style::default().fg(color),
                ),
            ]));
        }
        3 => {
            // H3: coloured marker bar + bold title
            out.push(Line::from(vec![
                RSpan::styled(
                    "    ▌ ",
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                RSpan::styled(
                    text.to_string(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        level => {
            // H4–H6: progressive indent, bold, dimmer
            let indent = " ".repeat((level as usize) * 2);
            let marker = "● ";
            out.push(Line::from(vec![
                RSpan::raw(indent),
                RSpan::styled(
                    marker,
                    Style::default().fg(Color::Rgb(100, 100, 100)),
                ),
                RSpan::styled(
                    text.to_string(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    }
}

// ─── Code blocks ─────────────────────────────────────────────────────────────

fn render_code_block(lang: Option<&str>, code: &str, out: &mut Vec<Line<'static>>) {
    let label = lang.unwrap_or("text").to_uppercase();
    let label_style = Style::default()
        .fg(Color::Rgb(200, 200, 200))
        .bg(CODE_HEADER_BG)
        .add_modifier(Modifier::BOLD);

    // Top bar: language label
    out.push(Line::from(vec![
        RSpan::styled(format!("  {} ", label), label_style),
        // Pad the rest of the bar (approximate; terminal will clip)
        RSpan::styled(
            " ".repeat(64_usize.saturating_sub(label.len() + 3)),
            Style::default().bg(CODE_HEADER_BG),
        ),
    ]));

    // Syntax-highlighted lines, each with code background
    let styled_lines = highlight_code(code, lang);
    for sl in styled_lines {
        let mut rspans: Vec<RSpan<'static>> = vec![
            RSpan::styled("  ", Style::default().bg(CODE_BG)), // left gutter
        ];
        rspans.extend(sl.into_iter().map(|(s, text)| {
            RSpan::styled(
                text,
                Style::default()
                    .fg(syntect_color(s.foreground))
                    .bg(CODE_BG),
            )
        }));
        rspans.push(RSpan::styled(" ", Style::default().bg(CODE_BG))); // right pad
        out.push(Line::from(rspans));
    }

    // Bottom border
    out.push(Line::from(vec![RSpan::styled(
        "─".repeat(66),
        Style::default().fg(Color::Rgb(60, 60, 70)),
    )]));
}

// ─── Tables ──────────────────────────────────────────────────────────────────

fn render_table(headers: &[String], rows: &[Vec<String>], out: &mut Vec<Line<'static>>) {
    if headers.is_empty() {
        return;
    }
    let col_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let max_row = rows
                .iter()
                .map(|r| r.get(i).map(|c| c.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            h.len().max(max_row).max(3)
        })
        .collect();

    // Top border
    let top: String = col_widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let seg = "─".repeat(w + 2);
            if i == 0 {
                format!("┌{}┬", seg)
            } else if i == col_widths.len() - 1 {
                format!("{}┐", seg)
            } else {
                format!("{}┬", seg)
            }
        })
        .collect();
    out.push(Line::from(vec![RSpan::styled(
        top,
        Style::default().fg(Color::Rgb(80, 80, 100)),
    )]));

    // Header row
    let header_spans: Vec<RSpan<'static>> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let cell = format!("│ {:<width$} ", h, width = col_widths[i]);
            RSpan::styled(
                cell,
                Style::default()
                    .fg(Color::Rgb(255, 215, 0))
                    .add_modifier(Modifier::BOLD),
            )
        })
        .chain(std::iter::once(RSpan::styled(
            "│",
            Style::default().fg(Color::Rgb(80, 80, 100)),
        )))
        .collect();
    out.push(Line::from(header_spans));

    // Separator
    let sep: String = col_widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let seg = "─".repeat(w + 2);
            if i == 0 {
                format!("├{}┼", seg)
            } else if i == col_widths.len() - 1 {
                format!("{}┤", seg)
            } else {
                format!("{}┼", seg)
            }
        })
        .collect();
    out.push(Line::from(vec![RSpan::styled(
        sep,
        Style::default().fg(Color::Rgb(80, 80, 100)),
    )]));

    // Data rows
    for row in rows {
        let row_spans: Vec<RSpan<'static>> = col_widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                RSpan::raw(format!("│ {:<width$} ", cell, width = w))
            })
            .chain(std::iter::once(RSpan::raw("│")))
            .collect();
        out.push(Line::from(row_spans));
    }

    // Bottom border
    let bottom: String = col_widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let seg = "─".repeat(w + 2);
            if i == 0 {
                format!("└{}┴", seg)
            } else if i == col_widths.len() - 1 {
                format!("{}┘", seg)
            } else {
                format!("{}┴", seg)
            }
        })
        .collect();
    out.push(Line::from(vec![RSpan::styled(
        bottom,
        Style::default().fg(Color::Rgb(80, 80, 100)),
    )]));
}

// ─── Inline spans ─────────────────────────────────────────────────────────────

fn render_span(span: &Span) -> Vec<RSpan<'static>> {
    match span {
        Span::Text(t) => vec![RSpan::raw(t.clone())],

        Span::Bold(t) => vec![RSpan::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )],

        Span::Italic(t) => vec![RSpan::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::ITALIC),
        )],

        Span::BoldItalic(t) => vec![RSpan::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
        )],

        Span::Code(t) => vec![RSpan::styled(
            format!(" {} ", t),
            Style::default()
                .fg(Color::Rgb(250, 200, 100))  // amber
                .bg(Color::Rgb(40, 40, 48)),    // subtle dark bg — no backticks shown
        )],

        Span::Link { text, .. } => vec![
            // Show only the link text; href visible in status bar when focused
            RSpan::styled(
                text.clone(),
                Style::default()
                    .fg(Color::Rgb(80, 190, 255))
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ],

        Span::Strikethrough(t) => vec![RSpan::styled(
            t.clone(),
            Style::default()
                .fg(Color::Rgb(120, 120, 120))
                .add_modifier(Modifier::CROSSED_OUT),
        )],
    }
}

fn syntect_color(c: syntect::highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}
