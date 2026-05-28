use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};

use crate::highlight::highlight_code;
use crate::parser::{Element, Span};
use crate::theme::{BlockColors, CodeColors, HeadingColors, InlineColors, Theme};

/// Convert a list of `Element`s into a flat list of ratatui `Line`s ready for display.
pub fn render_elements(elements: &[Element], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for element in elements {
        render_element(element, &mut lines, 0, theme);
        lines.push(Line::from(""));
    }
    lines
}

fn render_element(element: &Element, out: &mut Vec<Line<'static>>, indent: usize, theme: &Theme) {
    match element {
        Element::Heading { level, text } => {
            render_heading(*level, text, &theme.headings, theme.blocks.hrule, out)
        }

        Element::Paragraph(spans) => {
            let prefix = " ".repeat(indent);
            let mut rspans: Vec<RSpan<'static>> = Vec::new();
            if !prefix.is_empty() {
                rspans.push(RSpan::raw(prefix));
            }
            for span in spans {
                rspans.extend(render_span(span, &theme.inline));
            }
            out.push(Line::from(rspans));
        }

        Element::CodeBlock { lang, code } => {
            render_code_block(lang.as_deref(), code, &theme.code, theme.blocks.hrule, out);
        }

        Element::HRule => {
            out.push(Line::from(vec![RSpan::styled(
                "─".repeat(72),
                Style::default().fg(theme.blocks.hrule.to_color()),
            )]));
        }

        Element::BlockQuote(inner) => {
            let bar_style = Style::default()
                .fg(theme.blocks.blockquote.to_color())
                .add_modifier(Modifier::BOLD);
            let mut inner_lines: Vec<Line<'static>> = Vec::new();
            for el in inner {
                render_element(el, &mut inner_lines, indent + 2, theme);
            }
            for mut line in inner_lines {
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
            let bullet_color = theme.blocks.list_bullet.to_color();
            for (idx, item_elements) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("  {}. ", idx + 1)
                } else {
                    "  • ".to_string()
                };
                let bullet_style = Style::default()
                    .fg(bullet_color)
                    .add_modifier(Modifier::BOLD);
                let bullet_span = RSpan::styled(bullet, bullet_style);

                let mut item_lines: Vec<Line<'static>> = Vec::new();
                for el in item_elements {
                    render_element(el, &mut item_lines, indent + 4, theme);
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

        Element::Table { headers, rows } => render_table(headers, rows, &theme.blocks, out),

        Element::Image { alt, .. } => {
            out.push(Line::from(vec![
                RSpan::styled(
                    " 🖼  ",
                    Style::default().fg(theme.blocks.image_icon.to_color()),
                ),
                RSpan::styled(
                    alt.clone(),
                    Style::default()
                        .fg(theme.blocks.image_alt.to_color())
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        Element::Video { .. } => {
            out.push(Line::from(vec![RSpan::styled(
                " ▶  video thumbnail",
                Style::default().fg(theme.blocks.blockquote.to_color()),
            )]));
        }
    }
}

// ─── Headings ────────────────────────────────────────────────────────────────

fn render_heading(
    level: u8,
    text: &str,
    hc: &HeadingColors,
    marker_color: crate::theme::Rgb,
    out: &mut Vec<Line<'static>>,
) {
    let color = match level {
        1 => hc.h1.to_color(),
        2 => hc.h2.to_color(),
        3 => hc.h3.to_color(),
        4 => hc.h4.to_color(),
        5 => hc.h5.to_color(),
        _ => hc.h6.to_color(),
    };
    let char_count = text.chars().count();

    match level {
        1 => {
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
            let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            out.push(Line::from(vec![
                RSpan::styled("  ", Style::default()),
                RSpan::styled(text.to_string(), style),
            ]));
            out.push(Line::from(vec![
                RSpan::styled("  ", Style::default()),
                RSpan::styled("─".repeat(char_count.min(68)), Style::default().fg(color)),
            ]));
        }
        3 => {
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
            let indent = " ".repeat((level as usize) * 2);
            out.push(Line::from(vec![
                RSpan::raw(indent),
                RSpan::styled("● ", Style::default().fg(marker_color.to_color())),
                RSpan::styled(
                    text.to_string(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    }
}

// ─── Code blocks ─────────────────────────────────────────────────────────────

fn render_code_block(
    lang: Option<&str>,
    code: &str,
    cc: &CodeColors,
    border_color: crate::theme::Rgb,
    out: &mut Vec<Line<'static>>,
) {
    let label = lang.unwrap_or("text").to_uppercase();
    let header_bg = cc.header_bg.to_color();
    let code_bg = cc.bg.to_color();

    let label_style = Style::default()
        .fg(cc.label_fg.to_color())
        .bg(header_bg)
        .add_modifier(Modifier::BOLD);

    // Language label bar
    out.push(Line::from(vec![
        RSpan::styled(format!("  {} ", label), label_style),
        RSpan::styled(
            " ".repeat(64_usize.saturating_sub(label.len() + 3)),
            Style::default().bg(header_bg),
        ),
    ]));

    // Syntax-highlighted body
    let styled_lines = highlight_code(code, lang, cc);
    for sl in styled_lines {
        let mut rspans: Vec<RSpan<'static>> =
            vec![RSpan::styled("  ", Style::default().bg(code_bg))];
        rspans.extend(sl.into_iter().map(|(s, text)| {
            RSpan::styled(
                text,
                Style::default().fg(syntect_color(s.foreground)).bg(code_bg),
            )
        }));
        rspans.push(RSpan::styled(" ", Style::default().bg(code_bg)));
        out.push(Line::from(rspans));
    }

    out.push(Line::from(vec![RSpan::styled(
        "─".repeat(66),
        Style::default().fg(border_color.to_color()),
    )]));
}

// ─── Tables ──────────────────────────────────────────────────────────────────

fn render_table(
    headers: &[String],
    rows: &[Vec<String>],
    bc: &BlockColors,
    out: &mut Vec<Line<'static>>,
) {
    if headers.is_empty() {
        return;
    }
    let border_color = bc.table_border.to_color();
    let header_color = bc.table_header.to_color();

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
        Style::default().fg(border_color),
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
                    .fg(header_color)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .chain(std::iter::once(RSpan::styled(
            "│",
            Style::default().fg(border_color),
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
        Style::default().fg(border_color),
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
        Style::default().fg(border_color),
    )]));
}

// ─── Inline spans ─────────────────────────────────────────────────────────────

fn render_span(span: &Span, ic: &InlineColors) -> Vec<RSpan<'static>> {
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
                .fg(ic.code_fg.to_color())
                .bg(ic.code_bg.to_color()),
        )],

        Span::Link { text, .. } => vec![RSpan::styled(
            text.clone(),
            Style::default()
                .fg(ic.link.to_color())
                .add_modifier(Modifier::UNDERLINED),
        )],

        Span::Strikethrough(t) => vec![RSpan::styled(
            t.clone(),
            Style::default()
                .fg(ic.strikethrough.to_color())
                .add_modifier(Modifier::CROSSED_OUT),
        )],
    }
}

fn syntect_color(c: syntect::highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}
