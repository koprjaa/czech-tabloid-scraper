// Project: czech-tabloid-scraper
// File: src/lib.rs
// Description: Fetch, filter and save Czech news headlines.
// Author: Jan Alexandr Kopřiva
// License: MIT

pub const ASCII_ART: &str = r"
▛▀▖   ▜
▙▄▘▌ ▌▐▌ ▌▝▀▖▙▀▖ ▞▀▘▞▀▖▙▀▖▝▀▖▛▀▖▞▀▖▙▀▖
▌ ▌▌ ▌▐▐▐ ▞▀▌▌   ▝▀▖▌ ▖▌  ▞▀▌▙▄▘▛▀ ▌
▀▀ ▝▀▘ ▘▘ ▝▀▘▘   ▀▀ ▝▀ ▘  ▝▀▘▌  ▝▀▘▘
";
pub const FEEDS: &[(&str, &str)] = &[
    ("super_cz", "https://www.super.cz/rss"),
    ("blesk_cz", "https://www.blesk.cz/rss"),
    ("extra_cz", "https://www.extra.cz/rss.xml"),
    ("ahaonline_cz", "https://www.ahaonline.cz/rss"),
    ("novinky_cz", "https://www.novinky.cz/rss"),
    ("idnes_cz", "https://servis.idnes.cz/rss.aspx"),
    ("prozeny_cz", "https://www.prozeny.cz/rss"),
    ("zive_cz", "https://www.zive.cz/rss"),
    ("doupe_cz", "https://doupe.zive.cz/rss"),
    (
        "zive_sk_najnovsie",
        "https://zive.aktuality.sk/rss/najnovsie/",
    ),
    (
        "zive_sk_mobilmania",
        "https://zive.aktuality.sk/rss/mobilmania/",
    ),
    ("lupa_cz", "https://www.lupa.cz/rss/clanky-samostatne/"),
    ("root_cz", "https://www.root.cz/rss/clanky/"),
    ("reflex_cz", "https://www.reflex.cz/rss"),
    ("respekt_cz", "https://www.respekt.cz/api/rss"),
    (
        "ceskenoviny_cz",
        "https://www.ceskenoviny.cz/sluzby/rss/zpravy.php",
    ),
    ("irozhlas_cz", "https://www.irozhlas.cz/rss/irozhlas"),
    (
        "ct24_cz",
        "https://ct24.ceskatelevize.cz/rss/tema/vyber-redakce-84313",
    ),
];

use regex::Regex;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};

pub const MIN_TITLE_LENGTH: usize = 20;
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
static CDATA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!\[CDATA\[(.*?)\]\]>").unwrap());
static BREADCRUMB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z][a-z]+(?:\s+[a-z]+){0,10}$").unwrap());
static CAPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z\s]+$").unwrap());

fn python_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

pub fn clean_title(title: &str) -> String {
    let unwrapped = CDATA.replace_all(title, "$1");
    let title = unwrapped
        .split(python_space)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let length = title.chars().count();
    static UPPER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:\p{Uppercase}|\P{Cased})+$").unwrap());
    let upper = title.chars().any(char::is_uppercase) && UPPER.is_match(&title);
    if length < MIN_TITLE_LENGTH
        || (upper && length < 50)
        || BREADCRUMB.is_match(&title)
        || CAPS.is_match(&title)
    {
        String::new()
    } else {
        title
    }
}

// roxmltree parses elements recursively; bound depth before entering its parser.
fn bounded_xml_depth(xml: &str) -> bool {
    let bytes = xml.as_bytes();
    let mut cursor = 0;
    let mut depth = 0usize;
    while let Some(offset) = xml[cursor..].find('<') {
        let start = cursor + offset;
        let tail = &xml[start..];
        let skipped = if tail.starts_with("<!--") {
            Some("-->")
        } else if tail.starts_with("<![CDATA[") {
            Some("]]>")
        } else if tail.starts_with("<?") {
            Some("?>")
        } else {
            None
        };
        if let Some(ending) = skipped {
            let Some(end) = tail.find(ending) else {
                return false;
            };
            cursor = start + end + ending.len();
            continue;
        }
        let mut end = start + 1;
        let mut quote = None;
        let mut brackets = 0usize;
        while end < bytes.len() {
            let byte = bytes[end];
            if let Some(delimiter) = quote {
                if byte == delimiter {
                    quote = None;
                }
            } else {
                match byte {
                    b'\'' | b'"' => quote = Some(byte),
                    b'[' if tail.starts_with("<!") => brackets += 1,
                    b']' => brackets = brackets.saturating_sub(1),
                    b'>' if brackets == 0 => break,
                    _ => {}
                }
            }
            end += 1;
        }
        if end == bytes.len() {
            return false;
        }
        if tail.starts_with("</") {
            depth = depth.saturating_sub(1);
        } else if !tail.starts_with("<!") {
            if depth >= 128 {
                return false;
            }
            if !xml[start..end].trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = end + 1;
    }
    true
}

pub fn extract_titles(xml: &str) -> Vec<String> {
    if !bounded_xml_depth(xml) {
        return vec![];
    }
    // Ignore declaration-like text inside comments, CDATA and processing instructions.
    static DECLARATIONS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<!--.*?-->|<!\[CDATA\[.*?\]\]>|<\?.*?\?>|(?P<entity><!ENTITY\s)").unwrap()
    });
    if DECLARATIONS
        .captures_iter(xml)
        .any(|c| c.name("entity").is_some())
    {
        return vec![];
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(doc) = roxmltree::Document::parse_with_options(xml, options) else {
        return vec![];
    };
    let root = doc.root_element();
    let mut items: Vec<_> = root
        .descendants()
        .skip(1)
        .filter(|n| {
            n.is_element() && n.tag_name().namespace().is_none() && n.tag_name().name() == "item"
        })
        .collect();
    if items.is_empty() {
        items = root
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "entry")
            .collect();
    }
    items
        .into_iter()
        .filter_map(|item| {
            let title = item
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "title")?;
            let initial: String = title
                .children()
                .take_while(|n| !n.is_element())
                .filter_map(|n| n.text().filter(|_| n.is_text()))
                .collect();
            let cleaned = clean_title(&initial);
            (!cleaned.is_empty()).then_some(cleaned)
        })
        .collect()
}

fn wide_encoding(raw: &[u8]) -> Option<String> {
    let (little, width, skip) = if raw.starts_with(&[0xff, 0xfe, 0, 0]) {
        (true, 4, 4)
    } else if raw.starts_with(&[0, 0, 0xfe, 0xff]) {
        (false, 4, 4)
    } else if raw.starts_with(&[b'<', 0, 0, 0]) {
        (true, 4, 0)
    } else if raw.starts_with(&[0, 0, 0, b'<']) {
        (false, 4, 0)
    } else if raw.starts_with(&[b'<', 0]) {
        (true, 2, 0)
    } else if raw.starts_with(&[0, b'<']) {
        (false, 2, 0)
    } else {
        return None;
    };
    let data = &raw[skip..];
    if !data.len().is_multiple_of(width) {
        return Some(String::new());
    }
    if width == 2 {
        let units = data.as_chunks::<2>().0.iter().map(|c| {
            if little {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        });
        Some(
            char::decode_utf16(units)
                .collect::<Result<String, _>>()
                .unwrap_or_default(),
        )
    } else {
        Some(
            data.as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    let bytes = [c[0], c[1], c[2], c[3]];
                    char::from_u32(if little {
                        u32::from_le_bytes(bytes)
                    } else {
                        u32::from_be_bytes(bytes)
                    })
                })
                .collect::<Option<String>>()
                .unwrap_or_default(),
        )
    }
}

pub fn decode_feed(raw: &[u8]) -> String {
    if let Some(text) = wide_encoding(raw) {
        return text;
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(raw, true);
    let encoding = encoding_rs::Encoding::for_bom(raw)
        .map_or_else(|| detector.guess(None, true), |(encoding, _)| encoding);
    let (content, _, errors) = encoding.decode(raw);
    if errors {
        let mut remaining = raw;
        let mut valid = String::new();
        while !remaining.is_empty() {
            match std::str::from_utf8(remaining) {
                Ok(text) => {
                    valid.push_str(text);
                    break;
                }
                Err(error) => {
                    valid.push_str(std::str::from_utf8(&remaining[..error.valid_up_to()]).unwrap());
                    remaining = &remaining[error.valid_up_to()
                        + error
                            .error_len()
                            .unwrap_or(remaining.len() - error.valid_up_to())..];
                }
            }
        }
        valid
    } else {
        content.into_owned()
    }
}

pub fn extract_titles_bytes(raw: &[u8]) -> Vec<String> {
    if let Some(text) = wide_encoding(raw) {
        return extract_titles(&text);
    }
    let prefix = String::from_utf8_lossy(raw);
    static ENCODING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"\A\u{feff}?<\?xml[ \t\r\n]+[^?]*\bencoding[ \t\r\n]*=[ \t\r\n]*["']([^"']+)["']"#,
        )
        .unwrap()
    });
    let encoding = if let Some(capture) = ENCODING.captures(&prefix) {
        let label = capture[1].to_ascii_lowercase().replace('_', "-");
        if matches!(
            label.as_str(),
            "iso-8859-1" | "iso8859-1" | "latin-1" | "latin1"
        ) {
            let xml: String = raw.iter().map(|&b| char::from(b)).collect();
            return extract_titles(&xml);
        }
        if matches!(label.as_str(), "ascii" | "us-ascii") && !raw.is_ascii() {
            return vec![];
        }
        let Some(encoding) = encoding_rs::Encoding::for_label(capture[1].as_bytes()) else {
            return vec![];
        };
        encoding
    } else {
        encoding_rs::UTF_8
    };
    let (xml, _, errors) = encoding.decode(raw);
    if errors {
        vec![]
    } else {
        extract_titles(&xml)
    }
}

pub struct HttpClient {
    runtime: tokio::runtime::Runtime,
}

pub fn http_client() -> Result<HttpClient, Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(HttpClient { runtime })
}

pub fn fetch_rss_feed(client: &HttpClient, url: &str) -> Option<String> {
    client.runtime.block_on(async {
        // requests.get creates a fresh session; cookies only survive redirects.
        let session = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(15))
            .retry(reqwest::retry::never())
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(30))
            .build()
            .ok()?;
        let response = session
            .get(url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        Some(decode_feed(&response.bytes().await.ok()?))
    })
}

pub fn save_titles(titles: &[String], output_dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join(format!(
        "titles_{}.txt",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    let mut content: String = titles.iter().map(|title| format!("{title}\n")).collect();
    if cfg!(windows) {
        content = content.replace('\n', "\r\n");
    }
    // Write first so a failed write cannot destroy the previous snapshot.
    let mut staging = tempfile::NamedTempFile::new_in(output_dir)?;
    {
        use io::Write;
        staging.write_all(content.as_bytes())?;
        staging.as_file().sync_all()?;
    }
    staging.persist(&path).map_err(|error| error.error)?;
    for entry in fs::read_dir(output_dir)? {
        let old = entry?.path();
        let name = old.file_name().unwrap_or_default().to_string_lossy();
        let name = if cfg!(windows) {
            name.to_lowercase()
        } else {
            name.into_owned()
        };
        if old != path && name.starts_with("titles_") && name.ends_with(".txt") {
            match fs::remove_file(old) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
    }
    Ok(path)
}

pub fn run_in_current_dir<W: io::Write>(
    client: &HttpClient,
    feeds: &[(&str, &str)],
    log: W,
) -> io::Result<()> {
    let output = std::env::current_dir()?.join("scraped_output");
    run(client, feeds, &output, log)
}

pub fn run<W: io::Write>(
    client: &HttpClient,
    feeds: &[(&str, &str)],
    output: &Path,
    mut log: W,
) -> io::Result<()> {
    writeln!(log, "{ASCII_ART}\nStarting scraper...")?;
    let mut titles = vec![];
    for (name, url) in feeds {
        match fetch_rss_feed(client, url).filter(|s| !s.is_empty()) {
            Some(xml) => titles.extend(extract_titles(&xml)),
            None => writeln!(log, "✗ Failed to fetch {name}")?,
        }
    }
    writeln!(
        log,
        "Scraping {}... ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100%",
        feeds.last().map_or("feeds", |f| f.0)
    )?;
    writeln!(
        log,
        "Scraping finished! Found {} titles total.",
        titles.len()
    )?;
    if titles.is_empty() {
        writeln!(log, "No titles found.")?;
    } else {
        match save_titles(&titles, output) {
            Ok(path) => writeln!(
                log,
                "╭─ Success ─╮\nSaved {} titles to:\n{}\n╰───────────╯",
                titles.len(),
                path.display()
            )?,
            Err(error) => writeln!(log, "Error saving file: {error}")?,
        }
    }
    Ok(())
}
