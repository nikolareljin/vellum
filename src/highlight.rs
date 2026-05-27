use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// One rendered line = list of (syntect Style, owned text) pairs.
pub type StyledLine = Vec<(Style, String)>;

thread_local! {
    static SS: SyntaxSet = SyntaxSet::load_defaults_nonewlines();
    static TS: ThemeSet = ThemeSet::load_defaults();
}

/// Highlight `code` with the given language hint.
/// Returns one `StyledLine` per source line.
/// Falls back to plain text for unknown / absent language.
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
