# czech-tabloid-scraper

Reads 18 Czech news, tabloid, and tech RSS feeds and writes headlines to one UTF-8 text file.

![rust](https://img.shields.io/badge/rust-stable-DEA584?style=flat-square&logo=rust&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
[![ci](https://github.com/koprjaa/czech-tabloid-scraper/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/czech-tabloid-scraper/actions/workflows/ci.yml)

## Install

Install a stable Rust toolchain, then run:

```bash
cargo install --path .
```

## Use

```bash
bulvar_scraper
```

There are no switches; arguments are ignored. Feeds are downloaded sequentially with the original browser User-Agent, no application retries, and 15-second connection and read inactivity timeouts. A failed download prints `✗ Failed to fetch <name>` and processing continues. Handled download and save failures exit successfully, as before.

The file lands in `scraped_output/titles_YYYYMMDD_HHMMSS.txt` relative to the current working directory. Each title has a trailing newline. Files are UTF-8 without a BOM and use the platform's line endings. Only older `titles_*.txt` snapshots directly inside that directory are removed; other files and directories are retained. No output is written when no titles survive.

## How it works

- **Encoding.** Encoding is detected for each downloaded feed. Direct byte parsing also supports XML encoding declarations, including Windows-1250.
- **RSS and Atom.** Unnamespaced RSS items take precedence over Atom entries. Entry and title names are matched by local name. Only the first immediate title and its initial text are used. Order and duplicates are retained.
- **Navigation filter.** Titles shorter than 20 Unicode characters are removed. Unicode uppercase titles shorter than 50 characters are removed. ASCII uppercase menu labels are removed at every length. ASCII breadcrumbs consist of one capitalized word and at most ten lowercase trailing words; Czech diacritics bypass this breadcrumb rule. Unicode character properties follow the Rust toolchain and regex Unicode tables, which may be newer than the original interpreter.
- **CDATA and whitespace.** Single-line literal CDATA wrappers are stripped, and Unicode whitespace is collapsed.
- **Untrusted XML.** Malformed XML and DTDs are rejected without stopping other feeds.

The application has no configuration files or custom environment variables. Standard HTTP proxy environment variables are supported by the HTTP client. Edit `FEEDS` in `src/lib.rs` to change the source list.

## Sources

Super.cz, Blesk.cz, Extra.cz, Ahaonline.cz, Novinky.cz, iDnes.cz, Proženy.cz, Živě.cz, Doupě.cz, Živé.sk (najnovšie and mobilmania), Lupa.cz, Root.cz, Reflex.cz, Respekt.cz, České noviny, iRozhlas, and ČT24.

## Changes from the Python version

- The command is `bulvar_scraper` instead of invoking a script. Output and old-snapshot cleanup now use `scraped_output/` relative to the current working directory instead of the script directory. An installed binary therefore writes to the directory from which it is invoked, even when the executable lives in `~/.cargo/bin`.
- The console retains artwork and message text, but uses a static final progress line and a simple Success border. Rich animation, colors, adaptive panel widths and elapsed time are not reproduced. OS error text follows Rust and the host OS.
- The connection and per-read inactivity timeouts remain 15 seconds, with no total deadline or retries. Redirects remain limited to 30. TLS uses rustls instead of the Python/OpenSSL trust configuration. Client defaults for transport headers, compression, proxy discovery, TLS and certificate environment variables can differ; Python-specific `REQUESTS_CA_BUNDLE`, `CURL_CA_BUNDLE`, and `.netrc`/`NETRC` authentication settings are not read. Cookie validation and HTTP parser edge cases follow reqwest. SOCKS proxies and Brotli decompression are built in, whereas requests needs optional packages for them; this can also change the automatically advertised Accept-Encoding header.
- Encoding detection uses chardetng rather than chardet. Ambiguous byte streams can decode differently; UTF BOMs are honored explicitly. Decode failures discard invalid UTF-8, matching the original fallback intent. Direct byte XML parsing supports encoding_rs encodings and defaults to UTF-8; exotic encodings supported by Python may be unavailable.
- Unicode properties follow Rust/regex tables, which may differ for newly assigned characters from the Python interpreter’s Unicode version.
- All DTDs are refused, including harmless declarations that the original parser accepted. This deliberately reduces the untrusted XML surface.
- Snapshot creation uses a temporary file and rename before old files are removed, fixing loss of previous output when creating or writing the new file fails (including runs within the same second). A failure during old-file cleanup can leave both snapshots. Replacing a same-second snapshot replaces the file itself, resetting its metadata instead of truncating it; a symlink at that path is replaced rather than followed. Concurrent runs remain unsupported.
- The README corrects the original encoding-fallback description, source list, and overly broad claim that all ASCII headlines are removed. There is no guaranteed title count or run duration.

`REPORT.md` records the changes and validation.

## Development

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Tests use preserved RSS/Atom fixtures and local TCP HTTP mocks. No test contacts live sources. The CLI test redirects every HTTPS request to a closed loopback proxy. CI checks formatting, linting and tests on Ubuntu and Windows.

Dependencies are limited to reqwest (HTTP, rustls, compression and proxies), tokio (sequential HTTP runtime with per-read timeouts), roxmltree (strict XML), regex (the original filters), chardetng and encoding_rs (legacy encoding), and chrono (local timestamps). The only development dependency is tempfile for isolated filesystem tests; local HTTP mocks use the standard library.

## License

[MIT](LICENSE)
