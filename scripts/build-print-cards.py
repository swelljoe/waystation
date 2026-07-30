#!/usr/bin/env python3
"""Build exact-text, deliberately low-resolution verse cards from print art."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "content" / "prints.json"
DEFAULT_FONT = ROOT / "open-assets" / "fonts" / "EBGaramond-Variable.ttf"
WORK_SIZE = (512, 768)
OUTPUT_SIZE = WORK_SIZE
TEXT_TOP = 557
TEXT_BOTTOM = 733
TEXT_MARGIN = 42
INK = (20, 19, 15)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--font", type=Path, default=DEFAULT_FONT)
    parser.add_argument(
        "--id",
        action="append",
        dest="print_ids",
        help="Build only this print ID; repeat to select several.",
    )
    return parser.parse_args()


def wrap_words(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.FreeTypeFont, width: int) -> list[str]:
    lines: list[str] = []
    current = ""
    for word in text.split():
        candidate = f"{current} {word}".strip()
        if current and draw.textbbox((0, 0), candidate, font=font)[2] > width:
            lines.append(current)
            current = word
        else:
            current = candidate
    if current:
        lines.append(current)
    return lines


def centered_text(draw: ImageDraw.ImageDraw, y: int, text: str, font: ImageFont.FreeTypeFont) -> int:
    box = draw.textbbox((0, 0), text, font=font)
    width = box[2] - box[0]
    draw.text(((WORK_SIZE[0] - width) // 2, y), text, font=font, fill=INK)
    return y + (box[3] - box[1])


def render_card(entry: dict[str, str], font_path: Path) -> None:
    source_path = ROOT / entry["art"]
    output_path = ROOT / entry["card"]
    with Image.open(source_path) as source:
        source = source.convert("RGB")
        source.thumbnail(WORK_SIZE, Image.Resampling.LANCZOS)
        canvas = Image.new("RGB", WORK_SIZE, source.getpixel((0, 0)))
        canvas.paste(source, ((WORK_SIZE[0] - source.width) // 2, 0))

    draw = ImageDraw.Draw(canvas)
    verse_font = ImageFont.truetype(str(font_path), 30)
    reference_font = ImageFont.truetype(str(font_path), 21)
    max_width = WORK_SIZE[0] - 2 * TEXT_MARGIN
    lines = wrap_words(draw, entry["verse"], verse_font, max_width)
    line_height = 38
    block_height = len(lines) * line_height + 32
    y = max(TEXT_TOP, TEXT_TOP + (TEXT_BOTTOM - TEXT_TOP - block_height) // 2)

    for line in lines:
        centered_text(draw, y, line, verse_font)
        y += line_height
    y += 7
    centered_text(draw, y, entry["reference"], reference_font)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(output_path, optimize=True)


def main() -> None:
    args = parse_args()
    manifest_path = args.manifest.resolve()
    font_path = args.font.resolve()
    with manifest_path.open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    entries = manifest["prints"]
    if args.print_ids:
        requested = set(args.print_ids)
        known = {entry["id"] for entry in entries}
        unknown = requested - known
        if unknown:
            raise SystemExit(f"unknown print ID(s): {', '.join(sorted(unknown))}")
        entries = [entry for entry in entries if entry["id"] in requested]
    for entry in entries:
        render_card(entry, font_path)
        print(f"wrote {entry['card']}")


if __name__ == "__main__":
    main()
