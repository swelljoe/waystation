#!/usr/bin/env python3
"""Split an LPC sprite into editable layers, and put it back together.

LPC ships flat PNGs — there is no layered master for `long_messy` or for almost
anything else in the catalogue, upstream or down. There does not need to be: the
layers separate cleanly on alpha alone, because the two things in a hair sheet
are drawn differently on purpose.

The hair itself is fully opaque and uses exactly the six colours of its
material's base palette ramp, so the game can swap it to any of twenty-six hair
colours at draw time. The shadow it casts on the forehead is a flat black at
alpha 64, deliberately *off* the ramp so that it stays a shadow rather than
turning orange on a redhead.

Separating them means the hair layer can be indexed to six colours, which locks
a pencil to the palette and makes it impossible to paint a pixel that will not
recolour. The shadow stays out of the way in RGBA where indexing would destroy
it. Rejoining is exact — `split` proves it on every file before writing.

    python3 scripts/lpc-art-split.py split \\
        ~/src/Universal-LPC-.../spritesheets/hair/long_messy/adult \\
        assets/custom/lpc/spritesheets/hair/scribe_long/_work

    python3 scripts/lpc-art-split.py join \\
        assets/custom/lpc/spritesheets/hair/scribe_long/_work \\
        assets/custom/lpc/spritesheets/hair/scribe_long/adult
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image

# Where each layer lands under the working directory.
OPAQUE = "hair"
PARTIAL = "shadow"


def split_image(image: Image.Image) -> tuple[Image.Image, Image.Image]:
    """Divide a sprite into its fully opaque and partially transparent halves.

    The two are disjoint and together cover every visible pixel, which is what
    makes rejoining exact rather than approximate.
    """
    opaque = Image.new("RGBA", image.size, (0, 0, 0, 0))
    partial = Image.new("RGBA", image.size, (0, 0, 0, 0))
    for y in range(image.height):
        for x in range(image.width):
            pixel = image.getpixel((x, y))
            if pixel[3] == 255:
                opaque.putpixel((x, y), pixel)
            elif pixel[3] != 0:
                partial.putpixel((x, y), pixel)
    return opaque, partial


def shadow_tones(partial: Image.Image) -> set[tuple[int, int, int, int]]:
    """The distinct visible colours on the shadow layer, for reporting."""
    return {
        partial.getpixel((x, y))
        for y in range(partial.height)
        for x in range(partial.width)
        if partial.getpixel((x, y))[3]
    }


def join_images(opaque: Image.Image, partial: Image.Image) -> Image.Image:
    """Recombine the halves. Opaque art sits over the shadow, never under it."""
    out = partial.copy()
    out.alpha_composite(opaque)
    return out


def split(source: Path, destination: Path) -> int:
    sheets = sorted(source.glob("*.png"))
    if not sheets:
        raise SystemExit(f"no PNGs in {source}")
    (destination / OPAQUE).mkdir(parents=True, exist_ok=True)
    (destination / PARTIAL).mkdir(parents=True, exist_ok=True)

    for sheet in sheets:
        with Image.open(sheet) as opened:
            original = opened.convert("RGBA")
        opaque, partial = split_image(original)

        # Refuse to write a split that cannot be undone. An edit made on top of
        # a lossy split would be unrecoverable, and the loss would show up as a
        # subtly wrong sprite rather than an error.
        if join_images(opaque, partial).tobytes() != original.tobytes():
            raise SystemExit(f"{sheet.name}: split does not rejoin exactly, refusing to write")

        opaque.save(destination / OPAQUE / sheet.name)
        partial.save(destination / PARTIAL / sheet.name)
        note = f"shadow {shades}" if (shades := shadow_tones(partial)) else "no shadow"
        print(f"  {sheet.name:16} {note}")
    return len(sheets)


def join(source: Path, destination: Path) -> int:
    opaque_dir, partial_dir = source / OPAQUE, source / PARTIAL
    if not opaque_dir.is_dir():
        raise SystemExit(f"no {OPAQUE}/ under {source} — is this a split working directory?")
    destination.mkdir(parents=True, exist_ok=True)

    sheets = sorted(opaque_dir.glob("*.png"))
    for sheet in sheets:
        with Image.open(sheet) as opened:
            opaque = opened.convert("RGBA")
        shadow_path = partial_dir / sheet.name
        if shadow_path.is_file():
            with Image.open(shadow_path) as opened:
                partial = opened.convert("RGBA")
            if partial.size != opaque.size:
                raise SystemExit(f"{sheet.name}: layers are different sizes, {opaque.size} vs {partial.size}")
        else:
            # A frame with nothing cast on the face is legitimate; `climb` has
            # no shadow at all.
            partial = Image.new("RGBA", opaque.size, (0, 0, 0, 0))
        join_images(opaque, partial).save(destination / sheet.name)
        print(f"  {sheet.name}")
    return len(sheets)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("split", "join"))
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()

    if not args.source.is_dir():
        raise SystemExit(f"{args.source} is not a directory")

    if args.action == "split":
        count = split(args.source, args.destination)
        print(f"\n{count} sheets → {args.destination}/{OPAQUE} and /{PARTIAL}")
        print(f"edit {OPAQUE}/ indexed to the six ramp colours; leave {PARTIAL}/ in RGBA")
    else:
        count = join(args.source, args.destination)
        print(f"\n{count} sheets → {args.destination}")
        print("check it with: python3 scripts/check-lpc-art.py", args.destination)


if __name__ == "__main__":
    sys.exit(main())
