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
import re
import shutil
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "assets-manifest.json"
PALETTE = ["#f1dfad", "#c39a5b", "#77543b", "#34322a", "#78966b", "#9d5f55"]
CARD_SIZE = (96, 64)
TERRAIN_TILE_SIZE = 32
INTERIOR_ROOT = ROOT / "content/interiors"
BUILDING_ROOT = ROOT / "content/buildings"
REPAIR_PAIR_PATH = ROOT / "content/repair-pairs.json"
INTERIOR_LAYER_ORDER = {"floor": 0, "wall": 1, "object": 2, "overlay": 3}
INTERIOR_ID = re.compile(r"^[a-z0-9][a-z0-9_-]{0,63}$")

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
    (4, 0),
    (5, 0),
    (5, 1),
    (4, 1),
    (6, 2),
    (7, 2),
    (6, 3),
    (7, 3),
    (5, 3),
    (4, 3),
    (4, 2),
    (5, 2),
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

PROPS_SHEET = "Modern_Farm_v1.2/32x32/3_Props_and_Buildings_32x32.png"

# Pixel rectangles of the individual firewood and dry-plant props in the Modern
# Farm props sheet.
KINDLING_PIECES = {
    "log_left": (68, 714, 94, 732),
    "log_right": (98, 714, 124, 732),
    "log_long": (76, 744, 110, 766),
    "straw": (512, 170, 544, 190),
    "straw_bits": (548, 172, 570, 186),
    "twigs": (740, 72, 766, 88),
}

# The three gatherable kindling piles, each a canvas size plus the pieces to
# paste and their top-left offsets. Order runs from sound logs to loose tinder.
KINDLING_PILES = {
    "kindling_logs.png": (
        (48, 34),
        [("log_long", (0, 12)), ("log_right", (20, 0)), ("log_left", (4, 1))],
    ),
    "kindling_branches.png": (
        (48, 32),
        [("straw", (0, 10)), ("log_long", (12, 4)), ("straw_bits", (2, 2))],
    ),
    "kindling_tinder.png": (
        (46, 30),
        [("twigs", (16, 0)), ("straw", (0, 8)), ("straw_bits", (20, 14))],
    ),
}


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


def draw_fallback_log(draw: ImageDraw.ImageDraw, box: tuple[int, int, int, int]) -> None:
    x0, y0, x1, y1 = box
    radius = (y1 - y0) // 2
    draw.rounded_rectangle(box, radius=radius, fill="#77543b")
    draw.rounded_rectangle((x0 + 2, y0 + 1, x1 - 2, y1 - 3), radius=radius, fill="#9d7748")
    draw.ellipse((x1 - 2 * radius, y0, x1, y1), fill="#c39a5b")
    draw.ellipse((x1 - 2 * radius + 3, y0 + 3, x1 - 3, y1 - 3), fill="#8a6b41")


def draw_fallback_straw(
    draw: ImageDraw.ImageDraw, box: tuple[int, int, int, int], seed: int
) -> None:
    x0, y0, x1, y1 = box
    draw.ellipse(box, fill="#c39a5b")
    for step in range((x1 - x0) * (y1 - y0) // 8):
        x = x0 + 2 + (step * 37 + seed) % max(1, x1 - x0 - 4)
        y = y0 + 2 + (step * 53 + seed) % max(1, y1 - y0 - 4)
        px(draw, (x, y, x + 1, y), "#f1dfad")


def fallback_kindling_pile(name: str) -> Image.Image:
    """Draw a public-fallback stand-in for one gatherable kindling pile."""
    size, _ = KINDLING_PILES[name]
    image = Image.new("RGBA", size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    if name == "kindling_logs.png":
        draw_fallback_log(draw, (0, 14, 34, 30))
        draw_fallback_log(draw, (12, 2, 46, 18))
    elif name == "kindling_branches.png":
        draw_fallback_straw(draw, (0, 12, 26, 30), 5)
        draw_fallback_log(draw, (12, 6, 46, 22))
    else:
        draw_fallback_straw(draw, (0, 8, 32, 28), 11)
        for offset in range(3):
            top = 2 + offset * 5
            px(draw, (18 + offset * 2, top, 44 - offset * 4, top + 1), "#77543b")
    return image


def build_kindling_piles(props_path: Path) -> dict[str, Image.Image]:
    """Assemble the gatherable kindling piles from licensed props, or fall back."""
    if not props_path.is_file():
        return {name: fallback_kindling_pile(name) for name in KINDLING_PILES}
    sheet = Image.open(props_path).convert("RGBA")
    pieces = {name: sheet.crop(box) for name, box in KINDLING_PIECES.items()}
    piles = {}
    for name, (size, layout) in KINDLING_PILES.items():
        pile = Image.new("RGBA", size, (0, 0, 0, 0))
        for piece, offset in layout:
            pile.alpha_composite(pieces[piece], offset)
        piles[name] = pile
    return piles


def write_world_art(source: Path, output: Path) -> list[dict[str, object]]:
    world = output / "world"
    world.mkdir(parents=True, exist_ok=True)
    fallback = "project-authored procedural fallback"
    sources: dict[str, str] = {}
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
        tile_source = fallback
    tiles["stone.png"] = patterned_tile("#59594f", "#77766a", 13)
    tiles["floor.png"] = patterned_tile("#3f3428", "#594634", 17)
    for name, image in tiles.items():
        image.save(world / name, optimize=False)
        sources[name] = tile_source if name in {"grass.png", "road.png", "water.png"} else fallback

    props_path = source / PROPS_SHEET
    kindling_source = "licensed Modern Farm runtime extraction" if props_path.is_file() else fallback
    for name, image in build_kindling_piles(props_path).items():
        image.save(world / name, optimize=False)
        sources[name] = kindling_source

    private_tree = source / "THE NATURAL/Props/Tree 08.png"
    tree = Image.open(private_tree).convert("RGBA") if private_tree.is_file() else draw_tree()
    tree.save(world / "tree.png", optimize=False)
    sources["tree.png"] = "licensed runtime extraction" if private_tree.is_file() else fallback
    terrain_atlas, terrain_atlas_source = build_terrain_atlas(source)
    terrain_atlas.save(world / "terrain.png", optimize=False)
    sources["terrain.png"] = terrain_atlas_source
    draw_scribe().save(world / "scribe.png", optimize=False)
    sources["scribe.png"] = fallback
    records = []
    for path in sorted(world.glob("*.png")):
        records.append(
            {
                "path": str(path.relative_to(output)),
                "sha256": sha256(path),
                "size": list(Image.open(path).size),
                "source": sources[path.name],
            }
        )
    return records


def private_asset_path(source: Path, relative: str) -> Path:
    path = (source / relative).resolve()
    try:
        path.relative_to(source.resolve())
    except ValueError as error:
        raise SystemExit(f"interior source escapes private asset root: {relative}") from error
    return path


def fallback_interior_stamp(
    size: tuple[int, int], layer: str, seed: int
) -> Image.Image:
    """Make a readable open-build stand-in for one unavailable private stamp."""
    colors = {
        "floor": (91, 72, 52, 255),
        "wall": (70, 62, 54, 255),
        "object": (119, 84, 57, 255),
        "overlay": (146, 112, 70, 210),
    }
    image = Image.new("RGBA", size, colors[layer])
    draw = ImageDraw.Draw(image)
    detail = (42, 38, 33, 170)
    draw.rectangle((0, 0, size[0] - 1, size[1] - 1), outline=detail, width=2)
    for offset in range(seed % 7, max(size), 13):
        draw.line((offset, 0, 0, offset), fill=detail, width=1)
    return image


def load_interior_stamp(
    source_spec: dict[str, object], source: Path, layer: str
) -> Image.Image:
    """Crop one native-size private stamp or produce an equally sized fallback."""
    source_grid = int(source_spec["grid"])
    source_box = (
        int(source_spec["x"]) * source_grid,
        int(source_spec["y"]) * source_grid,
        (int(source_spec["x"]) + int(source_spec["width"])) * source_grid,
        (int(source_spec["y"]) + int(source_spec["height"])) * source_grid,
    )
    private_path = private_asset_path(source, str(source_spec["path"]))
    if private_path.is_file():
        with Image.open(private_path) as source_image:
            if source_box[2] > source_image.width or source_box[3] > source_image.height:
                raise SystemExit(f"interior crop exceeds {private_path}: {source_box}")
            stamp = source_image.convert("RGBA").crop(source_box)
        background_key = source_spec.get("background_key")
        if isinstance(background_key, dict):
            color = tuple(int(channel) for channel in background_key["color"])
            tolerance = int(background_key["tolerance"])
            softness = int(background_key["softness"])
            pixels = []
            for red, green, blue, alpha in stamp.get_flattened_data():
                distance = max(
                    abs(red - color[0]),
                    abs(green - color[1]),
                    abs(blue - color[2]),
                )
                coverage = max(0.0, min(1.0, (distance - tolerance) / softness))
                pixels.append((red, green, blue, round(alpha * coverage)))
            stamp.putdata(pixels)
        return stamp

    fallback_size = (
        int(source_spec["width"]) * source_grid,
        int(source_spec["height"]) * source_grid,
    )
    stamp_seed = int(hashlib.sha256(str(source_spec).encode()).hexdigest()[:8], 16)
    return fallback_interior_stamp(fallback_size, layer, stamp_seed)


def transform_interior_stamp(
    stamp: Image.Image, transform: dict[str, object] | None
) -> Image.Image:
    transformed = stamp
    if transform and transform.get("flip_x") is True:
        transformed = transformed.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    if transform and transform.get("flip_y") is True:
        transformed = transformed.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
    return transformed


def interior_pixel_position(item: dict[str, object], tile_size: int) -> tuple[int, int]:
    position = item.get("position")
    if isinstance(position, dict):
        grid = int(position["grid"])
        return (int(position["x"]) * grid, int(position["y"]) * grid)
    return (int(item["x"]) * tile_size, int(item["y"]) * tile_size)


def composite_scene_placements(
    image: Image.Image,
    level: dict[str, object],
    source: Path,
    tile_size: int,
) -> Image.Image:
    placements = sorted(
        level["placements"], key=lambda item: INTERIOR_LAYER_ORDER[item["layer"]]
    )
    for placement in placements:
        source_spec = placement["source"]
        stamp = load_interior_stamp(source_spec, source, placement["layer"])
        stamp = transform_interior_stamp(stamp, placement.get("transform"))

        placement_size = (
            int(placement["width"]) * tile_size,
            int(placement["height"]) * tile_size,
        )
        position = interior_pixel_position(placement, tile_size)
        if placement.get("repeat", False):
            repeated = Image.new("RGBA", placement_size, (0, 0, 0, 0))
            for y in range(0, placement_size[1], stamp.height):
                for x in range(0, placement_size[0], stamp.width):
                    repeated.alpha_composite(stamp, (x, y))
            stamp = repeated
        image.alpha_composite(stamp, position)
    return image


def render_interior(level: dict[str, object], source: Path) -> Image.Image:
    grid = level["grid"]
    tile_size = int(grid["tile_size"])
    room_size = (int(grid["width"]) * tile_size, int(grid["height"]) * tile_size)
    image = Image.new("RGBA", room_size, str(level.get("background", "#100c09")))
    floor_draw = ImageDraw.Draw(image)
    floor_line = str(level.get("floor_line", "#2d2119"))
    for y in range(tile_size, room_size[1], tile_size):
        floor_draw.line((0, y, room_size[0], y), fill=floor_line, width=1)
    for row, y in enumerate(range(0, room_size[1], tile_size)):
        offset = tile_size if row % 2 == 0 else tile_size * 2
        for x in range(offset, room_size[0], tile_size * 2):
            floor_draw.line(
                (x, y, x, min(y + tile_size, room_size[1])), fill=floor_line, width=1
            )
    return composite_scene_placements(image, level, source, tile_size)


def render_building(level: dict[str, object], source: Path) -> Image.Image:
    grid = level["grid"]
    tile_size = int(grid["tile_size"])
    size = (int(grid["width"]) * tile_size, int(grid["height"]) * tile_size)
    image = Image.new("RGBA", size, (0, 0, 0, 0))
    return composite_scene_placements(image, level, source, tile_size)


def write_mutable_interior_art(
    level: dict[str, object],
    source: Path,
    output: Path,
    interiors: Path,
    repair_pairs: dict[str, object] | None = None,
) -> list[dict[str, object]]:
    records = []
    room_id = str(level["id"])
    room_directory = interiors / room_id
    if room_directory.exists():
        shutil.rmtree(room_directory)
    room_directory.mkdir(parents=True)
    room_templates = level.get("templates", {})
    shared_templates = repair_pairs or {}
    instances = [*level.get("structures", []), *level.get("fixtures", [])]
    template_ids = sorted({instance["template"] for instance in instances})
    for template_id in template_ids:
        if INTERIOR_ID.fullmatch(template_id) is None:
            raise SystemExit(f"invalid mutable interior template: {template_id}")
        if int(level.get("schema_version", 2)) >= 3:
            template = shared_templates.get(template_id) or room_templates.get(template_id)
        else:
            template = room_templates.get(template_id) or shared_templates.get(template_id)
        if template is None:
            raise SystemExit(f"unknown mutable interior template: {template_id}")
        for state_name, visual in template["states"].items():
            if INTERIOR_ID.fullmatch(state_name) is None:
                raise SystemExit(f"invalid mutable interior state: {state_name}")
            if visual.get("visible", True) is False:
                continue
            stamp = load_interior_stamp(visual["source"], source, template["layer"])
            destination = room_directory / f"{template_id}--{state_name}.png"
            stamp.save(destination, optimize=False)
            records.append(
                {
                    "path": str(destination.relative_to(output)),
                    "sha256": sha256(destination),
                    "size": list(stamp.size),
                    "source": "authored repair-pair state flattened from a private crop or public fallback",
                }
            )
    return records


def write_interior_art(source: Path, output: Path) -> list[dict[str, object]]:
    interiors = output / "interiors"
    interiors.mkdir(parents=True, exist_ok=True)
    for stale in interiors.glob("*.png"):
        stale.unlink()
    repair_pair_document = json.loads(REPAIR_PAIR_PATH.read_text(encoding="utf-8"))
    if repair_pair_document.get("schema_version") != 1:
        raise SystemExit(f"unsupported repair-pair library in {REPAIR_PAIR_PATH}")
    repair_pairs = repair_pair_document.get("pairs", {})
    records = []
    for level_path in sorted(INTERIOR_ROOT.glob("*.json")):
        level = json.loads(level_path.read_text(encoding="utf-8"))
        if level.get("schema_version") not in {1, 2, 3, 4} or level.get("id") != level_path.stem:
            raise SystemExit(f"invalid interior identity in {level_path}")
        image = render_interior(level, source)
        destination = interiors / f"{level['id']}.png"
        image.save(destination, optimize=False)
        records.append(
            {
                "path": str(destination.relative_to(output)),
                "sha256": sha256(destination),
                "size": list(image.size),
                "source": "authored interior flattened from private stamps or public fallbacks",
            }
        )
        records.extend(
            write_mutable_interior_art(level, source, output, interiors, repair_pairs)
        )
    return records


def write_building_art(source: Path, output: Path) -> list[dict[str, object]]:
    buildings = output / "buildings"
    buildings.mkdir(parents=True, exist_ok=True)
    for stale in buildings.glob("*.png"):
        stale.unlink()
    repair_pair_document = json.loads(REPAIR_PAIR_PATH.read_text(encoding="utf-8"))
    if repair_pair_document.get("schema_version") != 1:
        raise SystemExit(f"unsupported repair-pair library in {REPAIR_PAIR_PATH}")
    repair_pairs = repair_pair_document.get("pairs", {})
    records = []
    for level_path in sorted(BUILDING_ROOT.glob("*.json")):
        level = json.loads(level_path.read_text(encoding="utf-8"))
        if (
            level.get("schema_version") != 4
            or level.get("scene_type") != "building"
            or level.get("id") != level_path.stem
        ):
            raise SystemExit(f"invalid building identity in {level_path}")
        image = render_building(level, source)
        destination = buildings / f"{level['id']}.png"
        image.save(destination, optimize=False)
        records.append(
            {
                "path": str(destination.relative_to(output)),
                "sha256": sha256(destination),
                "size": list(image.size),
                "source": "authored building cache flattened from private stamps or public fallbacks",
            }
        )
        records.extend(
            write_mutable_interior_art(level, source, output, buildings, repair_pairs)
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


def write_bundled_fonts(
    manifest: dict[str, object], output: Path
) -> list[dict[str, str]]:
    """Verify tracked open fonts and copy them into the generated runtime tree."""
    records = []
    for font in manifest["bundled_fonts"]:
        source = ROOT / font["source_file"]
        actual = sha256(source)
        if actual != font["sha256"]:
            raise SystemExit(f"hash mismatch for {source}: {actual}")

        destination = output / font["runtime_file"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

        license_source = ROOT / font["license_file"]
        license_actual = sha256(license_source)
        if license_actual != font["license_sha256"]:
            raise SystemExit(f"hash mismatch for {license_source}: {license_actual}")
        license_destination = output / font["runtime_license_file"]
        license_destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(license_source, license_destination)

        records.append(
            {
                "family": font["family"],
                "path": font["runtime_file"],
                "sha256": actual,
                "source": font["source"],
                "license": font["license"],
            }
        )
    return records


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
    generated.extend(write_interior_art(args.source, args.output))
    generated.extend(write_building_art(args.source, args.output))
    fonts = write_bundled_fonts(manifest, args.output)
    report = {
        "schema_version": 1,
        "private_checks": checks,
        "generated": generated,
        "bundled_fonts": fonts,
    }
    report_path = args.output / "provenance.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {len(generated)} generated assets, {len(fonts)} bundled fonts, "
        f"and {report_path}"
    )


if __name__ == "__main__":
    main()
