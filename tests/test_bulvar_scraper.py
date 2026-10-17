#
# Project: czech-tabloid-scraper
# File:    test_bulvar_scraper.py
#
# Description:
# Tests for feed parsing and for the filter that separates headlines from navigation text.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

"""Tests for the Czech tabloid RSS scraper."""

import re

import pytest

from bulvar_scraper import (
    FEEDS,
    MIN_TITLE_LENGTH,
    clean_title,
    extract_titles,
    save_titles,
)

# The eleven patterns this scraper used before they collapsed into BREADCRUMB_RE.
# Kept here so the replacement stays provably equivalent.
LEGACY_BREADCRUMB_PATTERNS = [r"^[A-Z][a-z]+" + r"\s+[a-z]+" * n + r"$" for n in range(11)]

RSS = """<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel>
  <item><title>Prezident dnes podepsal novelu zákona o dani</title></item>
  <item><title>Krátké</title></item>
</channel></rss>"""

ATOM = """<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry><title>Vláda schválila rozpočet na příští kalendářní rok</title></entry>
</feed>"""


def matches_any_legacy(title):
    return any(re.match(p, title) for p in LEGACY_BREADCRUMB_PATTERNS)


# --- breadcrumb filter ------------------------------------------------------


@pytest.mark.parametrize("extra_words", range(11))
def test_breadcrumb_filter_matches_every_legacy_pattern(extra_words):
    """One capitalized word plus n lowercase words, for every n the old list had."""
    title = "Zpravodajstvi" + " domova" * extra_words
    title = title + "x" * max(0, MIN_TITLE_LENGTH - len(title))
    assert matches_any_legacy(title)
    assert clean_title(title) == ""


@pytest.mark.parametrize(
    "title",
    [
        "Ministr financi oznamil zmenu v DPH",  # second capital
        "Cesko ma 10 novych nemocnic celkem",  # digit
        "Praha, Brno a Ostrava hlasi problem",  # punctuation
    ],
)
def test_real_headlines_survive_the_breadcrumb_filter(title):
    assert not matches_any_legacy(title)
    assert clean_title(title) == title


def test_diacritics_bypass_the_breadcrumb_filter():
    """The filter uses [a-z], which is ASCII only.

    A Czech headline almost always carries a diacritic, so it never looks like a
    breadcrumb. Navigation labels are usually written without them and get cut.
    This is what stops the filter from eating real headlines, so it is behavior
    worth pinning down rather than an accident to tidy away.
    """
    ascii_only = "Prezident dnes podepsal novelu zakona"
    with_diacritics = "Prezident dnes podepsal novelu zákona"
    assert clean_title(ascii_only) == ""
    assert clean_title(with_diacritics) == with_diacritics


def test_eleven_word_breadcrumb_is_dropped_but_twelve_word_is_kept():
    """The old pattern list stopped at ten trailing words. Keep that boundary."""
    eleven = "Zpravy" + " domova" * 10
    twelve = "Zpravy" + " domova" * 11
    assert clean_title(eleven) == ""
    assert clean_title(twelve) == twelve


# --- clean_title ------------------------------------------------------------


def test_cdata_wrapper_is_removed():
    assert clean_title("<![CDATA[Prezident podepsal novelu zákona]]>") == ("Prezident podepsal novelu zákona")


def test_whitespace_is_collapsed():
    assert clean_title("  Prezident\n\tpodepsal   novelu zákona  ") == ("Prezident podepsal novelu zákona")


@pytest.mark.parametrize("title", ["", None, "Krátký titulek"])
def test_short_and_empty_titles_are_dropped(title):
    assert clean_title(title) == ""


def test_all_caps_navigation_is_dropped():
    assert clean_title("NEJNOVEJSI CLANKY Z DOMOVA A ZE SVETA CELKEM") == ""


def test_a_long_headline_at_the_length_boundary_is_kept():
    title = "A" + "b" * (MIN_TITLE_LENGTH - 1) + " C2"
    assert clean_title(title) == title


# --- extract_titles ---------------------------------------------------------


def test_rss_items_are_read_and_short_titles_filtered():
    assert extract_titles(RSS) == ["Prezident dnes podepsal novelu zákona o dani"]


def test_namespaced_atom_entries_are_read():
    """Atom namespaces its tags, so a plain .//entry lookup finds nothing."""
    assert extract_titles(ATOM) == ["Vláda schválila rozpočet na příští kalendářní rok"]


def test_malformed_xml_yields_no_titles():
    assert extract_titles("<rss><channel><item></channel>") == []


def test_empty_feed_yields_no_titles():
    assert extract_titles('<?xml version="1.0"?><rss><channel/></rss>') == []


def test_entity_expansion_is_refused():
    """A billion-laughs payload must not be expanded. The feeds are untrusted."""
    bomb = """<?xml version="1.0"?>
    <!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;&lol;">]>
    <rss><channel><item><title>&lol2;</title></item></channel></rss>"""
    assert extract_titles(bomb) == []


def test_titles_are_decoded_from_a_windows_1250_feed():
    raw = RSS.replace("utf-8", "windows-1250")
    assert extract_titles(raw.encode("windows-1250")) == ["Prezident dnes podepsal novelu zákona o dani"]


# --- save_titles ------------------------------------------------------------


def test_titles_are_written_one_per_line(tmp_path):
    path = save_titles(["first", "second"], tmp_path)
    assert path.read_text(encoding="utf-8") == "first\nsecond\n"


def test_saving_replaces_the_output_of_an_earlier_run(tmp_path):
    stale = tmp_path / "titles_20200101_000000.txt"
    stale.write_text("old", encoding="utf-8")
    path = save_titles(["new"], tmp_path)
    assert not stale.exists()
    assert list(tmp_path.glob("titles_*.txt")) == [path]


def test_saving_creates_a_missing_output_directory(tmp_path):
    target = tmp_path / "nested" / "out"
    path = save_titles(["x"], target)
    assert path.exists()


# --- feed list --------------------------------------------------------------


def test_feed_list_has_eighteen_unique_https_sources():
    assert len(FEEDS) == 18
    assert len(set(FEEDS.values())) == 18
    assert all(url.startswith("https://") for url in FEEDS.values())
