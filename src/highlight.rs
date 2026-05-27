use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::theme::CodeColors;

/// One rendered line = list of (syntect Style, owned text) pairs.
pub type StyledLine = Vec<(Style, String)>;

thread_local! {
    static SS: SyntaxSet = SyntaxSet::load_defaults_nonewlines();
    static TS: ThemeSet = ThemeSet::load_defaults();
}

/// Highlight `code` with the given language hint.
///
/// The syntect theme is chosen per language via `code_colors.by_language`;
/// if no per-language override exists the `code_colors.default_theme` is used.
/// Falls back to plain text for unknown / absent languages, and to
/// `base16-ocean.dark` when the requested syntect theme name is not found.
pub fn highlight_code(code: &str, lang: Option<&str>, code_colors: &CodeColors) -> Vec<StyledLine> {
    SS.with(|ss| {
        TS.with(|ts| {
            let syntax = lang
                .and_then(|l| ss.find_syntax_by_token(l))
                .unwrap_or_else(|| ss.find_syntax_plain_text());

            // Per-language override → default_theme → hard fallback
            let theme_name = lang
                .and_then(|l| code_colors.by_language.get(l).map(String::as_str))
                .unwrap_or(&code_colors.default_theme);

            let theme = ts
                .themes
                .get(theme_name)
                .or_else(|| ts.themes.get("base16-ocean.dark"))
                .expect("base16-ocean.dark is always present in syntect defaults");

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
