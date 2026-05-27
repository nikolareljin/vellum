# vellum — Architecture

## Overview

```
┌─────────────────────────────────────────────────────┐
│  CLI (clap)  ─►  main.rs                           │
│                    │                                │
│            ┌───────┴────────┐                       │
│            │                │                       │
│         app::run()    app::open_code_view()         │
│            │                │                       │
│     ┌──────▼──────┐   spawn $EDITOR/bat/less        │
│     │  parser.rs  │                                 │
│     │  pulldown-  │                                 │
│     │  cmark      │                                 │
│     └──────┬──────┘                                 │
│            │  Vec<Element>                          │
│     ┌──────▼──────┐                                 │
│     │ renderer.rs │◄── highlight.rs (syntect)       │
│     └──────┬──────┘     ◄── image.rs (Phase 2)     │
│            │  Vec<Line<'static>>                    │
│     ┌──────▼──────┐                                 │
│     │  app.rs     │  ratatui event loop             │
│     │  TUI draw   │  crossterm backend              │
│     └─────────────┘                                 │
└─────────────────────────────────────────────────────┘
```

## Module Responsibilities

| Module | Role |
|--------|------|
| `main.rs` | CLI entry point — `clap` arg parse, dispatch to `app` or `about` |
| `app.rs` | `App` state, ratatui draw loop, crossterm events, scrolling, mouse |
| `parser.rs` | `pulldown-cmark` event stream → `Vec<Element>` tree |
| `renderer.rs` | `Vec<Element>` → `Vec<Line<'static>>` (ratatui text model) |
| `highlight.rs` | `syntect` wrapper — syntax-highlight code blocks to styled spans |
| `about.rs` | `--page` flag — print author/project info with ANSI colour |
| `image.rs` | *(Phase 2)* `ratatui-image` inline image rendering |
| `video.rs` | *(Phase 4)* `ffmpeg` subprocess → first-frame PNG thumbnail |
| `links.rs` | *(Phase 3)* OSC 8 external links + heading anchor offset map |
| `search.rs` | *(Phase 6)* in-document text search with match navigation |

## Data Flow

```
Markdown text (String)
    │
    ▼  parser::parse()
Vec<Element>          ← block-level AST (Heading, Paragraph, CodeBlock, …)
    │
    ▼  renderer::render_elements()
Vec<Line<'static>>    ← flat list of styled ratatui lines
    │
    ▼  app: scroll window
Vec<Line>  (viewport slice)
    │
    ▼  ratatui::Paragraph widget → terminal
```

## Element Types

```rust
pub enum Element {
    Heading { level: u8, text: String },
    Paragraph(Vec<Span>),
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote(Vec<Element>),
    List { ordered: bool, items: Vec<Vec<Element>> },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Image { alt: String, src: String },
    Video { src: String },
    HRule,
    Break,
}
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down 1 |
| `k` / `↑` | Scroll up 1 |
| `PgDn` / `Ctrl+F` | Page down |
| `PgUp` / `Ctrl+B` | Page up |
| `g` / `Home` | Top |
| `G` / `End` | Bottom |
| `e` | Code view |
| `q` / `Ctrl+C` | Quit |
| Mouse scroll | ±3 lines |

## CI / Release

- **ci-helpers@production** reusable workflows
- `rust.yml` — build + test on push/PR
- `rust-scan.yml` — fmt + clippy + `cargo audit`
- `release.yml` — multi-platform binaries on `*.*.*` tags (no `v` prefix)
