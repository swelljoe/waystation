#!/usr/bin/env python3
"""Draw generated travellers as a contact sheet.

`npc-preview` writes Universal LPC character files; this composites them so a
whole cast can be judged at a glance instead of one browser tab at a time. It
follows the same rules the LPC web app does — layer paths out of the sheet
definitions, `${head}`-style path substitution, palette recolouring against the
material palettes — against a local checkout of the generator.

    cargo run -p waystation-npcgen --bin npc-preview -- --count 24
    python3 scripts/preview-npcs.py

`--verify` checks the compositing itself: it renders a hand-made character and
searches for the result inside the sheet the web app exported for them. An exact
hit means every layer, order and recolour here matches the real thing.

    python3 scripts/preview-npcs.py --verify assets/custom/old-guy.txt \\
        assets/custom/old-guy.png
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LPC = Path.home() / "src" / "Universal-LPC-Spritesheet-Character-Generator"
DEFAULT_IN = ROOT / "target/npc-preview"
DEFAULT_OUT = ROOT / "target/npc-preview/contact-sheet.png"

FRAME = 64
# LPC animation sheets run up, left, down, right. Facing the viewer is the one
# worth reviewing, and its first frame is the standing pose.
FACING_SOUTH_ROW = 2
STANDING_FRAME = 0
ANIMATION = "walk"


class Catalog:
    """The LPC checkout: sheet definitions, palettes, and the sprites."""

    def __init__(self, lpc: Path) -> None:
        self.root = lpc
        self.definitions: dict[str, dict] = {}
        for path in sorted((lpc / "sheet_definitions").rglob("*.json")):
            if not path.name.startswith("meta_"):
                self.definitions[path.stem] = json.loads(path.read_text())

        self.materials: dict[str, dict] = {}
        for meta in sorted((lpc / "palette_definitions").glob("*/meta_*.json")):
            material = meta.parent.name
            entry = json.loads(meta.read_text())
            entry["palettes"] = {
                version.stem.split("_", 1)[1]: json.loads(version.read_text())
                for version in sorted(meta.parent.glob("*.json"))
                if not version.name.startswith("meta_")
            }
            self.materials[material] = entry

    def recolor_slots(self, definition: dict) -> list[dict]:
        """Normalise the two shapes `recolors` comes in.

        A piece with one colour states its material inline; a piece with more
        than one (hair plus its tie, a head plus its eyes) nests them under
        `color_1`, `color_2`. Only the first is ever chosen by the generator —
        the rest keep their default and are left alone here, exactly as the web
        app leaves them until someone picks.
        """
        recolors = definition.get("recolors")
        if not recolors:
            return []
        if "material" in recolors:
            return [recolors]
        return [recolors[key] for key in sorted(recolors)]

    def palette(self, material: str, version: str | None, color: str) -> list[str] | None:
        meta = self.materials.get(material)
        if not meta:
            return None
        if version and version in meta["palettes"]:
            found = meta["palettes"][version].get(color)
            if found:
                return found
        # The generator only ever names a bare colour, so fall back to any
        # version of this material that has one by that name.
        for palette in meta["palettes"].values():
            if color in palette:
                return palette[color]
        return None


def name_without_color(selection: dict) -> str:
    """The piece's own name, with the colour the generator appended stripped.

    `${head}` path substitution keys off this, and the exported name carries the
    colour in brackets: `Human Male Elderly (amber)`.
    """
    return selection["name"].split(" (")[0]


def substitute(path: str, selections: dict, definition: dict) -> str | None:
    """Resolve `${type}` placeholders against the other selections.

    Faces live under a directory named for the head wearing them, so a path
    reads `head/faces/${head}/sad/` and only becomes real once the head is
    known. A placeholder with no mapping means this combination has no art.
    """
    replacements = definition.get("replace_in_path", {})
    while "${" in path:
        start = path.index("${")
        end = path.index("}", start)
        type_name = path[start + 2 : end]
        selection = selections.get(type_name)
        if not selection:
            return None
        key = name_without_color(selection).replace(" ", "_")
        value = replacements.get(type_name, {}).get(key)
        if not value:
            return None
        path = path[:start] + value + path[end + 1 :]
    return path


def recolor(image: Image.Image, mappings: list[tuple[list[str], list[str]]]) -> Image.Image:
    """Swap one palette for another, pixel by pixel.

    The web app does this on the GPU with a 1-pixel tolerance; exact matching is
    enough here because the source sprites are drawn in the palette colours
    rather than resampled into them.
    """

    def rgb(value: str) -> tuple[int, int, int]:
        value = value.lstrip("#")
        return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16))

    table: dict[tuple[int, int, int], tuple[int, int, int]] = {}
    for source, target in mappings:
        for from_hex, to_hex in zip(source, target):
            table[rgb(from_hex)] = rgb(to_hex)
    if not table:
        return image

    out = image.copy()
    pixels = out.load()
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = pixels[x, y]
            if a == 0:
                continue
            swap = table.get((r, g, b))
            if swap:
                pixels[x, y] = (*swap, a)
    return out


def layers_for(catalog: Catalog, character: dict) -> list[tuple[int, int, Image.Image]]:
    """Every sprite this character draws, as `(zPos, order, frame)`."""
    body_type = character["bodyType"]
    selections = character["selections"]
    drawn: list[tuple[int, int, Image.Image]] = []

    for order, (_, selection) in enumerate(selections.items()):
        definition = catalog.definitions.get(selection["itemId"])
        if not definition:
            print(f"  ? no definition for {selection['itemId']}")
            continue

        mappings: list[tuple[list[str], list[str]]] = []
        for slot in catalog.recolor_slots(definition):
            chosen = selection.get("recolor")
            # Only the piece's own colour is ever chosen; nested slots such as
            # eye colour or a hair tie keep whatever they were drawn in.
            if not chosen or slot is not catalog.recolor_slots(definition)[0]:
                continue
            material = slot["material"]
            base = slot.get("base") or catalog.materials[material]["base"]
            version = catalog.materials[material]["default"]
            if "." in base:
                version, base = base.split(".", 1)
            source = slot.get("source") or catalog.palette(material, version, base)
            target = catalog.palette(material, version, chosen)
            if source and target:
                mappings.append((source, target))
            else:
                print(f"  ? no {material} palette for {chosen} on {selection['itemId']}")

        for index in range(1, 9):
            layer = definition.get(f"layer_{index}")
            if not layer:
                continue
            base_path = layer.get(body_type)
            if not base_path:
                # Not an error: LPC simply has no art for this piece on this
                # base, and the app draws nothing. The Rust generator avoids
                # picking these, so anything reaching here is worth knowing.
                print(f"  ? {selection['itemId']} has no {body_type} art")
                continue
            base_path = substitute(base_path, selections, definition)
            if base_path is None:
                print(f"  ? {selection['itemId']} has no art for this head")
                continue

            variant = selection.get("variant") or ""
            suffix = f"/{variant.replace(' ', '_')}" if variant else ""
            sprite = catalog.root / "spritesheets" / f"{base_path}{ANIMATION}{suffix}.png"
            if not sprite.is_file():
                print(f"  ? missing sprite {sprite.relative_to(catalog.root)}")
                continue

            sheet = Image.open(sprite).convert("RGBA")
            frame = sheet.crop(
                (
                    STANDING_FRAME * FRAME,
                    FACING_SOUTH_ROW * FRAME,
                    (STANDING_FRAME + 1) * FRAME,
                    (FACING_SOUTH_ROW + 1) * FRAME,
                )
            )
            drawn.append((layer["zPos"], order, recolor(frame, mappings) if mappings else frame))

    return drawn


def render(catalog: Catalog, character: dict) -> Image.Image:
    """One traveller, standing and facing the viewer."""
    canvas = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    for _, _, frame in sorted(layers_for(catalog, character), key=lambda l: (l[0], l[1])):
        canvas.alpha_composite(frame)
    return canvas


def contact_sheet(people: list[tuple[str, Image.Image]], columns: int, scale: int) -> Image.Image:
    cell = FRAME * scale
    caption = 12
    rows = (len(people) + columns - 1) // columns
    sheet = Image.new("RGBA", (columns * cell, rows * (cell + caption)), (32, 30, 28, 255))
    draw = ImageDraw.Draw(sheet)
    for index, (label, art) in enumerate(people):
        x = (index % columns) * cell
        y = (index // columns) * (cell + caption)
        sheet.alpha_composite(art.resize((cell, cell), Image.NEAREST), (x, y))
        draw.text((x + 3, y + cell), label, fill=(190, 185, 175, 255))
    return sheet


def verify(catalog: Catalog, character_path: Path, sheet_path: Path) -> bool:
    """Look for the rendered frame inside an sheet the web app exported.

    If the composite is right it appears verbatim somewhere in the full action
    sheet, because both were built from the same sprites in the same order.
    """
    character = json.loads(character_path.read_text())
    frame = render(catalog, character)
    sheet = Image.open(sheet_path).convert("RGBA")
    needle = frame.tobytes()
    for row in range(sheet.height // FRAME):
        for column in range(sheet.width // FRAME):
            box = (column * FRAME, row * FRAME, (column + 1) * FRAME, (row + 1) * FRAME)
            if sheet.crop(box).tobytes() == needle:
                print(f"exact match at column {column}, row {row} of {sheet_path.name}")
                return True
    print(f"no match anywhere in {sheet_path.name} — compositing differs from the web app")
    return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lpc", type=Path, default=DEFAULT_LPC, help="LPC generator checkout")
    parser.add_argument("--input", type=Path, default=DEFAULT_IN, help="folder of character files")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--columns", type=int, default=6)
    parser.add_argument("--scale", type=int, default=3)
    parser.add_argument(
        "--verify",
        nargs=2,
        metavar=("CHARACTER", "SHEET"),
        type=Path,
        help="render a character and find it inside an exported action sheet",
    )
    args = parser.parse_args()

    if not (args.lpc / "spritesheets").is_dir():
        raise SystemExit(f"no spritesheets under {args.lpc} — pass --lpc")
    catalog = Catalog(args.lpc)

    if args.verify:
        raise SystemExit(0 if verify(catalog, *args.verify) else 1)

    files = sorted(args.input.glob("npc-*.json"))
    if not files:
        raise SystemExit(f"no npc-*.json in {args.input} — run npc-preview first")

    people = []
    for path in files:
        print(path.name)
        people.append((path.stem.replace("npc-", ""), render(catalog, json.loads(path.read_text()))))

    args.output.parent.mkdir(parents=True, exist_ok=True)
    contact_sheet(people, args.columns, args.scale).save(args.output)
    print(f"\n{len(people)} travellers → {args.output}")


if __name__ == "__main__":
    main()
