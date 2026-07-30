#!/usr/bin/env python3
"""Generate missing Scribe print illustrations through resumable `codex exec` jobs."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "content" / "prints.json"
CARD_BUILDER = ROOT / "scripts" / "build-print-cards.py"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ids", nargs="*", help="Print IDs to generate; defaults to the catalog.")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--codex-bin", default="codex")
    parser.add_argument("--limit", type=int, help="Generate at most this many missing images.")
    parser.add_argument("--force", action="store_true", help="Explicitly replace existing art.")
    parser.add_argument("--keep-going", action="store_true", help="Continue after a failed job.")
    parser.add_argument("--dry-run", action="store_true", help="Print jobs without invoking Codex.")
    parser.add_argument("--no-cards", action="store_true", help="Do not compose verse cards afterward.")
    return parser.parse_args()


def within_root(relative_path: str) -> Path:
    path = (ROOT / relative_path).resolve()
    try:
        path.relative_to(ROOT)
    except ValueError as error:
        raise ValueError(f"print asset escapes the project root: {relative_path}") from error
    return path


def build_prompt(
    common_prompt: str, entry: dict[str, str], *, replace_existing: bool = False
) -> str:
    replacement = (
        "The caller explicitly used --force and authorized replacing the existing target PNG."
        if replace_existing
        else "The target is expected not to exist; do not replace any other asset."
    )
    return f"""$imagegen

Generate exactly one new image for the Waystation project.

Shared visual specification:
{common_prompt}

Subject for this card:
{entry['art_prompt']}

The image attached with --image is a style-and-format reference only. Generate a
brand-new composition; do not edit or overwrite the reference. Use the built-in
image-generation path. The source illustration itself must contain absolutely no
text because exact typography is added by a deterministic local compositor.

Save a copy of the final selected PNG at exactly:
{entry['art']}

{replacement}

Do not modify any other project file. Do not create the text-bearing card. Inspect
the saved PNG, verify that it is portrait-oriented and that its lower text panel
is empty, then finish by reporting the saved path.
"""


def verify_art(path: Path) -> None:
    if not path.is_file():
        raise RuntimeError(f"Codex did not create {path.relative_to(ROOT)}")
    try:
        with Image.open(path) as image:
            image.verify()
        with Image.open(path) as image:
            width, height = image.size
    except Exception as error:
        raise RuntimeError(f"generated file is not a readable image: {path}") from error
    if width < 256 or height < 384:
        raise RuntimeError(f"generated image is unexpectedly small: {width}x{height}")
    if height <= width:
        raise RuntimeError(f"generated image is not portrait-oriented: {width}x{height}")


def codex_command(codex_bin: str, reference: Path) -> list[str]:
    return [
        codex_bin,
        "exec",
        "--ephemeral",
        "--sandbox",
        "workspace-write",
        "--color",
        "never",
        "--cd",
        str(ROOT),
        "--image",
        str(reference),
        "-",
    ]


def compose_cards(manifest_path: Path, entries: list[dict[str, str]]) -> None:
    command = [sys.executable, str(CARD_BUILDER), "--manifest", str(manifest_path)]
    for entry in entries:
        command.extend(("--id", entry["id"]))
    subprocess.run(command, cwd=ROOT, check=True)


def main() -> None:
    args = parse_args()
    if args.limit is not None and args.limit < 1:
        raise SystemExit("--limit must be at least 1")
    manifest_path = args.manifest.resolve()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    generation = manifest["image_generation"]
    reference = within_root(generation["reference"])
    if not reference.is_file():
        raise SystemExit(f"missing style reference: {reference}")

    entries = manifest["prints"]
    if args.ids:
        requested = set(args.ids)
        known = {entry["id"] for entry in entries}
        unknown = requested - known
        if unknown:
            raise SystemExit(f"unknown print ID(s): {', '.join(sorted(unknown))}")
        entries = [entry for entry in entries if entry["id"] in requested]

    if not args.dry_run and shutil.which(args.codex_bin) is None:
        raise SystemExit(f"Codex CLI not found: {args.codex_bin}")

    attempted = 0
    completed = 0
    planned = 0
    failed = 0
    composable: list[dict[str, str]] = []
    for entry in entries:
        art_path = within_root(entry["art"])
        if art_path.exists() and not args.force:
            print(f"skip {entry['id']}: {entry['art']} already exists", flush=True)
            composable.append(entry)
            continue
        if args.limit is not None and attempted >= args.limit:
            print(f"defer {entry['id']}: generation limit reached", flush=True)
            continue

        prompt = build_prompt(
            generation["common_prompt"], entry, replace_existing=args.force
        )
        if args.dry_run:
            print(f"\n--- {entry['id']} -> {entry['art']} ---\n{prompt}")
            attempted += 1
            planned += 1
            continue

        art_path.parent.mkdir(parents=True, exist_ok=True)
        print(f"generate {entry['id']} -> {entry['art']}", flush=True)
        result = subprocess.run(
            codex_command(args.codex_bin, reference),
            cwd=ROOT,
            input=prompt,
            text=True,
            check=False,
        )
        attempted += 1
        try:
            if result.returncode != 0:
                raise RuntimeError(f"codex exec exited with status {result.returncode}")
            verify_art(art_path)
            composable.append(entry)
            completed += 1
        except RuntimeError as error:
            failed += 1
            print(f"error: {entry['id']}: {error}", file=sys.stderr)
            if not args.keep_going:
                raise SystemExit(1) from error

    if not args.dry_run and not args.no_cards and composable:
        compose_cards(manifest_path, composable)
    if args.dry_run:
        print(f"complete: {planned} planned, {failed} failed")
    else:
        print(
            f"complete: {completed} generated, "
            f"{len(composable)} composable, {failed} failed"
        )
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
