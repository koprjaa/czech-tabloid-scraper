// Project: czech-tabloid-scraper
// File: tests/parity.rs
// Description: Reference parity and offline integration tests.
// Author: Jan Alexandr Kopřiva
// License: MIT

use czech_tabloid_scraper::*;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
};
const RSS: &str = include_str!("fixtures/rss.xml");
const ATOM: &str = include_str!("fixtures/atom.xml");

#[test]
fn every_legacy_breadcrumb_pattern() {
    for n in 0..11 {
        let mut title = format!("Zpravodajstvi{}", " domova".repeat(n));
        title.push_str(&"x".repeat(20usize.saturating_sub(title.len())));
        assert_eq!(clean_title(&title), "", "pattern {n}");
    }
}
#[test]
fn real_headlines_survive() {
    for title in [
        "Ministr financi oznamil zmenu v DPH",
        "Cesko ma 10 novych nemocnic celkem",
        "Praha, Brno a Ostrava hlasi problem",
    ] {
        assert_eq!(clean_title(title), title);
    }
}
#[test]
fn diacritics_bypass_breadcrumb() {
    assert_eq!(clean_title("Prezident dnes podepsal novelu zakona"), "");
    let title = "Prezident dnes podepsal novelu zákona";
    assert_eq!(clean_title(title), title);
}
#[test]
fn eleven_and_twelve_words() {
    assert_eq!(clean_title(&format!("Zpravy{}", " domova".repeat(10))), "");
    let title = format!("Zpravy{}", " domova".repeat(11));
    assert_eq!(clean_title(&title), title);
}
#[test]
fn cdata_wrapper() {
    assert_eq!(
        clean_title("<![CDATA[Prezident podepsal novelu zákona]]>"),
        "Prezident podepsal novelu zákona"
    );
}
#[test]
fn whitespace() {
    assert_eq!(
        clean_title("  Prezident\n\tpodepsal   novelu zákona  "),
        "Prezident podepsal novelu zákona"
    );
}
#[test]
fn short_empty_and_absent() {
    for title in [Some(""), None, Some("Krátký titulek")] {
        assert_eq!(clean_title(title.unwrap_or_default()), "");
    }
}
#[test]
fn all_caps() {
    assert_eq!(
        clean_title("NEJNOVEJSI CLANKY Z DOMOVA A ZE SVETA CELKEM"),
        ""
    );
}
#[test]
fn length_boundary() {
    let title = format!("A{} C2", "b".repeat(MIN_TITLE_LENGTH - 1));
    assert_eq!(clean_title(&title), title);
}
#[test]
fn rss_items() {
    assert_eq!(
        extract_titles(RSS),
        ["Prezident dnes podepsal novelu zákona o dani"]
    );
}
#[test]
fn atom_entries() {
    assert_eq!(
        extract_titles(ATOM),
        ["Vláda schválila rozpočet na příští kalendářní rok"]
    );
}
#[test]
fn malformed_xml() {
    assert!(extract_titles("<rss><channel><item></channel>").is_empty());
}
#[test]
fn empty_xml() {
    assert!(extract_titles("<?xml version=\"1.0\"?><rss><channel/></rss>").is_empty());
}
#[test]
fn entities_refused() {
    assert!(extract_titles("<!DOCTYPE lolz [<!ENTITY lol 'lol'><!ENTITY lol2 '&lol;&lol;'>]><rss><item><title>&lol2;</title></item></rss>").is_empty());
}
#[test]
fn windows_1250() {
    let xml = RSS.replace("utf-8", "windows-1250");
    let (bytes, _, errors) = encoding_rs::WINDOWS_1250.encode(&xml);
    assert!(!errors);
    assert_eq!(extract_titles_bytes(&bytes), extract_titles(RSS));
    assert_eq!(extract_titles(&decode_feed(&bytes)), extract_titles(RSS));
}
#[test]
fn one_title_per_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = save_titles(&["first".into(), "second".into()], dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
        "first\nsecond\n"
    );
}
#[test]
fn replaces_previous_run() {
    let dir = tempfile::tempdir().unwrap();
    let stale = dir.path().join("titles_20200101_000000.txt");
    fs::write(&stale, "old").unwrap();
    fs::write(dir.path().join("keep.txt"), "keep").unwrap();
    let path = save_titles(&["new".into()], dir.path()).unwrap();
    assert!(!stale.exists());
    assert!(path.exists());
    assert!(dir.path().join("keep.txt").exists());
}
#[test]
fn creates_directory() {
    let dir = tempfile::tempdir().unwrap();
    assert!(save_titles(&["x".into()], &dir.path().join("nested/out"))
        .unwrap()
        .exists());
}
#[test]
fn feed_list() {
    assert_eq!(FEEDS.len(), 18);
    let urls: std::collections::HashSet<_> = FEEDS.iter().map(|f| f.1).collect();
    assert_eq!(urls.len(), 18);
    assert!(urls.iter().all(|u| u.starts_with("https://")));
}
fn mock(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = vec![];
        let mut buf = [0; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
        }
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, handle)
}
#[test]
fn http_headers_and_successful_run() {
    let (url, server) = mock("200 OK", RSS);
    let dir = tempfile::tempdir().unwrap();
    let mut log = vec![];
    run(
        &http_client().unwrap(),
        &[("test", &url)],
        dir.path(),
        &mut log,
    )
    .unwrap();
    let request = server.join().unwrap();
    assert!(request
        .to_ascii_lowercase()
        .contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase())));
    let log = String::from_utf8(log).unwrap();
    assert!(log.contains("Scraping finished! Found 1 titles total."));
    assert!(log.contains("Saved 1 titles to:"));
}
#[test]
fn http_failure_empty_and_malformed() {
    for (status, body, failed) in [
        ("500 Internal Server Error", RSS, true),
        ("200 OK", "", true),
        ("200 OK", "<broken>", false),
    ] {
        let (url, server) = mock(status, body);
        let dir = tempfile::tempdir().unwrap();
        let mut log = vec![];
        assert!(!run(
            &http_client().unwrap(),
            &[("test", &url)],
            dir.path(),
            &mut log,
        )
        .unwrap());
        server.join().unwrap();
        let log = String::from_utf8(log).unwrap();
        assert_eq!(log.contains("✗ Failed to fetch test"), failed);
        assert!(log.contains("No titles found."));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
#[test]
fn failed_save_preserves_old_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("titles_20000101_000000.txt");
    fs::write(&old, "old").unwrap();
    for offset in -2..=2 {
        let time = chrono::Local::now() + chrono::Duration::seconds(offset);
        let name = format!("titles_{}.txt", time.format("%Y%m%d_%H%M%S"));
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    assert!(save_titles(&["new".into()], dir.path()).is_err());
    assert_eq!(fs::read_to_string(old).unwrap(), "old");
}
#[test]
fn xml_selection_order_duplicates_and_initial_text() {
    let title = "Prezident dnes podepsal novelu zákona";
    let xml = format!("<feed><entry><title>Vláda schválila rozpočet na příští rok</title></entry><item><title>{title}<b>ignored</b>tail</title></item><item><title>{title}</title></item></feed>");
    assert_eq!(extract_titles(&xml), [title, title]);
    assert!(extract_titles(&format!("<item><title>{title}</title></item>")).is_empty());
    assert_eq!(
        extract_titles(&format!("<entry><title>{title}</title></entry>")),
        [title]
    );
}
#[test]
fn cli_ignores_arguments_and_handles_network_failure() {
    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let working_dir = dir.path().join("working");
    fs::create_dir(&bin_dir).unwrap();
    fs::create_dir(&working_dir).unwrap();
    let executable = bin_dir.join(if cfg!(windows) {
        "bulvar_scraper.exe"
    } else {
        "bulvar_scraper"
    });
    fs::copy(env!("CARGO_BIN_EXE_bulvar_scraper"), &executable).unwrap();
    // A closed loopback port ensures every feed fails without touching live sites.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let output = Command::new(executable)
        .arg("--ignored")
        .current_dir(&working_dir)
        .env("HTTPS_PROXY", &proxy)
        .env("https_proxy", &proxy)
        .env("ALL_PROXY", &proxy)
        .env("all_proxy", &proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let log = String::from_utf8(output.stdout).unwrap();
    assert_eq!(log.matches("✗ Failed to fetch ").count(), 18);
    assert!(log.contains("No titles found."));
    assert!(!working_dir.join("scraped_output").exists());
    assert!(!bin_dir.join("scraped_output").exists());
}

#[test]
fn unicode_uppercase_boundaries_and_whitespace() {
    assert_eq!(clean_title(&"Á".repeat(49)), "");
    assert_eq!(clean_title(&"Á".repeat(50)), "Á".repeat(50));
    assert_eq!(clean_title(&"A".repeat(60)), "");
    let title = format!("{}ǅ", "Á".repeat(20));
    assert_eq!(clean_title(&title), title);
    assert_eq!(
        clean_title("Prezident\u{1c}dnes podepsal novelu zákona"),
        "Prezident dnes podepsal novelu zákona"
    );
}

#[test]
fn save_failure_is_reported_and_empty_save_is_allowed() {
    let (url, server) = mock("200 OK", RSS);
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("file");
    fs::write(&blocked, "block").unwrap();
    let mut log = vec![];
    assert!(!run(
        &http_client().unwrap(),
        &[("test", &url)],
        &blocked,
        &mut log,
    )
    .unwrap());
    server.join().unwrap();
    assert!(String::from_utf8(log)
        .unwrap()
        .contains("Error saving file:"));
    let output = dir.path().join("empty");
    let path = save_titles(&[], &output).unwrap();
    assert!(fs::read(path).unwrap().is_empty());
}

#[test]
fn utf_boms_and_dtd_policy() {
    let text = RSS.replace("utf-8", "utf-16");
    let mut bytes = vec![0xff, 0xfe];
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    assert_eq!(extract_titles(&decode_feed(&bytes)), extract_titles(RSS));
    assert_eq!(extract_titles_bytes(&bytes), extract_titles(RSS));
    assert!(extract_titles("<!DOCTYPE rss><rss/>").is_empty());
}

#[test]
fn redirects_keep_cookies_only_within_one_fetch() {
    let (destination, final_server) = mock("200 OK", RSS);
    let redirect_status =
        format!("302 Found\r\nLocation: {destination}\r\nSet-Cookie: visit=1; Path=/");
    let (origin, redirect_server) = mock(&redirect_status, "");
    let client = http_client().unwrap();
    assert_eq!(
        extract_titles(&fetch_rss_feed(&client, &origin).unwrap()),
        extract_titles(RSS)
    );
    redirect_server.join().unwrap();
    assert!(final_server
        .join()
        .unwrap()
        .to_lowercase()
        .contains("cookie: visit=1"));
    let (url, server) = mock("200 OK", RSS);
    assert!(fetch_rss_feed(&client, &url).is_some());
    assert!(!server.join().unwrap().to_lowercase().contains("cookie:"));
}

#[test]
fn current_directory_output_and_cleanup() {
    const CHILD: &str = "SCRAPER_CWD_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let (url, server) = mock("200 OK", RSS);
        run_in_current_dir(&http_client().unwrap(), &[("test", &url)], std::io::sink()).unwrap();
        server.join().unwrap();
        return;
    }

    // Use a child process to avoid changing the working directory of parallel tests.
    let root = tempfile::tempdir().unwrap();
    let cwd = root.path().join("working");
    let output_dir = cwd.join("scraped_output");
    fs::create_dir_all(output_dir.join("nested")).unwrap();
    let stale = output_dir.join("titles_20000101_000000.txt");
    fs::write(&stale, "old").unwrap();
    let protected = [
        cwd.join("titles_20000101_000000.txt"),
        output_dir.join("keep.txt"),
        output_dir.join("titles_keep.csv"),
        output_dir.join("nested/titles_20000101_000000.txt"),
        root.path()
            .join("scraped_output/titles_20000101_000000.txt"),
    ];
    for path in &protected {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "keep").unwrap();
    }
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "current_directory_output_and_cleanup",
            "--nocapture",
        ])
        .current_dir(&cwd)
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(!stale.exists());
    for path in protected {
        assert_eq!(fs::read_to_string(path).unwrap(), "keep");
    }
    let snapshots: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with("titles_") && name.ends_with(".txt")
        })
        .collect();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        fs::read_to_string(&snapshots[0])
            .unwrap()
            .replace("\r\n", "\n"),
        "Prezident dnes podepsal novelu zákona o dani\n"
    );
}

#[test]
fn read_timeout_is_enforced() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx")
            .unwrap();
        thread::sleep(std::time::Duration::from_secs(16));
    });
    let started = std::time::Instant::now();
    assert!(fetch_rss_feed(&http_client().unwrap(), &url).is_none());
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    server.join().unwrap();
}

#[test]
fn redirect_loop_is_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let location = url.clone();
    let server = thread::spawn(move || {
        for _ in 0..31 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
            }
            write!(stream, "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        }
    });
    assert!(fetch_rss_feed(&http_client().unwrap(), &url).is_none());
    server.join().unwrap();
}
