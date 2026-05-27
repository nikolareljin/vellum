# vellum demo

> **Rich Markdown in your terminal** — no browser required.

## Features

| Feature                           | Status |
|-----------------------------------|--------|
| Headings (H1–H6, colour-coded)    | ✅     |
| Bold, italic, ~~strikethrough~~   | ✅     |
| `inline code` and code blocks     | ✅     |
| Tables                            | ✅     |
| Ordered & unordered lists         | ✅     |
| Inline images (Kitty/Sixel/halfblock) | ✅  |
| Clickable links + anchor jumps    | ✅     |
| In-document search (`/`)          | ✅     |
| Previous / Next history           | ✅     |

---

## Quick start

```bash
cargo install vellum
vellum README.md
```

## Keyboard shortcuts

- **j / k** — scroll down / up
- **g / G** — top / bottom
- **Tab** — cycle links
- **Enter** — follow link
- **/** — search
- **Alt+Left / Alt+Right** — back / forward history
- **e** — open in $EDITOR
- **q** — quit

---

## Code example

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    app::run(&cli.file, &mut NavHistory::new())?;
    Ok(())
}
```

## Links

- [README](../README.md) — project overview
- [Architecture](architecture.md) — component design
- [Roadmap](roadmap.md) — planned features
