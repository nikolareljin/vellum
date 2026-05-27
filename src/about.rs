const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_page() {
    println!();
    println!("  \x1b[1;33mvellum\x1b[0m  v{}  — Rich Markdown viewer for the terminal", VERSION);
    println!();
    println!("  \x1b[1mAuthor\x1b[0m   Nik Reljin");
    println!("  \x1b[1mGitHub\x1b[0m   \x1b[4;36mhttps://github.com/nikolareljin\x1b[0m");
    println!("  \x1b[1mLinkedIn\x1b[0m \x1b[4;36mhttps://www.linkedin.com/in/nikolareljin\x1b[0m");
    println!();
    println!("  Source:  https://github.com/nikolareljin/vellum");
    println!("  License: MIT");
    println!();
}
