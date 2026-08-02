#!/usr/bin/env python3
"""Check hand-edited LPC art before it reaches the game.

The failure this exists to catch is silent. Palette-recoloured art is drawn in
exactly the six colours of its material's base ramp, and the game swaps those
six for another six at draw time. A pixel painted in any other colour is not
recoloured — it keeps whatever it was, so one anti-aliased edge becomes an
orange speck that survives onto every brunette and every grey-haired elder. You
would not see it until you rendered a colour you were not looking at, possibly
after editing three hundred frames.

Anti-aliasing is the usual culprit, which is why alpha is checked too: a soft
brush leaves a fringe of part-transparent pixels that no palette can fix.

    python3 scripts/check-lpc-art.py assets/custom/lpc/spritesheets/hair/scribe_long/adult
    python3 scripts/check-lpc-art.py <dir> --material hair --like hair/long_messy/adult
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LPC = Path.home() / "src" / "Universal-LPC-Spritesheet-Character-Generator"

# Alpha values LPC art actually uses: invisible, the flat cast shadow, and solid.
# Anything else is almost always a soft brush or an anti-aliased edge.
EXPECTED_ALPHA = {0, 64, 255}


def ramp_for(lpc: Path, material: str) -> list[tuple[int, int, int]]:
    """The six colours a material's art must be drawn in to be recolourable.

    Which six comes from the material's own metadata — `base` names the palette
    entry the sprites are drawn in, and `default` names the version it lives in.
    """
    meta_path = lpc / "palette_definitions" / material / f"meta_{material}.json"
    if not meta_path.is_file():
        raise SystemExit(f"unknown material '{material}' — no {meta_path}")
    meta = json.loads(meta_path.read_text())
    version, base = meta["default"], meta["base"]
    if "." in base:
        version, base = base.split(".", 1)
    palette = json.loads((lpc / "palette_definitions" / material / f"{material}_{version}.json").read_text())
    if base not in palette:
        raise SystemExit(f"{material}: no '{base}' entry in the {version} palette")
    return [tuple(int(value.lstrip("#")[i : i + 2], 16) for i in (0, 2, 4)) for value in palette[base]]


def check_sheet(path: Path, ramp: set, complaints: list[str]) -> collections.Counter:
    with Image.open(path) as opened:
        image = opened.convert("RGBA")
    if image.width % 64 or image.height % 64:
        complaints.append(f"{path.name}: {image.width}x{image.height} is not a whole number of 64px frames")

    alpha_seen: collections.Counter = collections.Counter()
    off_ramp: collections.Counter = collections.Counter()
    for red, green, blue, alpha in image.get_flattened_data():
        alpha_seen[alpha] += 1
        if alpha == 255 and (red, green, blue) not in ramp:
            off_ramp[(red, green, blue)] += 1

    for colour, count in off_ramp.most_common():
        complaints.append(
            f"{path.name}: {count} opaque pixels of #{colour[0]:02x}{colour[1]:02x}{colour[2]:02x} "
            "are off the ramp and will not recolour"
        )
    for alpha, count in sorted(alpha_seen.items()):
        if alpha not in EXPECTED_ALPHA:
            complaints.append(
                f"{path.name}: {count} pixels at alpha {alpha} — LPC art uses only "
                f"{sorted(EXPECTED_ALPHA)}, so this is probably anti-aliasing"
            )
    return alpha_seen


def compare_to_reference(art: Path, reference: Path, complaints: list[str]) -> None:
    """Every animation the original had, at the size the original had it.

    A missing or resized sheet is invisible in the game — the layer simply does
    not draw for that animation — so it is worth catching here rather than
    wondering later why a character stops having hair when they sit down.
    """
    def sizes(folder: Path) -> dict[str, tuple[int, int]]:
        found = {}
        for path in sorted(folder.glob("*.png")):
            with Image.open(path) as image:
                found[path.name] = image.size
        return found

    theirs, mine = sizes(reference), sizes(art)
    for name, size in sorted(theirs.items()):
        if name not in mine:
            complaints.append(f"missing {name}, which the reference art has")
        elif mine[name] != size:
            complaints.append(f"{name}: {mine[name]} but the reference art has {size}")
    for name in sorted(set(mine) - set(theirs)):
        complaints.append(f"{name} is not an animation the reference art has — misnamed?")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("art", type=Path, help="directory of finished sheets to check")
    parser.add_argument("--lpc", type=Path, default=DEFAULT_LPC)
    parser.add_argument("--material", default="hair", help="palette the art must be drawn in")
    parser.add_argument("--like", help="LPC art this was derived from, e.g. hair/long_messy/adult")
    args = parser.parse_args()

    if not args.art.is_dir():
        raise SystemExit(f"{args.art} is not a directory")
    sheets = sorted(args.art.glob("*.png"))
    if not sheets:
        raise SystemExit(f"no PNGs in {args.art}")

    ramp = ramp_for(args.lpc, args.material)
    complaints: list[str] = []
    alpha_total: collections.Counter = collections.Counter()
    for sheet in sheets:
        alpha_total += check_sheet(sheet, set(ramp), complaints)

    if args.like:
        reference = args.lpc / "spritesheets" / args.like
        if not reference.is_dir():
            raise SystemExit(f"no such reference art: {reference}")
        compare_to_reference(args.art, reference, complaints)

    print(f"{len(sheets)} sheets in {args.art}")
    print(f"{args.material} ramp: {' '.join(f'#{r:02x}{g:02x}{b:02x}' for r, g, b in ramp)}")
    print("alpha: " + ", ".join(f"{a}×{n}" for a, n in sorted(alpha_total.items())))

    if complaints:
        print(f"\n{len(complaints)} problems:")
        for complaint in complaints:
            print(f"  {complaint}")
        raise SystemExit(1)
    print("\nclean — every opaque pixel is on the ramp and will recolour")


if __name__ == "__main__":
    sys.exit(main())
