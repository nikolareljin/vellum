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
    #[arg(required_unless_present = "page")]
    pub file: Option<std::path::PathBuf>,

    /// Colour theme: dark (default), dracula, solarized, or a name from ~/.config/vellum/themes/
    #[arg(short = 't', long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Open in code view (spawns $EDITOR / bat / less)
    #[arg(short, long)]
    pub code: bool,

    /// Show author and project info
    #[arg(long)]
    pub page: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.page {
        about::print_page();
        return Ok(());
    }

    let theme = theme::Theme::load(cli.theme.as_deref())?;
    let file = cli.file.expect("file required when --page not set");

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
