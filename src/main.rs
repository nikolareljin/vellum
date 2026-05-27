use clap::Parser;

mod about;
mod app;
mod highlight;
mod image;
mod links;
mod parser;
mod renderer;

#[derive(Parser, Debug)]
#[command(name = "vellum", about = "Rich Markdown viewer for the terminal")]
pub struct Cli {
    /// Markdown file to open
    #[arg(required_unless_present = "page")]
    pub file: Option<std::path::PathBuf>,

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

    let file = cli.file.expect("file required when --page not set");
    if cli.code {
        app::open_code_view(&file)?;
    } else {
        app::run(&file)?;
    }

    Ok(())
}
