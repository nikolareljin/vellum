# vellum — Roadmap

## Phase 1 — Core Viewer ✅ (0.1.0)

Full Markdown text rendering in a scrollable TUI. All standard block and inline elements. Syntax-highlighted code blocks. Mouse support. Code-view mode.

## Phase 2 — Inline Images (0.2.0)

Render image references directly in the terminal using `ratatui-image`.  
Auto-detects protocol: **Kitty Graphics** → **Sixel** → **iTerm2** → halfblock fallback.  
Local paths supported; remote URLs fetched on demand.

## Phase 3 — Link Navigation (0.3.0)

- External links open in system browser via `xdg-open` / `open`
- OSC 8 hyperlinks for terminals that support them (kitty, foot, WezTerm, iTerm2)
- `Tab` / `Shift+Tab` cycle through links in the document
- `Enter` follows the focused link; `#anchor` links jump to heading offset

## Phase 4 — Video Thumbnails (0.4.0)

For `<video>` HTML or Markdown links ending in `.mp4` / `.webm` / `.mov`:  
- Extract first frame via `ffmpeg -vframes 1`  
- Display as inline image using Phase 2 pipeline  
- Cache thumbnails in `/tmp/vellum/` for session lifetime

## Phase 5 — Code View Polish (included in 0.1.0+)

- `--code` flag and `e` key already wired
- Detect `bat --paging=always` for paginated, syntax-highlighted code view
- Return to TUI seamlessly after editor exits

## Phase 6 — Polish (0.5.0)

- `/` key opens incremental search; `n` / `N` cycle matches
- Match highlighting in rendered lines
- Config file (`~/.config/vellum/config.toml`) for theme, keybindings
- `--theme` CLI flag (themes from syntect's built-in set)
