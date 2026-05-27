# vellum Screenshots

> **Note:** Screenshots will be added after the first interactive session.
> Run `vellum README.md` locally to see the TUI live.

## UI Layout (text mockup)

```
┌──────────────────────────────────────────────────────────────────────────┐
│# vellum — Demo                                                           │
│                                                                          │
│Rich Markdown viewer for the terminal. Bold text, italic, ~~strike~~,     │
│and `inline code` all rendered with full styling.                         │
│                                                                          │
│## Features                                                               │
│                                                                          │
│• Syntax-highlighted code blocks                                          │
│• Tables with alignment                                                   │
│• Block quotes                                                            │
│• Inline images (Kitty/Sixel/iTerm2)                                      │
│• Clickable links with Tab navigation                                     │
│                                                                          │
│────────────────────────────────────────────────────────────────────────  │
│                                                                          │
│## Code Example                                                           │
│                                                                          │
│ rust                                                                     │
│  fn main() -> anyhow::Result<()> {                                      │
│      let source = std::fs::read_to_string("README.md")?;                │
│      let elements = vellum::parser::parse(&source);                     │
│      println!("Parsed {} elements", elements.len());                    │
│      Ok(())                                                              │
│  }                                                                       │
│────────────────────────────────────────────────────────────────────────  │
├──────────────────────────────────────────────────────────────────────────┤
│ vellum_demo.md   line 1/48 (12%)  │  j/k  g/G  Tab  /search  e  q       │
└──────────────────────────────────────────────────────────────────────────┘
```

## Key Commands Visible in Status Bar

| Mode | Status Bar |
|------|-----------|
| Normal | `filename │ line X/Y (N%) │ j/k g/G Tab /search e code q quit` |
| Search | `/ query_` |
| Match found | `[1/3] "query" │ n/N next/prev Esc clear` |
