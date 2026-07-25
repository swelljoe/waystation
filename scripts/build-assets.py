#!/usr/bin/env python3
"""Build deterministic, runtime-safe Waystation assets.

Purchased packs remain in the gitignored source directory. The generated output
contains only project-authored motifs plus selected runtime sprites permitted by
the pack licenses. Run without purchased packs to produce a complete public
fallback build.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "assets-manifest.json"
PALETTE = ["#f1dfad", "#c39a5b", "#77543b", "#34322a", "#78966b", "#9d5f55"]
CARD_SIZE = (96, 64)
TERRAIN_TILE_SIZE = 32

# Compact runtime atlas order. These coordinates select only the pieces the
# engine understands from the private 42x12 `THE GROUND` source sheet.
#
# 0..3 grass variants
# 4..17 dirt dual grid: full, single corners, edges, then three corners
# 18..34 water: center, isolated/caps, outer corners, edges, inner corners
GROUND_ATLAS_TILES = [
    (0, 0),
    (0, 0),
    (0, 1),
    (1, 0),
    (0, 2),
    (0, 2),
    (5, 1),
    (5, 2),
    (4, 0),
    (5, 0),
    (6, 2),
    (7, 2),
    (6, 3),
    (7, 3),
    (5, 3),
    (4, 3),
    (4, 2),
    (4, 1),
    (40, 0),
    (32, 1),
    (33, 1),
    (32, 0),
    (33, 0),
    (34, 0),
    (35, 0),
    (34, 1),
    (35, 1),
    (38, 1),
    (36, 0),
    (38, 0),
    (36, 1),
    (37, 1),
    (39, 1),
    (37, 0),
    (39, 0),
]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def px(draw: ImageDraw.ImageDraw, box: tuple[int, int, int, int], color: str) -> None:
    draw.rectangle(box, fill=color)


def draw_road_lamp(variant: int) -> Image.Image:
    image = Image.new("RGBA", CARD_SIZE, PALETTE[0])
    draw = ImageDraw.Draw(image)
    px(draw, (0, 44, 95, 63), PALETTE[2])
    px(draw, (0, 40, 95, 44), PALETTE[1])
    for offset in range(0, 96, 16):
        px(draw, (offset, 42, offset + 8, 43), PALETTE[0])
    px(draw, (46, 16, 49, 45), PALETTE[3])
    px(draw, (40, 12, 55, 18), PALETTE[3])
    px(draw, (42, 14, 53, 22), PALETTE[1])
    px(draw, (44, 15, 51, 20), PALETTE[0])
    if variant == 2:
        px(draw, (8, 28, 20, 39), PALETTE[4])
        px(draw, (75, 26, 88, 39), PALETTE[4])
    elif variant == 3:
        for x in (14, 76):
            px(draw, (x, 31, x + 2, 40), PALETTE[5])
            px(draw, (x - 3, 28, x + 5, 33), PALETTE[5])
    return image


def draw_shelter_tree(variant: int) -> Image.Image:
    image = Image.new("RGBA", CARD_SIZE, PALETTE[0])
    draw = ImageDraw.Draw(image)
    px(draw, (0, 48, 95, 63), PALETTE[2])
    px(draw, (44, 25, 52, 51), PALETTE[2])
    for box in [(30, 17, 54, 37), (43, 10, 67, 34), (22, 25, 45, 41)]:
        px(draw, box, PALETTE[4])
    px(draw, (38, 41, 42, 51), PALETTE[3])
    px(draw, (55, 42, 59, 51), PALETTE[3])
    px(draw, (40, 39, 57, 44), PALETTE[3])
    if variant == 2:
        px(draw, (73, 16, 77, 20), PALETTE[5])
        px(draw, (78, 13, 82, 17), PALETTE[5])
    elif variant == 3:
        px(draw, (9, 38, 24, 47), PALETTE[1])
    return image


def draw_open_hands(variant: int) -> Image.Image:
    image = Image.new("RGBA", CARD_SIZE, PALETTE[0])
    draw = ImageDraw.Draw(image)
    px(draw, (0, 52, 95, 63), PALETTE[2])
    left = [(12, 35, 39, 42), (20, 28, 27, 39), (29, 24, 35, 39), (37, 27, 43, 42)]
    right = [(56, 35, 83, 42), (68, 28, 75, 39), (60, 24, 66, 39), (52, 27, 58, 42)]
    for box in left + right:
        px(draw, box, PALETTE[1])
    px(draw, (40, 40, 55, 45), PALETTE[1])
    symbol_color = PALETTE[5] if variant == 1 else PALETTE[4]
    px(draw, (46, 14, 49, 28), symbol_color)
    px(draw, (40, 19, 55, 22), symbol_color)
    if variant == 3:
        px(draw, (44, 12, 51, 15), PALETTE[2])
    return image


def write_card_art(output: Path) -> list[dict[str, object]]:
    cards = output / "card"
    cards.mkdir(parents=True, exist_ok=True)
    records = []
    makers = [draw_road_lamp, draw_shelter_tree, draw_open_hands]
    for motif_index, maker in enumerate(makers, start=1):
        for variant in range(1, 4):
            image = maker(variant)
            path = cards / f"illustration_{motif_index}_{variant}.png"
            image.save(path, optimize=False)
            records.append(
                {
                    "path": str(path.relative_to(output)),
                    "sha256": sha256(path),
                    "size": list(image.size),
                    "source": "project-authored procedural motif",
                }
            )
    contact = Image.new("RGBA", (CARD_SIZE[0] * 3, CARD_SIZE[1] * 3), "#111111")
    for index, record in enumerate(records):
        image = Image.open(output / str(record["path"]))
        contact.paste(image, ((index % 3) * CARD_SIZE[0], (index // 3) * CARD_SIZE[1]))
    contact.save(output / "card-contact-sheet.png", optimize=False)
    return records


def patterned_tile(base: str, detail: str, seed: int) -> Image.Image:
    image = Image.new("RGBA", (32, 32), base)
    draw = ImageDraw.Draw(image)
    for y in range(32):
        for x in range(32):
            if (x * 17 + y * 31 + seed) % 29 == 0:
                px(draw, (x, y, x + 1, y + 1), detail)
    return image


def fallback_transition_tile(
    base: Image.Image, terrain: Image.Image, shape: str
) -> Image.Image:
    """Create readable public fallback art for a semantic transition piece."""
    mask = Image.new("L", (TERRAIN_TILE_SIZE, TERRAIN_TILE_SIZE), 0)
    draw = ImageDraw.Draw(mask)
    full = (0, 0, TERRAIN_TILE_SIZE - 1, TERRAIN_TILE_SIZE - 1)
    if shape == "center":
        draw.rectangle(full, fill=255)
    elif shape == "outer_nw":
        draw.rounded_rectangle((5, 5, 40, 40), radius=10, fill=255)
    elif shape == "outer_ne":
        draw.rounded_rectangle((-9, 5, 26, 40), radius=10, fill=255)
    elif shape == "outer_sw":
        draw.rounded_rectangle((5, -9, 40, 26), radius=10, fill=255)
    elif shape == "outer_se":
        draw.rounded_rectangle((-9, -9, 26, 26), radius=10, fill=255)
    elif shape == "edge_n":
        draw.rectangle((0, 5, 31, 31), fill=255)
    elif shape == "edge_e":
        draw.rectangle((0, 0, 26, 31), fill=255)
    elif shape == "edge_s":
        draw.rectangle((0, 0, 31, 26), fill=255)
    elif shape == "edge_w":
        draw.rectangle((5, 0, 31, 31), fill=255)
    elif shape.startswith("inner_"):
        draw.rectangle(full, fill=255)
        cutouts = {
            "inner_nw": (-9, -9, 10, 10),
            "inner_ne": (21, -9, 40, 10),
            "inner_se": (21, 21, 40, 40),
            "inner_sw": (-9, 21, 10, 40),
        }
        draw.ellipse(cutouts[shape], fill=0)
    elif shape == "isolated":
        draw.rounded_rectangle((5, 5, 26, 26), radius=8, fill=255)
    elif shape == "small_isolated":
        draw.ellipse((9, 9, 22, 22), fill=255)
    elif shape == "cap_w":
        draw.rounded_rectangle((5, 8, 40, 24), radius=8, fill=255)
    elif shape == "cap_e":
        draw.rounded_rectangle((-9, 8, 26, 24), radius=8, fill=255)
    else:
        raise ValueError(f"unknown fallback terrain shape: {shape}")
    return Image.composite(terrain, base, mask)


def fallback_dual_grid_tile(
    base: Image.Image, terrain: Image.Image, dirt_mask: int
) -> Image.Image:
    """Draw one four-corner dual-grid mask (NW, NE, SE, SW bits)."""
    mask = Image.new("L", (TERRAIN_TILE_SIZE, TERRAIN_TILE_SIZE), 0)
    draw = ImageDraw.Draw(mask)
    quadrants = [
        (1, (0, 0, 15, 15)),
        (2, (16, 0, 31, 15)),
        (4, (16, 16, 31, 31)),
        (8, (0, 16, 15, 31)),
    ]
    for bit, box in quadrants:
        if dirt_mask & bit:
            draw.rectangle(box, fill=255)
    return Image.composite(terrain, base, mask)


def build_terrain_atlas(source: Path) -> tuple[Image.Image, str]:
    ground_path = source / "THE GROUND/The Ground - 1-1.png"
    if ground_path.is_file():
        sheet = Image.open(ground_path).convert("RGBA")
        tiles = [
            sheet.crop(
                (
                    x * TERRAIN_TILE_SIZE,
                    y * TERRAIN_TILE_SIZE,
                    (x + 1) * TERRAIN_TILE_SIZE,
                    (y + 1) * TERRAIN_TILE_SIZE,
                )
            )
            for x, y in GROUND_ATLAS_TILES
        ]
        source_label = "licensed THE GROUND runtime extraction"
    else:
        grass = patterned_tile("#34482c", "#526e42", 3)
        dirt = patterned_tile("#66533e", "#80694c", 7)
        water = patterned_tile("#2f6670", "#4a8490", 11)
        grass_variants = [
            patterned_tile("#34482c", "#526e42", seed) for seed in (3, 9, 17, 25)
        ]
        dirt_masks = [15, 15, 1, 2, 4, 8, 3, 6, 12, 9, 14, 13, 11, 7]
        water_shapes = [
            "center",
            "isolated",
            "small_isolated",
            "cap_w",
            "cap_e",
            "outer_nw",
            "outer_ne",
            "outer_sw",
            "outer_se",
            "edge_n",
            "edge_e",
            "edge_s",
            "edge_w",
            "inner_nw",
            "inner_ne",
            "inner_se",
            "inner_sw",
        ]
        tiles = grass_variants
        tiles.extend(fallback_dual_grid_tile(grass, dirt, mask) for mask in dirt_masks)
        tiles.extend(fallback_transition_tile(grass, water, shape) for shape in water_shapes)
        source_label = "project-authored procedural fallback"

    atlas = Image.new(
        "RGBA", (len(tiles) * TERRAIN_TILE_SIZE, TERRAIN_TILE_SIZE), (0, 0, 0, 0)
    )
    for index, tile in enumerate(tiles):
        atlas.paste(tile, (index * TERRAIN_TILE_SIZE, 0))
    return atlas, source_label


def crop_tile(sheet: Image.Image, tile_id: int) -> Image.Image:
    columns = sheet.width // 32
    x = (tile_id % columns) * 32
    y = (tile_id // columns) * 32
    return sheet.crop((x, y, x + 32, y + 32)).convert("RGBA")


def draw_scribe() -> Image.Image:
    image = Image.new("RGBA", (32, 48), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    px(draw, (10, 5, 21, 10), "#4b4334")
    px(draw, (7, 10, 24, 25), "#6e6045")
    px(draw, (11, 11, 20, 20), "#c39a75")
    px(draw, (13, 14, 14, 15), "#34322a")
    px(draw, (18, 14, 19, 15), "#34322a")
    px(draw, (8, 24, 23, 39), "#8f815a")
    px(draw, (5, 27, 10, 40), "#8f815a")
    px(draw, (22, 27, 27, 40), "#8f815a")
    px(draw, (10, 39, 15, 47), "#34322a")
    px(draw, (18, 39, 23, 47), "#34322a")
    px(draw, (22, 25, 24, 34), "#d6c28e")
    return image


def draw_tree() -> Image.Image:
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    px(draw, (28, 37, 35, 61), "#60452f")
    for box in [(12, 17, 39, 45), (25, 7, 52, 40), (5, 27, 31, 51)]:
        px(draw, box, "#526e42")
    for box in [(20, 13, 31, 24), (37, 17, 48, 29), (12, 34, 24, 44)]:
        px(draw, box, "#78966b")
    return image


def write_world_art(source: Path, output: Path) -> list[dict[str, object]]:
    world = output / "world"
    world.mkdir(parents=True, exist_ok=True)
    terrain_path = source / "Modern_Farm_v1.2/32x32/1_Terrains_32x32.png"
    if terrain_path.is_file():
        sheet = Image.open(terrain_path).convert("RGBA")
        tiles = {
            "grass.png": crop_tile(sheet, 67),
            "road.png": crop_tile(sheet, 33),
            "water.png": crop_tile(sheet, 49),
        }
        tile_source = "licensed Modern Farm runtime extraction"
    else:
        tiles = {
            "grass.png": patterned_tile("#34482c", "#526e42", 3),
            "road.png": patterned_tile("#66533e", "#80694c", 7),
            "water.png": patterned_tile("#2f6670", "#4a8490", 11),
        }
        tile_source = "project-authored procedural fallback"
    tiles["stone.png"] = patterned_tile("#59594f", "#77766a", 13)
    tiles["floor.png"] = patterned_tile("#3f3428", "#594634", 17)
    for name, image in tiles.items():
        image.save(world / name, optimize=False)

    private_tree = source / "THE NATURAL/Props/Tree 08.png"
    tree = Image.open(private_tree).convert("RGBA") if private_tree.is_file() else draw_tree()
    tree.save(world / "tree.png", optimize=False)
    terrain_atlas, terrain_atlas_source = build_terrain_atlas(source)
    terrain_atlas.save(world / "terrain.png", optimize=False)
    draw_scribe().save(world / "scribe.png", optimize=False)
    records = []
    for path in sorted(world.glob("*.png")):
        records.append(
            {
                "path": str(path.relative_to(output)),
                "sha256": sha256(path),
                "size": list(Image.open(path).size),
                "source": (
                    "licensed runtime extraction"
                    if path.name == "tree.png" and private_tree.is_file()
                    else terrain_atlas_source
                    if path.name == "terrain.png"
                    else tile_source if path.name in {"grass.png", "road.png", "water.png"}
                    else "project-authored procedural fallback"
                ),
            }
        )
    return records


def verify_private_sources(source: Path, manifest: dict[str, object], strict: bool) -> list[dict[str, object]]:
    checks = []
    for pack in manifest["packs"]:
        required = source / pack["required_file"]
        present = required.is_file()
        result = {"id": pack["id"], "present": present, "path": str(required)}
        expected = pack.get("sha256")
        if present and expected:
            actual = sha256(required)
            result["sha256"] = actual
            if actual != expected:
                raise SystemExit(f"hash mismatch for {required}: {actual}")
        if strict and not present:
            raise SystemExit(f"required purchased asset is missing: {required}")
        checks.append(result)
    return checks


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, default=ROOT / "runtime-assets")
    parser.add_argument("--strict-private", action="store_true")
    args = parser.parse_args()

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    args.output.mkdir(parents=True, exist_ok=True)
    checks = verify_private_sources(args.source, manifest, args.strict_private)
    generated = write_card_art(args.output)
    generated.extend(write_world_art(args.source, args.output))
    report = {
        "schema_version": 1,
        "private_checks": checks,
        "generated": generated,
    }
    report_path = args.output / "provenance.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(generated)} runtime assets and {report_path}")


if __name__ == "__main__":
    main()
