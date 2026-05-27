# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

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
