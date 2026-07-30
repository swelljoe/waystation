#!/usr/bin/env python3
"""Append one reviewed verse and illustration brief to the Scribe print catalog."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "content" / "prints.json"
SLUG_PATTERN = re.compile(r"^[a-z0-9]+(?:[-_][a-z0-9]+)*$")


def slug(value: str) -> str:
    if not SLUG_PATTERN.fullmatch(value):
        raise argparse.ArgumentTypeError(
            "use lowercase letters, numbers, hyphens, or underscores"
        )
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--id", type=slug, dest="print_id")
    parser.add_argument("--title")
    parser.add_argument("--theme", type=slug)
    parser.add_argument("--reference", help="Human-readable Bible reference.")
    parser.add_argument("--verse", help="Exact reviewed verse text.")
    parser.add_argument("--art-prompt", help="Wordless illustration subject and action.")
    parser.add_argument("--stage", type=slug, default="early_monochrome")
    parser.add_argument("--note")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def required(value: str | None, label: str) -> str:
    if value is not None and value.strip():
        return value.strip()
    if not sys.stdin.isatty():
        raise SystemExit(f"missing --{label.lower().replace(' ', '-')}")
    while True:
        answer = input(f"{label}: ").strip()
        if answer:
            return answer


def make_entry(
    *,
    print_id: str,
    title: str,
    theme: str,
    reference: str,
    verse: str,
    art_prompt: str,
    stage: str = "early_monochrome",
    note: str | None = None,
) -> dict[str, str]:
    entry = {
        "id": print_id,
        "title": title,
        "theme": theme,
        "reference": reference,
        "verse": verse,
        "art": f"assets/prints/{print_id}-art.png",
        "card": f"assets/prints/{print_id}-card.png",
        "stage": stage,
        "art_prompt": art_prompt,
    }
    if note:
        entry["note"] = note
    return entry


def append_entry(manifest: dict[str, object], entry: dict[str, str]) -> None:
    prints = manifest.get("prints")
    if not isinstance(prints, list):
        raise ValueError("manifest must contain a prints array")
    for existing in prints:
        if not isinstance(existing, dict):
            raise ValueError("every prints entry must be an object")
        for key in ("id", "art", "card"):
            if existing.get(key) == entry[key]:
                raise ValueError(f"duplicate {key}: {entry[key]}")
    prints.append(entry)


def write_json_atomic(path: Path, manifest: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o644
    temporary_name = ""
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_name = temporary.name
            json.dump(manifest, temporary, indent=2, ensure_ascii=False)
            temporary.write("\n")
        os.chmod(temporary_name, mode)
        os.replace(temporary_name, path)
    finally:
        if temporary_name and os.path.exists(temporary_name):
            os.unlink(temporary_name)


def main() -> None:
    args = parse_args()
    print_id = required(args.print_id, "ID")
    try:
        print_id = slug(print_id)
    except argparse.ArgumentTypeError as error:
        raise SystemExit(f"invalid ID: {error}") from error
    theme = required(args.theme, "Theme")
    try:
        theme = slug(theme)
    except argparse.ArgumentTypeError as error:
        raise SystemExit(f"invalid theme: {error}") from error

    entry = make_entry(
        print_id=print_id,
        title=required(args.title, "Title"),
        theme=theme,
        reference=required(args.reference, "Reference"),
        verse=required(args.verse, "Verse"),
        art_prompt=required(args.art_prompt, "Art prompt"),
        stage=args.stage,
        note=args.note.strip() if args.note else None,
    )
    manifest_path = args.manifest.resolve()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    append_entry(manifest, entry)

    if args.dry_run:
        print(json.dumps(entry, indent=2, ensure_ascii=False))
        return
    write_json_atomic(manifest_path, manifest)
    print(f"added {entry['id']} to {manifest_path.relative_to(ROOT)}")
    print(f"next: python3 scripts/generate-print-art.py {entry['id']}")


if __name__ == "__main__":
    main()
