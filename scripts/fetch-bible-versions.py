#!/usr/bin/env python3
"""Rebuild the reviewed language-to-Bible-version map from the YouVersion catalog.

The server never discovers versions at runtime. It reads a committed file, the
same way every other authored thing in this project works, so that which
translation a player is handed stays a reviewable decision and not whatever the
catalog happened to return that morning.

A version is only eligible if it contains every book `content/passages.ron`
draws from. Many entries in the catalog are New Testament only, and one of those
chosen for a Psalm would hand the traveler a 404 in their own language.
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
DEFAULT_PASSAGES = ROOT / "content" / "passages.ron"
DEFAULT_OUTPUT = ROOT / "content" / "bible-versions.json"
CATALOG_URL = "https://api.youversion.com/v1/bibles"

# What the server falls back to when a player's language has no eligible
# version. English, because the authored reflections around it are English too.
FALLBACK_LANGUAGE = "en"
FALLBACK_ABBREVIATION = "BSB"

# Languages whose pick is a project decision rather than a ranking. English is
# pinned to BSB because the reviewed wording in `content/passages.ron` is BSB;
# ranking on book count alone would hand English a deuterocanonical edition and
# quietly disagree with every fixture and every offline fallback.
PREFERRED_VERSIONS = {FALLBACK_LANGUAGE: FALLBACK_ABBREVIATION}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--passages", type=Path, default=DEFAULT_PASSAGES)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--app-key",
        default=os.environ.get("YVP_APP_KEY"),
        help="YouVersion app key; defaults to YVP_APP_KEY.",
    )
    parser.add_argument(
        "--catalog",
        type=Path,
        help="Read a saved catalog JSON array instead of calling the API.",
    )
    parser.add_argument("--dry-run", action="store_true", help="Report without writing.")
    return parser.parse_args()


def required_books(passages_ron: str) -> list[str]:
    """The books the authored passages actually reach for, in USFM form.

    Parsed from the RON rather than listed here, so adding a passage from a new
    book cannot silently leave a language mapped to a version missing it.
    """
    books = {match.group(1) for match in re.finditer(r'id:\s*"([A-Z0-9]{3})\.', passages_ron)}
    if not books:
        raise ValueError("no passage ids found; refusing to build an unconstrained map")
    return sorted(books)


def fetch_catalog(app_key: str) -> list[dict]:
    """Every version the key can reach, following the page tokens to the end."""
    versions: list[dict] = []
    token: str | None = None
    while True:
        query = [("language_ranges[]", "*")]
        if token:
            query.append(("page_token", token))
        request = urllib.request.Request(
            f"{CATALOG_URL}?{urllib.parse.urlencode(query)}",
            headers={"X-YVP-App-Key": app_key},
        )
        with urllib.request.urlopen(request, timeout=60) as response:
            payload = json.load(response)
        versions.extend(payload.get("data", []))
        token = payload.get("next_page_token")
        if not token:
            return versions


def eligible(version: dict, books: list[str]) -> bool:
    available = set(version.get("books") or [])
    return all(book in available for book in books)


def rank(version: dict) -> tuple[int, int, int]:
    """A pinned pick first, then the fullest Bible, then the lowest id.

    The id tie-break is what keeps a rebuild from churning the file when the
    catalog returns equally complete versions in a different order.
    """
    pinned = PREFERRED_VERSIONS.get(version.get("language_tag", "")) == version["abbreviation"]
    return (0 if pinned else 1, -len(version.get("books") or []), int(version["id"]))


def entry_for(version: dict) -> dict:
    """Every entry carries its own language, alternatives included.

    The runtime flattens the picks and the alternatives into one list so that
    `YVP_BIBLE_ID` can name any version in the catalog. An alternative that did
    not know its own language would come back out of that list unattributable.
    """
    return {
        "id": int(version["id"]),
        "abbreviation": version["abbreviation"],
        "title": version.get("localized_title") or version.get("title") or "",
        "language": version["language_tag"],
    }


def build_map(versions: list[dict], books: list[str]) -> dict:
    """One reviewed pick per language, with the rejected candidates kept visible.

    `alternatives` exists so changing a pick is an edit to this file rather than
    a rerun against a catalog that may have moved underneath it.
    """
    by_language: dict[str, list[dict]] = {}
    for version in versions:
        if not eligible(version, books):
            continue
        language = version.get("language_tag")
        if not language:
            continue
        by_language.setdefault(language, []).append(version)

    languages = []
    for language in sorted(by_language):
        ranked = sorted(by_language[language], key=rank)
        chosen = entry_for(ranked[0])
        chosen["alternatives"] = [entry_for(other) for other in ranked[1:]]
        languages.append(chosen)

    fallback = next(
        (
            language
            for language in languages
            if language["language"] == FALLBACK_LANGUAGE
            and language["abbreviation"] == FALLBACK_ABBREVIATION
        ),
        None,
    )
    if fallback is None:
        raise ValueError(
            f"the catalog no longer offers {FALLBACK_ABBREVIATION} for "
            f"{FALLBACK_LANGUAGE}; the server has nothing to fall back to"
        )

    return {
        "schema_version": 1,
        "note": (
            "Which translation a player is handed, by language tag. Rebuilt with "
            "`make bible-versions`, then reviewed. Only versions containing every "
            "book content/passages.ron draws from are eligible, so a New "
            "Testament edition is never chosen for a Psalm. Move an entry out of "
            "`alternatives` to change a pick; the rebuild keeps it."
        ),
        "required_books": books,
        "fallback": {
            "language": FALLBACK_LANGUAGE,
            "abbreviation": FALLBACK_ABBREVIATION,
        },
        "languages": languages,
    }


def main() -> int:
    args = parse_args()
    books = required_books(args.passages.read_text(encoding="utf-8"))

    if args.catalog:
        versions = json.loads(args.catalog.read_text(encoding="utf-8"))
    elif args.app_key:
        try:
            versions = fetch_catalog(args.app_key)
        except urllib.error.HTTPError as error:
            print(f"YouVersion returned {error.code}: {error.read()[:200]!r}", file=sys.stderr)
            return 1
    else:
        print("set YVP_APP_KEY or pass --app-key or --catalog", file=sys.stderr)
        return 1

    mapping = build_map(versions, books)
    languages = mapping["languages"]
    print(
        f"{len(versions)} versions in the catalog; "
        f"{sum(1 + len(item['alternatives']) for item in languages)} carry {', '.join(books)}; "
        f"{len(languages)} languages mapped"
    )
    if args.dry_run:
        return 0

    args.output.write_text(json.dumps(mapping, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.output.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
