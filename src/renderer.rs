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
        lines.push(Line::from(""));
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
            let lang_str = lang.as_deref().unwrap_or("code");
            let header = format!(" {} ", lang_str);
            out.push(Line::from(vec![RSpan::styled(
                header,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )]));
            let styled_lines = highlight_code(code, lang.as_deref());
            for sl in styled_lines {
                let rspans: Vec<RSpan<'static>> = sl
                    .into_iter()
                    .map(|(style, text)| {
                        let fg = syntect_color(style.foreground);
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
            let bar = RSpan::styled(
                "│ ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            );
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
                } else {
                    out.push(Line::from(bullet_span));
                    continue;
                }
                out.extend(item_lines);
            }
        }

        Element::Table { headers, rows } => {
            let col_widths: Vec<usize> = headers.iter().enumerate().map(|(i, h)| {
                let max_row = rows
                    .iter()
                    .map(|r| r.get(i).map(|c| c.len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                h.len().max(max_row).max(3)
            }).collect();

            let header_spans: Vec<RSpan<'static>> = headers.iter().enumerate().map(|(i, h)| {
                RSpan::styled(
                    format!(" {:<width$} │", h, width = col_widths[i]),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )
            }).collect();
            out.push(Line::from(header_spans));

            let sep: String = col_widths.iter().map(|w| "─".repeat(w + 2) + "┼").collect();
            out.push(Line::from(vec![RSpan::styled(sep, Style::default().fg(Color::DarkGray))]));

            for row in rows {
                let row_spans: Vec<RSpan<'static>> = col_widths.iter().enumerate().map(|(i, w)| {
                    let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    RSpan::raw(format!(" {:<width$} │", cell, width = w))
                }).collect();
                out.push(Line::from(row_spans));
            }
        }

        Element::Image { alt, .. } => {
            // Phase 1 placeholder — Phase 2 replaces with ratatui-image widget
            out.push(Line::from(vec![
                RSpan::styled("[🖼  ", Style::default().fg(Color::Magenta)),
                RSpan::raw(alt.clone()),
                RSpan::styled("]", Style::default().fg(Color::Magenta)),
            ]));
        }

        Element::Video { .. } => {
            // Phase 4 placeholder — replaced with ffmpeg thumbnail
            out.push(Line::from(vec![RSpan::styled(
                "[▶  video thumbnail]",
                Style::default().fg(Color::Blue),
            )]));
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
            RSpan::styled(
                text.clone(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            ),
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

fn syntect_color(c: syntect::highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}
