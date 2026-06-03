# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.6.2] — 2026-06-03

### Fixed
- Long text lines (paragraphs, list items, blockquotes, headings) now word-wrap at the terminal edge instead of overflowing invisibly to the right. Leading indentation (code-block padding, list bullets, heading prefixes) is preserved on the first line; hard-break fallback handles tokens wider than the terminal width (closes #9).
- Images no longer insert a blank separator row between themselves and the following text; content appears immediately below with no extra gap (closes #9).
- Image slot height is now `min(⌈img_h_px / cell_h_px⌉, ⌈img_h_px × cols / (img_w_px × 2)⌉)`, clamped to `[1, max_h]`. `cell_h_px` is the actual terminal row height in pixels from `TIOCGWINSZ` (queried before raw mode; falls back to 16 px). This matches `Resize::Fit`'s no-upscale behaviour and correctly handles images wider than the terminal (closes #9).

## [0.6.1] — 2026-05-31

### Added
- `ureq` dependency for fetching remote images over HTTP/HTTPS
- `src/image.rs` `load_image_url`: fetches and decodes images from URLs; SVG URLs rasterised via `resvg`; 10 s timeout; 20 MiB download cap; 50 MP pixel-count guard
- `src/image.rs` `is_remote_url`: case-insensitive URI scheme check (`http://`/`https://`) per RFC 3986 §3.1
- `src/parser.rs` `html_attr` / `extract_img_tag`: safe HTML attribute extraction (no shell, no `eval`)
- `VELLUM_NO_REMOTE_IMAGES` environment variable: when set, remote image fetches are blocked without any network connection (opt-out for privacy/security-conscious users)

### Changed
- Image slot height is now proportional to the terminal width (`cols/4` rows, clamped to `[12, min(cols/2, rows-2)]`) so images render at ≥ half the terminal width instead of the previous fixed 10-row slot

### Fixed
- `<img src="…">` HTML tags in Markdown files are now rendered in the TUI; both block-level (`Event::Html`) and inline (`Event::InlineHtml`) variants are handled (closes #6)

## [0.6.0] — 2026-05-27

### Added
- JSON theme engine: `--theme <name>` CLI flag selects a full colour scheme
- Three built-in themes: `dark` (default), `dracula`, `solarized`
- User themes at `~/.config/vellum/themes/<name>.json` override built-ins
- Per-language syntect theme override via `code.by_language` in theme JSON
- `src/theme.rs`: `Theme`, `HeadingColors`, `CodeColors`, `InlineColors`, `BlockColors`, `Rgb`
- Partial JSON supported — missing fields fill from dark-theme defaults via `#[serde(default)]`
- In-app About page available from the `a` shortcut

### Changed
- `renderer.rs`: rendered Markdown colours now come from theme values
- `highlight.rs`: `highlight_code()` now accepts `&CodeColors` for per-language theme selection
- `app.rs`, `main.rs`: theme threaded through rendering pipeline

## [0.5.0] — 2026-05-27

### Added
- In-document search: `/` opens incremental search, `Enter` confirms, `n`/`N` cycle matches, `Esc` clears
- Search status bar showing current match index and query string
- `search.rs` module with case-insensitive line search

## [0.4.0] — 2026-05-27

### Added
- Video thumbnail extraction via `ffmpeg` (`src/video.rs`)
- `Element::Video` rendered as inline image using Phase 2 pipeline
- Thumbnail temp files cached for session lifetime

### Security
- Fixed argument injection (flag smuggling) in `open_url`: scheme allowlist + `--` separator for `xdg-open`

## [0.3.0] — 2026-05-27

### Added
- Link navigation: `Tab`/`Shift+Tab` cycle through links, `Enter` follows
- External links open in system browser via `xdg-open`/`open`
- `#anchor` links jump to heading offset in rendered document
- `links.rs` module with anchor map builder and URL opener

## [0.2.0] — 2026-05-27

### Added
- Inline image rendering via `ratatui-image` (Kitty → Sixel → iTerm2 → halfblock auto-detect)
- `image.rs` module with `ImageCache` for session-scoped image loading
- `DisplayLine` enum separating text lines from image slots in draw loop

## [0.1.0] — 2026-05-27

### Added
- Core Markdown TUI viewer with full scrolling (ratatui + crossterm)
- Parser: headings (H1–H6), paragraphs, code blocks, block quotes, lists, tables, horizontal rules, images (placeholder), videos (placeholder)
- Inline span rendering: bold, italic, bold+italic, strikethrough, inline code, links
- Syntax highlighting for fenced code blocks via syntect (`base16-ocean.dark` theme)
- Status bar showing filename, line position, and percentage
- Mouse scroll wheel support (3 lines per tick)
- Code-view mode: `e` key or `--code` flag spawns `$EDITOR` / `bat` / `less`, returns to TUI on exit
- `--page` flag prints author info (GitHub, LinkedIn) without entering TUI
- `rust-toolchain.toml` pinning stable Rust channel
- CI scaffold via ci-helpers@production: `rust.yml`, `rust-scan.yml`, `release.yml`
- `scripts/` suite (build, test, lint, run, setup, update) via script-helpers submodule
