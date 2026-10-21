// Project: czech-tabloid-scraper
// File: src/main.rs
// Description: Command-line entry point.
// Author: Jan Alexandr Kopřiva
// License: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = czech_tabloid_scraper::http_client()?;
    // Nothing saved (every feed failed, or the write did) must not look like success.
    if !czech_tabloid_scraper::run_in_current_dir(
        &client,
        czech_tabloid_scraper::FEEDS,
        std::io::stdout(),
    )? {
        std::process::exit(1);
    }
    Ok(())
}
