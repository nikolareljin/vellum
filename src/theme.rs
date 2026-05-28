use std::collections::HashMap;
use std::path::PathBuf;

// ── Colour primitive ──────────────────────────────────────────────────────────

/// An RGB colour deserialised from a `"#rrggbb"` hex string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Convert to ratatui's `Color::Rgb`.
    #[inline]
    pub fn to_color(self) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(self.0, self.1, self.2)
    }
}

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let s = s.trim();
        if s.len() != 7 || !s.starts_with('#') {
            return Err(format!("expected \"#rrggbb\", got {:?}", s));
        }
        let r = u8::from_str_radix(&s[1..3], 16)
            .map_err(|_| format!("invalid red component in {:?}", s))?;
        let g = u8::from_str_radix(&s[3..5], 16)
            .map_err(|_| format!("invalid green component in {:?}", s))?;
        let b = u8::from_str_radix(&s[5..7], 16)
            .map_err(|_| format!("invalid blue component in {:?}", s))?;
        Ok(Rgb(r, g, b))
    }
}

// ── Theme sub-structs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct HeadingColors {
    pub h1: Rgb,
    pub h2: Rgb,
    pub h3: Rgb,
    pub h4: Rgb,
    pub h5: Rgb,
    pub h6: Rgb,
}

impl Default for HeadingColors {
    fn default() -> Self {
        Self {
            h1: Rgb(255, 215, 0),   // gold
            h2: Rgb(100, 200, 255), // sky blue
            h3: Rgb(100, 220, 120), // seafoam
            h4: Rgb(220, 130, 255), // lilac
            h5: Rgb(100, 180, 255), // steel blue
            h6: Rgb(180, 180, 180), // silver
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct CodeColors {
    pub bg: Rgb,
    pub header_bg: Rgb,
    pub label_fg: Rgb,
    pub default_theme: String,
    pub by_language: HashMap<String, String>,
}

impl Default for CodeColors {
    fn default() -> Self {
        Self {
            bg: Rgb(30, 30, 36),
            header_bg: Rgb(45, 45, 55),
            label_fg: Rgb(200, 200, 200),
            default_theme: "base16-ocean.dark".into(),
            by_language: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct InlineColors {
    pub code_fg: Rgb,
    pub code_bg: Rgb,
    pub link: Rgb,
    pub strikethrough: Rgb,
}

impl Default for InlineColors {
    fn default() -> Self {
        Self {
            code_fg: Rgb(250, 200, 100), // amber
            code_bg: Rgb(40, 40, 47),
            link: Rgb(80, 190, 255),
            strikethrough: Rgb(120, 120, 120),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct BlockColors {
    pub blockquote: Rgb,
    pub hrule: Rgb,
    pub table_border: Rgb,
    pub table_header: Rgb,
    pub list_bullet: Rgb,
    pub image_icon: Rgb,
    pub image_alt: Rgb,
}

impl Default for BlockColors {
    fn default() -> Self {
        Self {
            blockquote: Rgb(100, 180, 255),
            hrule: Rgb(80, 80, 80),
            table_border: Rgb(80, 80, 100),
            table_header: Rgb(255, 215, 0),
            list_bullet: Rgb(100, 200, 255),
            image_icon: Rgb(200, 120, 255),
            image_alt: Rgb(160, 160, 160),
        }
    }
}

// ── Top-level Theme ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct Theme {
    pub headings: HeadingColors,
    pub code: CodeColors,
    pub inline: InlineColors,
    pub blocks: BlockColors,
}

// ── Built-in theme JSON blobs ─────────────────────────────────────────────────

const DARK_JSON: &str = include_str!("../themes/dark.json");
const DRACULA_JSON: &str = include_str!("../themes/dracula.json");
const SOLARIZED_JSON: &str = include_str!("../themes/solarized.json");

fn builtin(name: &str) -> Option<&'static str> {
    match name {
        "dark" => Some(DARK_JSON),
        "dracula" => Some(DRACULA_JSON),
        "solarized" => Some(SOLARIZED_JSON),
        _ => None,
    }
}

// ── Loading ───────────────────────────────────────────────────────────────────

fn is_safe_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Path to the user's theme override directory.
/// Respects `$XDG_CONFIG_HOME`; falls back to `$HOME/.config`.
fn user_theme_path(name: &str) -> Option<PathBuf> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(
        config_dir
            .join("vellum")
            .join("themes")
            .join(format!("{}.json", name)),
    )
}

/// Parse a JSON string into a `Theme`; missing fields use Rust `Default` values.
fn parse_json(json: &str, label: &str) -> anyhow::Result<Theme> {
    serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse theme '{}': {}", label, e))
}

impl Theme {
    /// Load a theme by name.
    ///
    /// Resolution order:
    /// 1. `~/.config/vellum/themes/<name>.json` (user override)
    /// 2. Built-in themes: `dark`, `dracula`, `solarized`
    ///
    /// If `name` is `None`, returns the built-in `dark` theme (no I/O).
    pub fn load(name: Option<&str>) -> anyhow::Result<Self> {
        let Some(name) = name else {
            return parse_json(DARK_JSON, "dark");
        };
        if !is_safe_theme_name(name) {
            anyhow::bail!(
                "invalid theme name '{}'; use only ASCII letters, numbers, '-' or '_'",
                name
            );
        }

        // 1. User directory
        let user_path = user_theme_path(name);
        if let Some(path) = user_path.as_ref().filter(|path| path.is_file()) {
            let json = std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("cannot read '{}': {}", path.display(), e))?;
            return parse_json(&json, name);
        }

        // 2. Built-ins
        if let Some(json) = builtin(name) {
            return parse_json(json, name);
        }

        let user_location = user_path
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "$XDG_CONFIG_HOME/vellum/themes/<name>.json".to_string());
        anyhow::bail!(
            "theme '{}' not found; looked in {} and built-ins (dark, dracula, solarized)",
            name,
            user_location
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_loads() {
        let t = Theme::load(None).expect("dark theme should load");
        let Rgb(r, g, b) = t.headings.h1;
        assert_eq!((r, g, b), (255, 215, 0), "H1 should be gold");
    }

    #[test]
    fn dracula_theme_loads() {
        // Parse the embedded JSON directly — avoids user $XDG_CONFIG_HOME lookup.
        let t: Theme = parse_json(DRACULA_JSON, "dracula").expect("dracula theme should parse");
        let Rgb(r, g, b) = t.headings.h1;
        assert_eq!((r, g, b), (255, 121, 198), "Dracula H1 = #ff79c6");
    }

    #[test]
    fn solarized_theme_loads() {
        let t: Theme =
            parse_json(SOLARIZED_JSON, "solarized").expect("solarized theme should parse");
        let Rgb(r, g, b) = t.headings.h1;
        assert_eq!((r, g, b), (181, 137, 0), "Solarized H1 = #b58900");
    }

    #[test]
    fn unknown_theme_errors() {
        // Test the built-in lookup directly — no filesystem I/O, no $HOME dependency.
        assert!(builtin("nonexistent").is_none(), "nonexistent should not be a built-in");
    }

    #[test]
    fn default_matches_builtin_dark() {
        assert_eq!(Theme::default(), Theme::load(None).unwrap());
    }

    #[test]
    fn unsafe_theme_names_are_rejected() {
        assert!(Theme::load(Some("../dark")).is_err());
        assert!(Theme::load(Some("dark/theme")).is_err());
    }

    #[test]
    fn partial_json_fills_defaults() {
        // Only headings.h1 overridden — rest should be dark defaults
        let json = r##"{"headings": {"h1": "#ff0000"}}"##;
        let t: Theme = serde_json::from_str(json).expect("partial JSON should parse");
        let Rgb(r, g, b) = t.headings.h1;
        assert_eq!((r, g, b), (255, 0, 0));
        // h2 falls back to dark default
        let Rgb(r2, g2, b2) = t.headings.h2;
        assert_eq!((r2, g2, b2), (100, 200, 255));
    }

    #[test]
    fn rgb_parse_valid() {
        let rgb: Rgb = "#ffd700".to_string().try_into().unwrap();
        assert_eq!((rgb.0, rgb.1, rgb.2), (255, 215, 0));
    }

    #[test]
    fn rgb_parse_invalid() {
        let r: Result<Rgb, _> = "#gg0000".to_string().try_into();
        assert!(r.is_err());
    }

    #[test]
    fn rgb_parse_wrong_format() {
        let r: Result<Rgb, _> = "ffd700".to_string().try_into();
        assert!(r.is_err());
    }
}
