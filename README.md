# vellum

<p align="center">
  <img src="assets/img/logo.png" alt="vellum logo" width="128"/>
</p>

[![CI](https://github.com/nikolareljin/vellum/actions/workflows/ci.yml/badge.svg)](https://github.com/nikolareljin/vellum/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

User guide: https://nikolareljin.github.io/vellum/ (install, usage, keys, troubleshooting).

Rich Markdown viewer for the terminal. Renders headings, paragraphs, **bold/italic/strikethrough**, `inline code`, fenced code blocks with syntax highlighting, tables, lists, block quotes, images, and clickable links — all in a full TUI, without leaving your shell.

```
vellum README.md
```

<p align="center">
  <img src="docs/screenshots/demo.gif" alt="vellum demo" width="800"/>
</p>

## Features

| Feature | Status |
|---------|--------|
| Headings (H1–H6, colour-coded) | ✅ Phase 1 |
| Paragraphs with inline styles | ✅ Phase 1 |
| Fenced code blocks + syntax highlighting | ✅ Phase 1 |
| Tables | ✅ Phase 1 |
| Ordered & unordered lists | ✅ Phase 1 |
| Block quotes | ✅ Phase 1 |
| Horizontal rules | ✅ Phase 1 |
| Mouse scroll | ✅ Phase 1 |
| Code-view mode (`e` key / `--code`) | ✅ Phase 1 |
| `--page` author info | ✅ Phase 1 |
| Inline images (Kitty/Sixel/iTerm2) | ✅ Phase 2 |
| Link navigation (Tab/Enter/anchor jumps) | ✅ Phase 3 |
| Video thumbnails (ffmpeg) | ✅ Phase 4 |
| In-document search (`/`) | ✅ Phase 6 |

## Requirements

- Rust 1.90 or newer (stable)
- `ffmpeg` (optional — required for video thumbnails, Phase 4+)
- Terminal supporting Kitty Graphics / Sixel / iTerm2 (optional — for inline images, Phase 2+)

## Install

### Script (quickest — installs to `~/.local/bin`)

```bash
git clone https://github.com/nikolareljin/vellum
cd vellum
./setup          # install ffmpeg + rustfmt/clippy (one-time)
./install        # builds release binary + copies to ~/.local/bin

# Upgrade
./install        # re-run any time to build + overwrite

# Uninstall
./install --uninstall

# Custom prefix (e.g. /usr/local)
./install --prefix /usr/local   # may need sudo for /usr/local/bin
```

> `~/.local/bin` must be in your `PATH`. If not, add to `~/.bashrc` / `~/.zshrc`:
> ```bash
> export PATH="$HOME/.local/bin:$PATH"
> ```

### Cargo (quickest one-liner, any platform with Rust)

```bash
cargo install --locked --git https://github.com/nikolareljin/vellum --bin vellum
```

### Homebrew (macOS / Linux)

```bash
brew tap nikolareljin/tap
brew install vellum
```

### DEB package (Debian / Ubuntu)

```bash
# Download from Releases, then:
sudo apt install ./vellum_0.6.0_amd64.deb

# Or build locally:
cargo install cargo-deb --locked && cargo deb
sudo apt install ./target/debian/vellum_0.6.0_amd64.deb
```

### From source

```bash
cargo install --path .
```

Or grab a pre-built binary from [Releases](https://github.com/nikolareljin/vellum/releases).

See [`docs/packaging.md`](docs/packaging.md) for full packaging details.

## Usage

```bash
vellum <file.md>              # rich TUI viewer (default)
vellum --code <file.md>       # open in $EDITOR / bat / less
vellum --page                 # show author info
vellum --theme dracula        # preview a theme (no file required)
vellum --theme dracula <file> # open file with a specific theme
vellum --remote-images <file> # also fetch https images (off by default)
```

## Remote images

Opening a document makes no network requests. A remote image shows as
`[remote image not loaded: host]`. With `--remote-images` (or
`VELLUM_REMOTE_IMAGES=1`) vellum fetches them, https only, never from a
private, loopback or link-local address (redirects included), with a 10 s
timeout, a 20 MiB cap and a pixel-count limit. `VELLUM_NO_REMOTE_IMAGES=1`
always turns fetching off. SVG images never load files they reference;
only inline `data:` images inside an SVG are drawn.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down 1 line |
| `k` / `↑` | Scroll up 1 line |
| `PgDn` / `Ctrl+F` | Page down |
| `PgUp` / `Ctrl+B` | Page up |
| `g` / `Home` | Top of document |
| `G` / `End` | Bottom of document |
| `a` | About page |
| `e` | Code view (spawns `$EDITOR` / `bat` / `less`) |
| `Tab` / `Shift+Tab` | Cycle links |
| `c` or `]` / `C` or `[` | Select next / previous code block (headers are numbered `1/3`, `2/3`, ...) |
| `y` / `Enter` | Copy the selected code block to the clipboard |
| `Enter` | Follow link / confirm search |
| `/` | Search the document text (plain text, case-insensitive; `Enter` runs it) |
| `n` / `N` | Next / previous match (current match yellow, others cyan) |
| `Esc` | Clear search, deselect link or code block |
| `q` / `Ctrl+C` | Quit |
| Mouse scroll | Scroll 3 lines |
| Click a code block | Select it; click its `[ y: copy ]` label to copy |

Copying is two steps, select then confirm, so a stray key or click never
overwrites the clipboard. The last copied block stays marked `[copied]`.
Locally vellum uses `wl-copy`, `xclip`, `xsel`, `pbcopy` or `clip.exe` when
present, else the system clipboard directly (nothing to install; on Linux a
small `vellum --clipboard-daemon` process keeps the text available after vellum
exits, until something else is copied), else the OSC 52 escape sequence. Over SSH it always uses OSC 52, so
the text lands in the clipboard of the machine you type at. The terminal must
allow OSC 52 (kitty, WezTerm, iTerm2, foot, Windows Terminal do; xterm needs
`allowWindowOps`). In tmux set `set -g set-clipboard on` or
`set -g allow-passthrough on`. Terminals that ignore OSC 52 (GNOME Terminal,
xfce4-terminal and other VTE terminals) get an error naming what to install
instead of a copy that silently did not happen. OSC 52 copies are limited to about 73 KB of
code; vellum says so instead of copying part of a block.

## Development

```bash
./setup          # install system deps + rustfmt/clippy + init submodules
./build          # cargo build --release
./test           # cargo test --verbose
./lint           # cargo fmt --check + cargo clippy -D warnings
./lint -f        # auto-fix formatting and clippy suggestions
./run [file]     # build + run (defaults to README.md)
./update         # update git submodules to latest production
```

## About

```bash
vellum --page
```

---

**Author:** [Nik Reljin](https://github.com/nikolareljin) · [LinkedIn](https://www.linkedin.com/in/nikolareljin)  
**License:** MIT

---

## Clone traffic

![Clone traffic](https://raw.githubusercontent.com/nikolareljin/stats/main/charts/vellum.svg)

_Updated daily. Total and unique cloners over the last 14 days._
