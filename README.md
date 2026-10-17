# czech-tabloid-scraper

Collects headlines from 18 Czech news and tabloid feeds.

[![ci](https://github.com/koprjaa/czech-tabloid-scraper/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/czech-tabloid-scraper/actions/workflows/ci.yml)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)

It downloads the RSS and Atom feeds of Czech news, tabloid and tech sites (Blesk, Novinky, iDNES, Root.cz and others) and writes all headlines to one UTF-8 text file. Navigation labels and very short titles are filtered out. The source list is the `FEEDS` constant in `src/lib.rs`.

## Install

Needs a stable Rust toolchain.

```bash
cargo build --release
```bash
cargo install --path .
```

## Use

```bash
bulvar_scraper
```

The command takes no arguments. Feeds are fetched one by one with a 15-second timeout. A failed feed is reported and skipped.

Output goes to `scraped_output/titles_YYYYMMDD_HHMMSS.txt` in the current working directory, one title per line. Older `titles_*.txt` files in that folder are deleted after a new one is written.

## License

[MIT](LICENSE)
