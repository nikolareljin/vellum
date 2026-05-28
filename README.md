# vellum

<p align="center">
  <img src="assets/img/logo.png" alt="vellum logo" width="128"/>
</p>

[![CI](https://github.com/nikolareljin/vellum/actions/workflows/ci.yml/badge.svg)](https://github.com/nikolareljin/vellum/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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

- Rust ≥ 1.79 (stable)
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

### Homebrew (macOS / Linux)

```bash
# Once a tap is published:
brew tap nikolareljin/vellum https://github.com/nikolareljin/homebrew-vellum
brew install vellum

# Or install directly from the local formula:
brew install --formula packaging/homebrew/vellum.rb
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
```

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
| `Enter` | Follow link / confirm search |
| `/` | Open incremental search |
| `n` / `N` | Next / previous match |
| `Esc` | Clear search |
| `q` / `Ctrl+C` | Quit |
| Mouse scroll | Scroll 3 lines |

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
