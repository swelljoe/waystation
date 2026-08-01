#!/usr/bin/env python3
"""Replace the verse text in the content files with text fetched from YouVersion.

The wording in `content/prints.json` and `content/readings.json` was typed from
memory and never checked against a source. This fetches every reference from a
named version and writes back exactly what the API returned, so the text in the
game is text somebody can point at.

The illustrations are untouched. They carry no words by design, so changing
translation is a `make prints` recomposite and not a regeneration.

Partial references — `Matthew 12:20a` — are the one place this cannot be fully
automatic. The API returns whole verses; the card holds about four lines. Where
the verse divides into sentences the matching sentence is taken; where it does
not, the whole verse is written and reported, because guessing at a clause
boundary in Scripture is not something a script should do quietly.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PRINTS = ROOT / "content" / "prints.json"
READINGS = ROOT / "content" / "readings.json"
VERSIONS = ROOT / "content" / "bible-versions.json"
PASSAGE_URL = "https://api.youversion.com/v1/bibles/{version}/passages/{reference}"

BOOKS = {
    "genesis": "GEN", "exodus": "EXO", "leviticus": "LEV", "numbers": "NUM",
    "deuteronomy": "DEU", "joshua": "JOS", "judges": "JDG", "ruth": "RUT",
    "1 samuel": "1SA", "2 samuel": "2SA", "1 kings": "1KI", "2 kings": "2KI",
    "1 chronicles": "1CH", "2 chronicles": "2CH", "ezra": "EZR",
    "nehemiah": "NEH", "esther": "EST", "job": "JOB", "psalm": "PSA",
    "psalms": "PSA", "proverbs": "PRO", "ecclesiastes": "ECC",
    "song of solomon": "SNG", "isaiah": "ISA", "jeremiah": "JER",
    "lamentations": "LAM", "ezekiel": "EZK", "daniel": "DAN", "hosea": "HOS",
    "joel": "JOL", "amos": "AMO", "obadiah": "OBA", "jonah": "JON",
    "micah": "MIC", "nahum": "NAM", "habakkuk": "HAB", "zephaniah": "ZEP",
    "haggai": "HAG", "zechariah": "ZEC", "malachi": "MAL",
    "matthew": "MAT", "mark": "MRK", "luke": "LUK", "john": "JHN",
    "acts": "ACT", "romans": "ROM", "1 corinthians": "1CO",
    "2 corinthians": "2CO", "galatians": "GAL", "ephesians": "EPH",
    "philippians": "PHP", "colossians": "COL", "1 thessalonians": "1TH",
    "2 thessalonians": "2TH", "1 timothy": "1TI", "2 timothy": "2TI",
    "titus": "TIT", "philemon": "PHM", "hebrews": "HEB", "james": "JAS",
    "1 peter": "1PE", "2 peter": "2PE", "1 john": "1JN", "2 john": "2JN",
    "3 john": "3JN", "jude": "JUD", "revelation": "REV",
}

# `Ecclesiastes 4:9–10` uses an en dash; `2 Corinthians 12:9a` carries a part
# marker. Both appear in the authored files and neither is valid in a USFM id.
REFERENCE = re.compile(
    r"^\s*(?P<book>[1-3]?\s*[A-Za-z][A-Za-z ]*?)\s+"
    r"(?P<chapter>\d+):(?P<verse>\d+)(?:\s*[-–—]\s*(?P<end>\d+))?"
    r"(?P<part>[ab])?\s*$"
)


class ReferenceError(ValueError):
    """A reference this script will not guess at."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--version",
        default="BSB",
        help="Abbreviation from content/bible-versions.json (default: BSB).",
    )
    parser.add_argument(
        "--app-key",
        default=os.environ.get("YVP_APP_KEY"),
        help="YouVersion app key; defaults to YVP_APP_KEY.",
    )
    parser.add_argument("--dry-run", action="store_true", help="Report without writing.")
    parser.add_argument("--prints", type=Path, default=PRINTS)
    parser.add_argument("--readings", type=Path, default=READINGS)
    return parser.parse_args()


def usfm(reference: str) -> tuple[str, str | None]:
    """`2 Corinthians 12:9a` -> (`2CO.12.9`, `a`).

    Returns the part marker separately because the API has no notion of one; the
    caller has to decide what half of a verse means.
    """
    match = REFERENCE.match(reference)
    if not match:
        raise ReferenceError(f"cannot parse reference: {reference!r}")
    book = re.sub(r"\s+", " ", match.group("book").strip().lower())
    code = BOOKS.get(book)
    if code is None:
        raise ReferenceError(f"unknown book in reference: {reference!r}")
    verse_id = f"{code}.{match.group('chapter')}.{match.group('verse')}"
    if match.group("end"):
        verse_id = f"{verse_id}-{match.group('end')}"
    return verse_id, match.group("part")


def sentences(text: str) -> list[str]:
    """Split on sentence ends, keeping the punctuation and any closing quote."""
    parts = re.split(r'(?<=[.!?])(?=\s)|(?<=[.!?][”"])(?=\s)', text)
    return [part.strip() for part in parts if part.strip()]


def clean(content: str) -> str:
    """Drop the editorial marks the API leaves in the text.

    `Matthew 6:34` comes back ending `own.[’’]` — a bracketed quotation-mark
    note, not words. Only bracket groups holding no letters or digits go; a
    bracketed clause is a textual-doubt marker around real Scripture and
    removing it would change what the verse says.
    """
    stripped = re.sub(r"\[[^\w\]]*\]", "", content)
    return re.sub(r"\s+", " ", stripped).strip()


def excerpt(text: str, part: str | None, existing: str | None = None) -> tuple[str, bool]:
    """The half a partial reference names, and whether a human still has to look.

    An `existing` excerpt that is a literal span of the fetched verse is a cut
    somebody already reviewed, so it survives a rerun. Otherwise a verse that
    divides into sentences divides cleanly, and a verse that is one sentence
    does not — that one comes back whole rather than cut at a comma and hoped
    over.
    """
    if part is None:
        return text, False
    # A *proper* span only. Text equal to the whole verse is what this function
    # wrote last time when it could not divide it, not a cut anybody approved.
    trimmed = (existing or "").strip()
    if trimmed and trimmed != text and trimmed in text:
        return trimmed, False
    divided = sentences(text)
    if len(divided) < 2:
        return text, True
    return (divided[0] if part == "a" else divided[-1]), False


def version_id(abbreviation: str) -> int:
    catalog = json.loads(VERSIONS.read_text(encoding="utf-8"))
    for entry in catalog["languages"]:
        for candidate in [entry, *entry.get("alternatives", [])]:
            if candidate["abbreviation"] == abbreviation:
                return int(candidate["id"])
    raise ValueError(f"{abbreviation} is not in content/bible-versions.json")


def fetch(app_key: str, version: int, reference: str) -> str:
    request = urllib.request.Request(
        PASSAGE_URL.format(version=version, reference=urllib.parse.quote(reference)),
        headers={"X-YVP-App-Key": app_key},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        payload = json.load(response)
    content = payload.get("content")
    if not content:
        raise ValueError(f"no content returned for {reference}")
    return clean(content)


def update(entries: list[dict], app_key: str, version: int) -> list[dict]:
    """Rewrite each entry's verse in place; report what changed and what to read."""
    report = []
    for entry in entries:
        verse_id, part = usfm(entry["reference"])
        whole = fetch(app_key, version, verse_id)
        text, needs_review = excerpt(whole, part, entry.get("verse"))
        report.append(
            {
                "id": entry["id"],
                "reference": entry["reference"],
                "usfm": verse_id,
                "was": entry["verse"],
                "now": text,
                "whole_verse": whole if part else None,
                "needs_review": needs_review,
            }
        )
        entry["verse"] = text
    return report


# Redistribution terms are the version's, not the API's, and only a human can
# have checked them. Anything not listed here says so rather than implying a
# freedom nobody confirmed.
LICENCE_NOTES = {
    "BSB": (
        "The Berean Standard Bible was dedicated to the public domain (CC0) on "
        "30 April 2023. Attribution appreciated, not required."
    ),
    "ASV": "American Standard Version (1901). Public domain.",
    "engWEBUS": "World English Bible. Public domain.",
    "enggnv": "Geneva Bible (1599). Public domain.",
}


def translation_block(abbreviation: str, version: int) -> dict:
    return {
        "id": abbreviation.lower(),
        "name": abbreviation,
        "youversion_id": version,
        "source": "YouVersion Platform API",
        "licence_note": LICENCE_NOTES.get(
            abbreviation,
            f"Check the licence for {abbreviation} before redistributing this text.",
        ),
    }


def main() -> int:
    args = parse_args()
    if not args.app_key:
        print("set YVP_APP_KEY or pass --app-key", file=sys.stderr)
        return 1
    version = version_id(args.version)

    reports: dict[str, list[dict]] = {}
    documents: dict[Path, dict] = {}
    for path, key in ((args.prints, "prints"), (args.readings, "readings")):
        document = json.loads(path.read_text(encoding="utf-8"))
        try:
            reports[key] = update(document[key], args.app_key, version)
        except (ReferenceError, urllib.error.HTTPError, ValueError) as error:
            print(f"{path.name}: {error}", file=sys.stderr)
            return 1
        document["translation"] = translation_block(args.version, version)
        documents[path] = document

    changed = 0
    for key, report in reports.items():
        print(f"\n=== {key} ===")
        for item in report:
            mark = "!" if item["needs_review"] else " "
            if item["was"] != item["now"]:
                changed += 1
            print(f" {mark} {item['reference']:<22} {item['usfm']:<14} {item['now'][:64]}")
    review = [item for report in reports.values() for item in report if item["needs_review"]]
    print(f"\n{changed} verses changed to {args.version}.")
    if review:
        print(f"{len(review)} partial reference(s) need a human excerpt:")
        for item in review:
            print(f"  {item['reference']}: {item['whole_verse']}")

    if args.dry_run:
        return 0
    for path, document in documents.items():
        path.write_text(
            json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        print(f"wrote {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
