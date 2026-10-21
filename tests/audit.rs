// Project: czech-tabloid-scraper
// File: tests/audit.rs
// Description: Adversarial regression cases captured from the reference implementation.
// Author: Jan Alexandr Kopřiva
// License: MIT

use czech_tabloid_scraper::*;
fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn captured_reference_cases() {
    for line in include_str!("fixtures/reference.tsv").lines() {
        let columns: Vec<_> = line.split('\t').collect();
        let raw = unhex(columns[1]);
        let expected: Vec<String> = if columns[2] == "-" {
            vec![]
        } else {
            columns[2]
                .split(',')
                .map(|s| String::from_utf8(unhex(s)).unwrap())
                .collect()
        };
        let actual = match columns[0] {
            "clean" => vec![clean_title(std::str::from_utf8(&raw).unwrap())],
            "decode" => vec![decode_feed(&raw)],
            "bytes" => extract_titles_bytes(&raw),
            _ => extract_titles(std::str::from_utf8(&raw).unwrap()),
        };
        assert_eq!(actual, expected, "{}", columns[3]);
    }
}
#[test]
fn same_second_save_replaces_destination_and_cleans_staging() {
    let dir = tempfile::tempdir().unwrap();
    for _ in 0..5 {
        let path = save_titles(&["new".into()], dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
            "new\n"
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn entity_text_in_comments_is_safe_but_declarations_are_refused() {
    let title = "Prezident dnes podepsal novelu zákona";
    for extra in [
        "<!-- <!ENTITY fake 'x'> -->",
        "<?test <!ENTITY fake ?>",
        "<!DOCTYPE rss>",
    ] {
        assert_eq!(
            extract_titles(&format!(
                "{extra}<rss><item><title>{title}</title></item></rss>"
            )),
            [title]
        );
    }
    assert!(extract_titles("<!DOCTYPE rss [<!ENTITY unused 'x'>]><rss/>").is_empty());
}
#[test]
fn large_and_invalid_inputs_do_not_panic() {
    let title = format!("Zpráva 2: {}", "text ".repeat(200_000));
    let xml = format!("<rss><item><title>{title}</title></item></rss>");
    assert_eq!(extract_titles(&xml), [title.trim()]);
    for length in 0..256 {
        let bytes: Vec<u8> = (0..length).map(|i| i as u8).collect();
        let _ = decode_feed(&bytes);
        assert!(extract_titles_bytes(&bytes).is_empty());
    }
}

#[test]
fn depth_limit_and_markup_are_checked_without_recursion() {
    let title = "Prezident dnes podepsal novelu zákona";
    let item = format!("<item><title>{title}</title></item>");
    assert_eq!(
        extract_titles(&format!(
            "{}{}{}",
            "<r>".repeat(125),
            item,
            "</r>".repeat(125)
        )),
        [title]
    );
    assert!(extract_titles(&format!(
        "{}{}{}",
        "<r>".repeat(128),
        item,
        "</r>".repeat(128)
    ))
    .is_empty());
    let noise = "<fake>".repeat(2000);
    for xml in [
        format!("<!--{noise}--><r>{item}</r>"),
        format!("<r ignored='{}'>{item}</r>", noise.replace('<', "&lt;")),
        format!("<r><![CDATA[{noise}]]>{item}</r>"),
    ] {
        assert_eq!(extract_titles(&xml), [title]);
    }
}

#[test]
fn cleanup_error_keeps_the_new_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("titles_blocked.txt")).unwrap();
    assert!(save_titles(&["new".into()], dir.path()).is_err());
    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&files[0])
            .unwrap()
            .replace("\r\n", "\n"),
        "new\n"
    );
}

#[cfg(unix)]
#[test]
fn snapshots_are_private_and_symlink_targets_are_untouched() {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let external = dir.path().join("external");
    std::fs::write(&external, "keep").unwrap();
    for offset in -2..=2 {
        let time = chrono::Local::now() + chrono::Duration::seconds(offset);
        symlink(
            &external,
            dir.path()
                .join(format!("titles_{}.txt", time.format("%Y%m%d_%H%M%S"))),
        )
        .unwrap();
    }
    let path = save_titles(&["new".into()], dir.path()).unwrap();
    assert_eq!(std::fs::read_to_string(external).unwrap(), "keep");
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn server_charset_beats_detection() {
    // "Česká policie zadržela podezřelého z krádeže" in windows-1250.
    let (bytes, _, _) =
        encoding_rs::WINDOWS_1250.encode("Česká policie zadržela podezřelého z krádeže");
    assert_eq!(
        decode_feed_as(
            &bytes,
            charset_of("text/xml; charset=\"Windows-1250\"").as_deref()
        ),
        "Česká policie zadržela podezřelého z krádeže"
    );
    assert_eq!(charset_of("text/xml"), None);
}
