"""Turn a reference somebody wrote into the id an API will answer to.

`Hebrews 13:2` is a thing an English reader looks up. `HEB.13.2` is a thing a
server asks for, in any language, and it is what both the card catalog and the
runtime translation path key on. Two scripts need the same answer, so the
mapping lives in one place rather than in whichever of them was edited last.
"""

from __future__ import annotations

import re


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
    """A reference this will not guess at."""


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
