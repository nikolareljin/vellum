use clap::Parser;

mod about;
mod app;
mod highlight;
mod image;
mod links;
mod parser;
mod renderer;
mod search;
mod svg;
mod theme;
mod video;

#[derive(Parser, Debug)]
#[command(name = "vellum", about = "Rich Markdown viewer for the terminal")]
pub struct Cli {
    /// Markdown file to open
    #[arg(required_unless_present_any = &["page", "theme"])]
    pub file: Option<std::path::PathBuf>,

    /// Colour theme: dark (default), dracula, solarized, or a name from $XDG_CONFIG_HOME/vellum/themes/ (defaults to ~/.config/vellum/themes/)
    #[arg(short = 't', long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Open in code view (spawns $EDITOR / bat / less)
    #[arg(short, long)]
    pub code: bool,

    /// Show author and project info
    #[arg(long)]
    pub page: bool,
}

const THEME_PREVIEW_MD: &str = r#"
# Heading 1
## Heading 2
### Heading 3

Paragraph with `inline code`, **bold**, *italic*, ~~strikethrough~~,
and a [link](https://github.com).

---

> Blockquote line one.
> Blockquote line two.

- Item one
- Item two
  - Nested item
- Item three

| Column A | Column B | Column C |
|----------|----------|----------|
| alpha    | beta     | gamma    |
| one      | two      | three    |

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

```python
def greet(name: str) -> str:
    return f"Hello, {name}!"
```
"#;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.page {
        about::print_page();
        return Ok(());
    }

    if cli.code && cli.file.is_none() {
        anyhow::bail!("--code requires a file argument");
    }

    let theme = theme::Theme::load(cli.theme.as_deref())?;

    // When --theme is given without a file, show a built-in preview document.
    let mut _preview_tmp: Option<tempfile::NamedTempFile> = None;
    let file = match cli.file {
        Some(f) => f,
        None => {
            use std::io::Write;
            let mut tmp = tempfile::Builder::new()
                .prefix("vellum-preview-")
                .suffix(".md")
                .tempfile()?;
            tmp.write_all(THEME_PREVIEW_MD.as_bytes())?;
            let path = tmp.path().to_path_buf();
            _preview_tmp = Some(tmp); // keep alive until end of main
            path
        }
    };

    if cli.code {
        app::open_code_view(&file)?;
    } else {
        let mut history = app::NavHistory::new();
        let mut current = file.clone();

        loop {
            match app::run(&current, &history, &theme)? {
                app::NavAction::Quit => break,
                app::NavAction::GoTo(next) => {
                    history.push_back(current.clone());
                    current = next;
                }
                app::NavAction::Back => {
                    if let Some(prev) = history.go_back(current.clone()) {
                        current = prev;
                    }
                }
                app::NavAction::Forward => {
                    if let Some(next) = history.go_forward(current.clone()) {
                        current = next;
                    }
                }
            }
        }
    }

    Ok(())
}
