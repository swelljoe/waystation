#!/usr/bin/env python3
"""Put the wardrobe's sprites where the running game can reach them.

`build-npc-wardrobe.py` decides which LPC pieces a traveller may be made of and
records, for each one, the sprite directories it draws from and the palette it
recolours against. That record is committed. The art it points at is not — it
lives in a checkout of the LPC generator — so this copies the sheets the
wardrobe actually names into the runtime tree, and nothing else.

Only the walk sheets are copied. The game draws visitors from the walk rows and
nothing else: they approach, they stand on frame 0, they leave. Copying the
other fifty rows would multiply the payload by an order of magnitude for frames
that never reach a screen.

    python3 scripts/build-npc-art.py
    python3 scripts/build-npc-art.py --lpc ~/src/Universal-LPC-...

Without a checkout this writes nothing and says so; the game falls back to the
hand-made visitor sheets, which `build-assets.py` always produces.

Reference sheets are written alongside the art. They are composited here, in
Python, from the same wardrobe record the game reads, and the game's own
compositor is tested against them byte-for-byte — so a disagreement about layer
order, `${head}` substitution or palette recolouring fails a test rather than
producing a traveller with someone else's face.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LPC = Path.home() / "src" / "Universal-LPC-Spritesheet-Character-Generator"
OVERLAY = ROOT / "assets/custom/lpc"
WARDROBE = ROOT / "crates/npcgen/data/wardrobe.json"
CREDITS_MD = ROOT / "docs/NPC_ART_CREDITS.md"
OUTPUT = ROOT / "runtime-assets/npc"

# The one animation the game plays, and the shape every LPC sheet of it has.
ANIMATION = "walk"
FRAME = 64
COLUMNS = 9
ROWS = 4
SHEET_SIZE = (COLUMNS * FRAME, ROWS * FRAME)

# The order slots are chosen in, which is also the order pieces are drawn in
# when two of them claim the same zPos. It mirrors `generate_for` in
# crates/npcgen/src/lib.rs; the reference sheets are only a fair test of the
# game's compositor if the tie-break matches.
SLOT_ORDER = [
    "head",
    "body",
    "hair",
    "beard",
    "mustache",
    "eyebrows",
    "expression",
    "nose",
    "wrinkles",
    "clothes",
    "legs",
    "shoes",
    "apron",
    "overalls",
    "belt",
    "sash",
    "neck",
    "headcover",
    "bandana",
    "hat",
    "backpack",
    "weapon",
]


# --- Reading the wardrobe --------------------------------------------------


def substitutions(item: dict, path: str) -> list[str]:
    """Every real path a `${type}`-templated layer path can become."""
    paths = [path]
    for key, table in item.get("replace", {}).items():
        placeholder = "${" + key + "}"
        grown = []
        for candidate in paths:
            if placeholder not in candidate:
                grown.append(candidate)
                continue
            grown += [candidate.replace(placeholder, value) for value in sorted(set(table.values()))]
        paths = grown
    return paths


def sheets_for(item: dict, path: str) -> list[str]:
    """The sheet files one resolved layer directory contributes.

    A piece recoloured at draw time has one sheet; a piece that ships baked
    colours has one per colour it offers, and the wardrobe already narrowed
    those to the colours this game rolls.
    """
    if item["field"] == "variant":
        return [f"{path}{ANIMATION}/{option.replace(' ', '_')}.png" for option in item["options"]]
    return [f"{path}{ANIMATION}.png"]


def wanted_files(wardrobe: dict) -> dict[str, str]:
    """Every sheet the wardrobe names, mapped to where it comes from.

    The value is `"lpc"` or `"overlay"` — a piece drawn for this project
    resolves against our own tree, not the upstream checkout.
    """
    found: dict[str, str] = {}
    for slot in wardrobe["slots"].values():
        for item in slot["items"]:
            origin = item.get("origin", "lpc")
            for layer in item["layers"]:
                for path in layer["paths"].values():
                    for resolved in substitutions(item, path):
                        for sheet in sheets_for(item, resolved):
                            found[sheet] = origin
    return found


# --- Compositing -----------------------------------------------------------
#
# The same rules the LPC web app follows, and the same ones the game follows.
# Kept here so the game's version has something independent to be checked
# against; a compositor tested only against itself proves nothing.


def rgb(value: str) -> tuple[int, int, int]:
    value = value.lstrip("#")
    return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16))


def swap_table(wardrobe: dict, item: dict, colour: str) -> dict:
    """What this piece's palette becomes once a colour is rolled for it."""
    recolor = item.get("recolor")
    if not recolor or not colour:
        return {}
    target = wardrobe["materials"].get(recolor["material"], {}).get(colour)
    if not target:
        return {}
    return {rgb(a): rgb(b) for a, b in zip(recolor["from"], target)}


def recolour(image: Image.Image, table: dict) -> Image.Image:
    if not table:
        return image
    out = image.copy()
    pixels = out.load()
    for y in range(out.height):
        for x in range(out.width):
            red, green, blue, alpha = pixels[x, y]
            if alpha == 0:
                continue
            swap = table.get((red, green, blue))
            if swap:
                pixels[x, y] = (*swap, alpha)
    return out


def resolve(path: str, item: dict, character: list[dict], wardrobe: dict) -> str | None:
    """Fill in `${type}` from whatever the character chose for that slot.

    Faces live under a directory named for the head above them, so their layer
    path is only real once the head is known. No mapping means this pairing has
    no art, which the app renders as nothing at all.
    """
    while "${" in path:
        start = path.index("${")
        end = path.index("}", start)
        key = path[start + 2 : end]
        chosen = next((piece for piece in character if piece["slot"] == key), None)
        if not chosen:
            return None
        name = item_named(wardrobe, key, chosen["item"])["name"].replace(" ", "_")
        value = item.get("replace", {}).get(key, {}).get(name)
        if not value:
            return None
        path = path[:start] + value + path[end + 1 :]
    return path


def item_named(wardrobe: dict, slot: str, item_id: str) -> dict:
    for item in wardrobe["slots"][slot]["items"]:
        if item["id"] == item_id:
            return item
    raise SystemExit(f"{slot}: no wardrobe item named {item_id}")


def render(wardrobe: dict, art: Path, character: dict) -> Image.Image:
    """One traveller's walk sheet, layers stacked bottom to top."""
    body = character["body"]
    drawn: list[tuple[int, int, Image.Image]] = []
    for order, piece in enumerate(character["pieces"]):
        item = item_named(wardrobe, piece["slot"], piece["item"])
        table = swap_table(wardrobe, item, piece.get("color", ""))
        for layer in item["layers"]:
            path = layer["paths"].get(body)
            if not path:
                continue
            path = resolve(path, item, character["pieces"], wardrobe)
            if path is None:
                continue
            colour = piece.get("color", "")
            if item["field"] == "variant":
                sheet = art / f"{path}{ANIMATION}/{colour.replace(' ', '_')}.png"
            else:
                sheet = art / f"{path}{ANIMATION}.png"
            if not sheet.is_file():
                raise SystemExit(f"{piece['item']}: {sheet} is not in the runtime tree")
            with Image.open(sheet) as opened:
                frame = opened.convert("RGBA")
            drawn.append((layer["z"], order, recolour(frame, table)))

    canvas = Image.new("RGBA", SHEET_SIZE, (0, 0, 0, 0))
    for _, _, frame in sorted(drawn, key=lambda entry: (entry[0], entry[1])):
        canvas.alpha_composite(frame)
    return canvas


# --- Reference characters --------------------------------------------------


def reference_characters(wardrobe: dict) -> list[dict]:
    """A small deterministic cast, built to exercise the compositor.

    Not plausible travellers — that is the Rust generator's job — but drawable
    ones covering the parts a compositor can get wrong: templated face paths,
    palette recolouring, baked colour variants, and two pieces claiming the same
    zPos. Taking the first and the last fitting item in every slot gets there
    without a hand-written cast that would drift the moment the allowlist moves.
    """
    cast = []
    for base in wardrobe["body_types"]:
        for end, label in ((0, "first"), (-1, "last")):
            pieces = []
            for slot in SLOT_ORDER:
                spec = wardrobe["slots"].get(slot)
                if not spec:
                    continue
                fitting = [item for item in spec["items"] if base["id"] in item["bodies"]]
                if not fitting:
                    continue
                item = fitting[end]
                pieces.append({"slot": slot, "item": item["id"], "color": colour_for(wardrobe, item, end)})
            cast.append({"name": f"{base['id']}-{label}", "body": base["id"], "pieces": pieces})
    return cast


def colour_for(wardrobe: dict, item: dict, end: int) -> str:
    """A colour this piece can actually take, from whichever end is asked for."""
    source = item["source"]
    if source == "none":
        return ""
    if source == "fixed":
        return item["options"][end]
    palette = {"skin": "skin", "hair": "hair", "cloth": "cloth_muted"}[source]
    offered = [entry["color"] for entry in wardrobe["palettes"][palette]]
    if item["field"] == "variant":
        offered = [colour for colour in offered if colour in item["options"]]
    return offered[end] if offered else ""


# --- Building --------------------------------------------------------------


def copy_art(wardrobe: dict, lpc: Path, output: Path) -> tuple[int, int]:
    """Copy every named sheet, and remove any that is no longer named.

    Pruning matters more than it looks: a piece cut from the allowlist for a
    licensing reason would otherwise keep shipping its art, credited to nobody,
    for as long as nobody thought to clear the directory by hand.
    """
    roots = {"lpc": lpc / "spritesheets", "overlay": OVERLAY / "spritesheets"}
    wanted = wanted_files(wardrobe)
    copied = 0
    for name, origin in sorted(wanted.items()):
        source = roots[origin] / name
        if not source.is_file():
            raise SystemExit(f"{name} is named by the wardrobe but missing from {roots[origin]}")
        destination = output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        if not destination.is_file() or destination.stat().st_size != source.stat().st_size:
            shutil.copyfile(source, destination)
            copied += 1

    keep = {output / name for name in wanted}
    removed = 0
    for existing in sorted(output.rglob("*.png")):
        if existing not in keep and "reference" not in existing.parts:
            existing.unlink()
            removed += 1
    for folder in sorted(output.rglob("*"), reverse=True):
        if folder.is_dir() and not any(folder.iterdir()):
            folder.rmdir()
    return len(wanted), copied, removed


def as_ulpc_character(wardrobe: dict, character: dict) -> dict:
    """The same character in the shape the LPC web app stores, for comparison."""
    selections = {}
    for piece in character["pieces"]:
        item = item_named(wardrobe, piece["slot"], piece["item"])
        colour = piece.get("color", "")
        variant = colour if item["field"] == "variant" else ""
        recolor = colour if item["field"] == "recolor" else ""
        name = f"{item['name']} ({colour})" if colour else item["name"]
        selections[piece["slot"]] = {
            "itemId": item["id"],
            "variant": variant,
            "recolor": recolor,
            "name": name,
        }
    return {"bodyType": character["body"], "selections": selections}


def check_against_catalog(wardrobe: dict, output: Path, lpc: Path) -> str:
    """Composite the reference cast a second way, and insist on the same pixels.

    The first way reads the committed wardrobe and the copied runtime art. The
    second reads the LPC checkout directly, through `preview-npcs.py`, whose
    output has already been checked byte-for-byte against sheets the web app
    exported itself.

    Agreeing means the wardrobe's recorded layer paths, palette ramps and
    stacking numbers still say what the catalogue says. Drift there is the kind
    of thing that shows up as one traveller in forty wearing the wrong colour,
    months later, with nothing to point at.
    """
    spec = importlib.util.spec_from_file_location(
        "waystation_preview_npcs", Path(__file__).with_name("preview-npcs.py")
    )
    if not spec or not spec.loader:
        return "could not load preview-npcs.py; reference sheets unchecked"
    preview = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(preview)
    catalog = preview.Catalog(lpc)

    checked = 0
    for character in reference_characters(wardrobe):
        theirs = preview.render(catalog, as_ulpc_character(wardrobe, character))
        with Image.open(output / "reference" / f"{character['name']}.png") as opened:
            mine = opened.convert("RGBA")
        # `preview-npcs.py` draws the standing frame only: south-facing, frame 0.
        frame = mine.crop((0, 2 * FRAME, FRAME, 3 * FRAME))
        here, there = frame.tobytes(), theirs.tobytes()
        if here != there:
            wrong = sum(
                1
                for mine_pixel, their_pixel in zip(
                    [here[at : at + 4] for at in range(0, len(here), 4)],
                    [there[at : at + 4] for at in range(0, len(there), 4)],
                )
                if mine_pixel != their_pixel
            )
            raise SystemExit(
                f"{character['name']}: {wrong} pixels differ between the committed "
                "wardrobe and the LPC catalogue itself. Rerun `make wardrobe` — the "
                "recorded layer paths, palette ramps or stacking numbers have drifted."
            )
        checked += 1
    return f"{checked} reference sheets match the LPC catalogue exactly"


def write_references(wardrobe: dict, output: Path) -> int:
    reference = output / "reference"
    reference.mkdir(parents=True, exist_ok=True)
    for stale in reference.glob("*"):
        stale.unlink()
    cast = reference_characters(wardrobe)
    for character in cast:
        render(wardrobe, output, character).save(reference / f"{character['name']}.png")
    (reference / "cast.json").write_text(json.dumps(cast, indent=2) + "\n")
    return len(cast)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lpc", type=Path, default=DEFAULT_LPC)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()

    wardrobe = json.loads(WARDROBE.read_text())
    if not (args.lpc / "spritesheets").is_dir():
        print(f"no spritesheets under {args.lpc} — skipping generated visitor art")
        print("the game will fall back to the hand-made visitor sheets")
        return

    wanted, copied, removed = copy_art(wardrobe, args.lpc, args.output)
    references = write_references(wardrobe, args.output)
    agreement = check_against_catalog(wardrobe, args.output, args.lpc)

    # The art carries attribution obligations wherever it goes, so the credits
    # travel with it rather than living only in the source tree.
    if CREDITS_MD.is_file():
        shutil.copyfile(CREDITS_MD, args.output / "CREDITS.md")
    (args.output / "provenance.json").write_text(
        json.dumps(
            {
                "lpc_revision": wardrobe["lpc_revision"],
                "license_policy": wardrobe["license_policy"],
                "animation": ANIMATION,
                "sheets": wanted,
                "credits": "CREDITS.md",
            },
            indent=2,
        )
        + "\n"
    )
    print(f"{wanted} walk sheets in {args.output} ({copied} written, {removed} pruned)")
    print(f"{references} reference sheets for the game's compositor to be checked against")
    print(agreement)


if __name__ == "__main__":
    main()
