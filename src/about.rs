const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_page() {
    println!();
    println!("  \x1b[1;33m  vellum  \x1b[0m  v{}", VERSION);
    println!("  Rich Markdown viewer for the terminal");
    println!();
    println!("  ─────────────────────────────────────────");
    println!();
    println!("  \x1b[1mAuthor\x1b[0m    Nikola Reljin");
    println!("  \x1b[1mGitHub\x1b[0m    \x1b[4;36mhttps://github.com/nikolareljin\x1b[0m");
    println!("  \x1b[1mLinkedIn\x1b[0m  \x1b[4;36mhttps://www.linkedin.com/in/nikolareljin\x1b[0m");
    println!();
    println!("  ─────────────────────────────────────────");
    println!();
    println!("  \x1b[1mSource\x1b[0m    \x1b[4;36mhttps://github.com/nikolareljin/vellum\x1b[0m");
    println!("  \x1b[1mLicense\x1b[0m   MIT");
    println!();
}
