// Project: czech-tabloid-scraper
// File: src/main.rs
// Description: Command-line entry point.
// Author: Jan Alexandr Kopřiva
// License: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = czech_tabloid_scraper::http_client()?;
    czech_tabloid_scraper::run_in_current_dir(
        &client,
        czech_tabloid_scraper::FEEDS,
        std::io::stdout(),
    )?;
    Ok(())
}
