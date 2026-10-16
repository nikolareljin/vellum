//! One rule for text that reaches the screen, the clipboard or a search.
//!
//! Markdown is untrusted input.  A document (or a pasted search query) can
//! carry characters that never show but change what the terminal or a later
//! paste does: C0/C1 controls (ESC sequences, CR overwrites, BEL) and the
//! bidi embedding, override and isolate marks used to reorder text
//! ("Trojan Source").  They are removed before anything else sees the text.

use crate::parser::{Element, Span};

/// Longest search query accepted, in characters.
pub const MAX_QUERY_CHARS: usize = 200;

/// True for characters that are invisible but change meaning.  Tab and
/// newline are kept: they are layout, and code blocks need them.
pub fn is_hidden_control(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            c,
            // bidi embeddings, overrides, isolates and marks
            '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{061C}'
                // invisible formatting: soft hyphen, Mongolian vowel separator,
                // zero width space, word joiner and invisible operators, BOM
                | '\u{00AD}'
                | '\u{180E}'
                | '\u{200B}'
                | '\u{2060}'..='\u{2064}'
                | '\u{FEFF}'
                // tag characters and supplementary variation selectors: carry
                // hidden text ("ASCII smuggling")
                | '\u{E0000}'..='\u{E007F}'
                | '\u{E0100}'..='\u{E01EF}'
        )
}

/// Line and paragraph separators: shown as nothing, pasted as a line break.
fn is_line_separator(c: char) -> bool {
    c == '\u{2028}' || c == '\u{2029}'
}

/// `s` without hidden control characters.
pub fn clean(s: &str) -> String {
    s.chars()
        .filter(|&c| !is_hidden_control(c))
        .map(|c| if is_line_separator(c) { '\n' } else { c })
        .collect()
}

/// `clean`, and tab/newline become spaces: for one-line UI text such as the
/// file name or a link target in the status bar.
pub fn clean_inline(s: &str) -> String {
    s.chars()
        .filter(|&c| !is_hidden_control(c))
        .map(|c| {
            if c == '\n' || c == '\t' || is_line_separator(c) {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Whether `c` may be typed into the search query.  The query is plain text,
/// one line, matched literally; nothing in it is interpreted.
pub fn is_query_char(c: char) -> bool {
    !c.is_control() && !is_hidden_control(c) && !is_line_separator(c)
}

/// Clean every string in a parsed document, recursively.
pub fn clean_elements(elements: &mut [Element]) {
    for el in elements {
        match el {
            Element::Heading { text, .. } => *text = clean_inline(text),
            Element::Paragraph(spans) => clean_spans(spans),
            Element::CodeBlock { lang, code } => {
                *code = clean(code);
                if let Some(l) = lang {
                    *l = clean_inline(l);
                }
            }
            Element::BlockQuote(inner) => clean_elements(inner),
            Element::List { items, .. } => {
                for item in items {
                    clean_elements(item);
                }
            }
            Element::Table { headers, rows } => {
                for cell in headers.iter_mut().chain(rows.iter_mut().flatten()) {
                    *cell = clean_inline(cell);
                }
            }
            Element::Image { alt, src } => {
                *alt = clean_inline(alt);
                *src = clean_inline(src);
            }
            Element::Video { src } => *src = clean_inline(src),
            Element::HRule => {}
        }
    }
}

fn clean_spans(spans: &mut [Span]) {
    for span in spans {
        match span {
            Span::Text(t)
            | Span::Bold(t)
            | Span::Italic(t)
            | Span::BoldItalic(t)
            | Span::Code(t)
            | Span::Strikethrough(t) => *t = clean(t),
            Span::Link { text, href } => {
                *text = clean(text);
                *href = clean_inline(href);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_drops_controls_and_bidi_keeps_layout() {
        assert_eq!(
            clean("a\x1b[31mb\rc\x07\u{202E}d\u{2066}e\te\nf\u{200D}"),
            "a[31mbcde\te\nf\u{200D}"
        );
    }

    #[test]
    fn clean_drops_invisible_format_chars_keeps_emoji_joiners() {
        let hidden =
            "a\u{200B}b\u{200E}c\u{061C}d\u{00AD}e\u{FEFF}f\u{2060}g\u{E0041}\u{E0042}h\u{E0100}i";
        assert_eq!(clean(hidden), "abcdefghi");
        // ZWJ family emoji, ZWNJ (Persian), emoji variation selector survive.
        let keep = "\u{1F468}\u{200D}\u{1F469} \u{0645}\u{200C}\u{06CC} \u{2764}\u{FE0F}";
        assert_eq!(clean(keep), keep);
        assert_eq!(clean("a\u{2028}b\u{2029}c"), "a\nb\nc");
        assert_eq!(clean_inline("a\u{2028}b"), "a b");
    }

    #[test]
    fn clean_inline_is_one_line() {
        assert_eq!(clean_inline("a\tb\nc\x1b"), "a b c");
    }

    #[test]
    fn query_chars_are_printable_only() {
        for c in ['a', ' ', '/', '*', '[', '\\', '\u{e9}', '\u{4e2d}'] {
            assert!(is_query_char(c), "{c:?}");
        }
        for c in ['\x1b', '\n', '\t', '\x7f', '\u{9b}', '\u{202E}'] {
            assert!(!is_query_char(c), "{c:?}");
        }
    }

    #[test]
    fn clean_elements_reaches_nested_text() {
        let mut els = crate::parser::parse(
            "# T\x1bX\n\n- [l\u{202E}k](https://e.x/\x07a)\n\n> `c\x1bd`\n\n| h\x1b |\n|---|\n| v\x07 |\n",
        );
        clean_elements(&mut els);
        let dump = format!("{els:?}");
        assert!(
            !dump.contains("\\u{1b}") && !dump.contains("\\u{7}") && !dump.contains("\\u{202e}"),
            "{dump}"
        );
    }
}
