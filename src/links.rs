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
/// Each block element occupies ~2 lines (content + blank gap).
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

/// Collect all link hrefs and their approximate rendered-line offsets from elements.
pub fn collect_links(elements: &[Element]) -> Vec<(String, usize)> {
    let mut links = Vec::new();
    let mut line = 0usize;
    for el in elements {
        match el {
            Element::Paragraph(spans) => {
                for span in spans {
                    if let crate::parser::Span::Link { href, .. } = span {
                        links.push((href.clone(), line));
                    }
                }
                line += 2;
            }
            Element::Heading { .. } => line += 2,
            Element::CodeBlock { .. } | Element::HRule | Element::Image { .. } => line += 2,
            Element::Table { rows, .. } => line += rows.len() + 3,
            Element::List { items, .. } => line += items.len() + 1,
            _ => line += 1,
        }
    }
    links
}
