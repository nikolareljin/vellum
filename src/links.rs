use std::collections::HashMap;

use crate::parser::{Element, Span};

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

/// Open a URL in the system browser.
pub fn open_url(url: &str) -> anyhow::Result<()> {
    // Allowlist schemes to prevent argument injection via flag-looking URLs
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:"))
    {
        anyhow::bail!("refusing to open non-http(s) url: {}", url);
    }
    if url.starts_with('-') {
        anyhow::bail!("refusing url starting with '-'");
    }

    let status = if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open")
            .arg("--")
            .arg(url)
            .status()?
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

/// Map heading anchor slugs to approximate line offsets in the rendered output.
/// Each block element occupies ~2 lines (content + blank gap).
///
/// Note: `app::build_display_lines` now builds an accurate anchor map using
/// actual display-line indices; this function is kept for tests and external
/// callers that only need the approximate offsets.
#[allow(dead_code)]
pub fn build_anchor_map(elements: &[Element]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut line = 0usize;
    count_anchor_lines(elements, &mut map, &mut line);
    map
}

fn count_anchor_lines(elements: &[Element], map: &mut HashMap<String, usize>, line: &mut usize) {
    for el in elements {
        match el {
            Element::Heading { text, .. } => {
                map.insert(anchor_from_heading(text), *line);
                *line += 2;
            }
            Element::Paragraph(_)
            | Element::CodeBlock { .. }
            | Element::HRule
            | Element::Image { .. }
            | Element::Video { .. } => {
                *line += 2;
            }
            Element::Table { rows, .. } => {
                *line += rows.len() + 3;
            }
            Element::List { items, .. } => {
                for item in items {
                    count_anchor_lines(item, map, line);
                }
            }
            Element::BlockQuote(inner) => {
                count_anchor_lines(inner, map, line);
            }
        }
    }
}

/// Collect all link hrefs with their approximate rendered-line offsets.
/// Recurses into list items and blockquotes.
///
/// Note: `app::build_display_lines` now builds an accurate link map using
/// actual display-line indices; this function is kept for tests and external
/// callers that only need the approximate offsets.
#[allow(dead_code)]
pub fn collect_links(elements: &[Element]) -> Vec<(String, usize)> {
    let mut links = Vec::new();
    let mut line = 0usize;
    collect_links_inner(elements, &mut links, &mut line);
    links
}

fn collect_links_inner(elements: &[Element], links: &mut Vec<(String, usize)>, line: &mut usize) {
    for el in elements {
        match el {
            Element::Paragraph(spans) => {
                collect_span_links(spans, links, *line);
                *line += 2;
            }
            Element::Heading { .. } => *line += 2,
            Element::CodeBlock { .. }
            | Element::HRule
            | Element::Image { .. }
            | Element::Video { .. } => {
                *line += 2;
            }
            Element::Table { rows, .. } => *line += rows.len() + 3,
            Element::List { items, .. } => {
                for item in items {
                    collect_links_inner(item, links, line);
                }
            }
            Element::BlockQuote(inner) => {
                collect_links_inner(inner, links, line);
            }
        }
    }
}

fn collect_span_links(spans: &[Span], links: &mut Vec<(String, usize)>, line: usize) {
    for span in spans {
        if let Span::Link { href, .. } = span {
            links.push((href.clone(), line));
        }
    }
}
