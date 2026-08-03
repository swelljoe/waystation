#!/usr/bin/env python3
"""Replace the verse text in the content files with text fetched from YouVersion.

The wording in `content/prints.json` and `content/readings.json` was typed from
memory and never checked against a source. This fetches every reference from a
named version and writes back exactly what the API returned, so the text in the
game is text somebody can point at.

The illustrations are untouched. They carry no words by design, so changing
translation is a `make prints` recomposite and not a regeneration.

Excerpting is the one place this cannot be fully automatic. A card carries a
phrase — a few words, the way a block cut by hand does — and the API returns
whole verses, so every card's wording is a literal span of one, chosen by a
person and preserved across reruns. A card with no reviewed span comes back
whole and is reported, because guessing at a clause boundary in Scripture is
not something a script should do quietly. Readings keep whole verses, and the
older `Matthew 12:20a` form still names the sentence to take.
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

from bible_reference import ReferenceError, usfm


ROOT = Path(__file__).resolve().parents[1]
PRINTS = ROOT / "content" / "prints.json"
READINGS = ROOT / "content" / "readings.json"
VERSIONS = ROOT / "content" / "bible-versions.json"
PASSAGE_URL = "https://api.youversion.com/v1/bibles/{version}/passages/{reference}"


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


def excerpt(
    text: str,
    part: str | None,
    existing: str | None = None,
    *,
    span_only: bool = False,
) -> tuple[str, bool]:
    """The words a reference names, and whether a human still has to look.

    An `existing` excerpt that is a literal span of the fetched verse is a cut
    somebody already reviewed, so it survives a rerun. Otherwise a verse that
    divides into sentences divides cleanly, and a verse that is one sentence
    does not — that one comes back whole rather than cut at a comma and hoped
    over.

    `span_only` is what a card holds: a phrase of a few words rather than a
    verse, because a block cut by hand carries a phrase and because a whole
    verse in a language that renders longer than English will not fit the panel.
    There is no rule for finding that phrase, so nothing here invents one — a
    print with no reviewed cut comes back whole and is reported, which is a
    failed `make prints` rather than an unread excerpt on a card.
    """
    # A *proper* span only. Text equal to the whole verse is what this function
    # wrote last time when it could not divide it, not a cut anybody approved.
    trimmed = (existing or "").strip()
    reviewed = bool(trimmed) and trimmed != text and trimmed in text
    if span_only:
        return (trimmed, False) if reviewed else (text, True)
    if part is None:
        return text, False
    if reviewed:
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


def update(
    entries: list[dict], app_key: str, version: int, *, span_only: bool = False
) -> list[dict]:
    """Rewrite each entry's verse in place; report what changed and what to read.

    A card also gets its USFM id written back, because that — not the printed
    reference — is what the server asks YouVersion for when a traveler arrives
    speaking something other than English. Deriving it here keeps the two from
    drifting apart.
    """
    report = []
    for entry in entries:
        verse_id, part = usfm(entry["reference"])
        if span_only:
            entry["passage_id"] = verse_id
        whole = fetch(app_key, version, verse_id)
        text, needs_review = excerpt(whole, part, entry.get("verse"), span_only=span_only)
        report.append(
            {
                "id": entry["id"],
                "reference": entry["reference"],
                "usfm": verse_id,
                "was": entry["verse"],
                "now": text,
                "whole_verse": whole if part or span_only else None,
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
            reports[key] = update(
                document[key], args.app_key, version, span_only=key == "prints"
            )
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
        print(f"{len(review)} reference(s) need a human excerpt:")
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
