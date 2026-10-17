#!/usr/bin/env python3
#
# Project: czech-tabloid-scraper
# File:    bulvar_scraper.py
#
# Description:
# Reads Czech tabloid RSS and Atom feeds, filters navigation text out of the titles, and writes the titles to a file.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

import re
import xml.etree.ElementTree as ET
from datetime import datetime
from pathlib import Path

import chardet
import requests

# defusedxml blocks entity-expansion and external-entity attacks. The feeds are
# third-party XML, so they are untrusted input.
from defusedxml.common import DefusedXmlException
from defusedxml.ElementTree import fromstring as defused_fromstring
from rich.console import Console
from rich.panel import Panel
from rich.progress import (
    BarColumn,
    Progress,
    SpinnerColumn,
    TaskProgressColumn,
    TextColumn,
    TimeElapsedColumn,
)
from rich.text import Text

# Initialize Rich Console
console = Console()

ASCII_ART = r"""
▛▀▖   ▜
▙▄▘▌ ▌▐▌ ▌▝▀▖▙▀▖ ▞▀▘▞▀▖▙▀▖▝▀▖▛▀▖▞▀▖▙▀▖
▌ ▌▌ ▌▐▐▐ ▞▀▌▌   ▝▀▖▌ ▖▌  ▞▀▌▙▄▘▛▀ ▌
▀▀ ▝▀▘ ▘▘ ▝▀▘▘   ▀▀ ▝▀ ▘  ▝▀▘▌  ▝▀▘▘
"""

CDATA_RE = re.compile(r"<!\[CDATA\[(.*?)\]\]>")

# A breadcrumb reads "Zpravy z domova": one capitalized word, then lowercase words
# only. Real headlines carry a second capital, a digit or punctuation somewhere.
# The upper bound of ten trailing words comes from the original pattern list.
BREADCRUMB_RE = re.compile(r"^[A-Z][a-z]+(?:\s+[a-z]+){0,10}$")

# Menu labels are often set in capitals, for example "NEJNOVEJSI CLANKY".
ALL_CAPS_RE = re.compile(r"^[A-Z\s]+$")

# Shorter strings are navigation, not headlines.
MIN_TITLE_LENGTH = 20

FEEDS = {
    "super_cz": "https://www.super.cz/rss",
    "blesk_cz": "https://www.blesk.cz/rss",
    "extra_cz": "https://www.extra.cz/rss.xml",
    "ahaonline_cz": "https://www.ahaonline.cz/rss",
    "novinky_cz": "https://www.novinky.cz/rss",
    "idnes_cz": "https://servis.idnes.cz/rss.aspx",
    "prozeny_cz": "https://www.prozeny.cz/rss",
    "zive_cz": "https://www.zive.cz/rss",
    "doupe_cz": "https://doupe.zive.cz/rss",
    "zive_sk_najnovsie": "https://zive.aktuality.sk/rss/najnovsie/",
    "zive_sk_mobilmania": "https://zive.aktuality.sk/rss/mobilmania/",
    "lupa_cz": "https://www.lupa.cz/rss/clanky-samostatne/",
    "root_cz": "https://www.root.cz/rss/clanky/",
    "reflex_cz": "https://www.reflex.cz/rss",
    "respekt_cz": "https://www.respekt.cz/api/rss",
    "ceskenoviny_cz": "https://www.ceskenoviny.cz/sluzby/rss/zpravy.php",
    "irozhlas_cz": "https://www.irozhlas.cz/rss/irozhlas",
    "ct24_cz": "https://ct24.ceskatelevize.cz/rss/tema/vyber-redakce-84313",
}


def print_art():
    """Print embedded ASCII art."""
    console.print(Text(ASCII_ART, style="bold white"))


def fetch_rss_feed(url):
    """Fetch RSS feed from the given URL."""
    try:
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
        }
        response = requests.get(url, headers=headers, timeout=15)
        response.raise_for_status()

        raw_content = response.content
        detected = chardet.detect(raw_content)
        encoding = detected["encoding"] if detected["encoding"] else "utf-8"

        try:
            content = raw_content.decode(encoding)
        except UnicodeDecodeError:
            content = raw_content.decode("utf-8", errors="ignore")

    except requests.RequestException:
        # The caller reports the failure, so the progress bar stays intact.
        return None
    else:
        return content


def clean_title(title):
    """Clean and filter title text."""
    if not title:
        return ""

    title = CDATA_RE.sub(r"\1", title)
    title = re.sub(r"\s+", " ", title.strip())

    # Skip likely navigation items or empty content
    if len(title) < MIN_TITLE_LENGTH:
        return ""

    if title.isupper() and len(title) < 50:
        return ""

    if BREADCRUMB_RE.match(title) or ALL_CAPS_RE.match(title):
        return ""

    return title


def extract_titles(xml_content):
    """Extract titles from RSS XML content."""
    try:
        root = defused_fromstring(xml_content)
    except (ET.ParseError, DefusedXmlException):
        # A hostile feed must not take the whole run down, so it is skipped like
        # any other unreadable feed. DefusedXmlException is not a ParseError.
        return []

    # RSS uses <item>, Atom uses <entry>. Atom also namespaces its tags, so match
    # on the local name instead of hardcoding the namespace of each feed.
    items = root.findall(".//item") or [el for el in root.iter() if el.tag.rpartition("}")[2] == "entry"]

    titles = []
    for item in items:
        title_elem = next((el for el in item if el.tag.rpartition("}")[2] == "title"), None)
        if title_elem is not None and title_elem.text:
            cleaned = clean_title(title_elem.text)
            if cleaned:
                titles.append(cleaned)
    return titles


def save_titles(titles, output_dir):
    """Write titles one per line into a timestamped file, replacing older runs."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    for old_file in output_dir.glob("titles_*.txt"):
        old_file.unlink(missing_ok=True)

    filepath = output_dir / f"titles_{datetime.now().strftime('%Y%m%d_%H%M%S')}.txt"
    filepath.write_text("".join(f"{title}\n" for title in titles), encoding="utf-8")
    return filepath


def main():
    """Main function to scrape all feeds and save titles to one file."""
    print_art()

    all_titles = []

    console.print("[bold white]Starting scraper...[/bold white]")

    with Progress(
        SpinnerColumn(style="bold white"),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(bar_width=None, style="white", complete_style="bold white", finished_style="bold white"),
        TaskProgressColumn(style="bold white"),
        TimeElapsedColumn(),
        console=console,
    ) as progress:
        task = progress.add_task("[bold white]Scraping feeds...", total=len(FEEDS))

        for feed_name, url in FEEDS.items():
            progress.update(task, description=f"[white]Scraping {feed_name}...")

            xml_content = fetch_rss_feed(url)
            if xml_content:
                titles = extract_titles(xml_content)
                all_titles.extend(titles)
                # We can print individual successes if we want, but it might clutter the progress bar area
                # console.print(f"[green]✓ {feed_name}: {len(titles)} titles[/green]")
            else:
                console.print(f"[bold white]✗ Failed to fetch {feed_name}[/bold white]")

            progress.advance(task)

    console.print(
        f"[bold white]Scraping finished![/bold white] Found [bold white]{len(all_titles)}[/bold white] titles total."
    )

    if not all_titles:
        console.print("[bold white]No titles found.[/bold white]")
        return

    output_dir = Path(__file__).resolve().parent / "scraped_output"
    try:
        filepath = save_titles(all_titles, output_dir)
    except OSError as e:
        console.print(f"[bold white]Error saving file:[/bold white] {e}")
        return

    console.print(
        Panel(
            f"Saved {len(all_titles)} titles to:\n[bold white]{filepath}[/bold white]",
            title="Success",
            border_style="white",
        )
    )


if __name__ == "__main__":
    main()
