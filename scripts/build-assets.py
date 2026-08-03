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
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "assets-manifest.json"
PALETTE = ["#f1dfad", "#c39a5b", "#77543b", "#34322a", "#78966b", "#9d5f55"]
CARD_SIZE = (96, 64)
TERRAIN_TILE_SIZE = 32
SCRIBE_FRAME_SIZE = 64
SCRIBE_COLUMNS = 13
SCRIBE_ROWS = 54
SCRIBE_TOOL_FRAME_SIZE = 128
SCRIBE_TOOL_COLUMNS = 6
SCRIBE_TOOL_ROWS = 4
SCRIBE_SLASH_FIRST_ROW = 12
# The LPC long-handled tools ride the thrust cycle instead of the slash cycle,
# and their overlay layers are drawn at the body's own frame size rather than
# the oversized swing frames the hammer and axe need.
SCRIBE_THRUST_FRAME_SIZE = 64
SCRIBE_THRUST_COLUMNS = 8
SCRIBE_THRUST_ROWS = 4
SCRIBE_THRUST_FIRST_ROW = 4
# Everyone else who walks into the valley. Each source is a complete LPC action
# sheet in the same geometry as the Scribe; only the walk rows are kept, because
# that is all a visitor ever does on screen and it is the shape generated
# travellers are composited at.
VISITOR_WALK_FIRST_ROW = 8
VISITOR_COLUMNS = 9
VISITOR_ROWS = 4
VISITOR_SHEETS = {
    "walker.png": "custom/redhead-lady.png",
    "elder-sibling.png": "custom/black-teen.png",
    "younger-sibling.png": "custom/little-sister.png",
    "old-hand.png": "custom/old-guy.png",
}
# Tints for the open fallback bodies, so a build without the private sheets still
# tells one visitor from another.
VISITOR_FALLBACK_TINTS = {
    "walker.png": "#a2564a",
    "elder-sibling.png": "#4a5c72",
    "younger-sibling.png": "#7c6a94",
    "old-hand.png": "#6b6455",
}
PRINT_CATALOG_PATH = ROOT / "content/prints.json"
PRINT_CARD_SIZE = (512, 768)
INTERIOR_ROOT = ROOT / "content/interiors"
BUILDING_ROOT = ROOT / "content/buildings"
REPAIR_PAIR_PATH = ROOT / "content/repair-pairs.json"
INTERIOR_LAYER_ORDER = {"floor": 0, "wall": 1, "object": 2, "overlay": 3}
SCENE_LAYERS = ("floor", "wall", "object", "overlay")
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
    (39, 1),  # W.inner_nw; upper corners run opposite the sheet's visual sequence
    (37, 1),  # W.inner_ne
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


def draw_bible_icon() -> Image.Image:
    """Open-build stand-in matching the authored icon's compact pixel canvas."""
    image = Image.new("RGBA", (34, 34), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    px(draw, (2, 5, 16, 30), "#6e3f2c")
    px(draw, (17, 5, 31, 30), "#80503a")
    px(draw, (4, 3, 16, 28), "#d1bd86")
    px(draw, (17, 3, 29, 28), "#dfcb92")
    px(draw, (15, 5, 18, 30), "#533326")
    px(draw, (9, 10, 11, 22), "#6e3f2c")
    px(draw, (6, 14, 14, 17), "#6e3f2c")
    return image


def write_ui_art(source: Path, output: Path) -> list[dict[str, object]]:
    """Copy redistributable custom UI art, with a complete public fallback."""
    ui = output / "ui"
    ui.mkdir(parents=True, exist_ok=True)
    for stale in ui.glob("*.png"):
        stale.unlink()

    custom_bible = source / "custom/bible-32.png"
    if custom_bible.is_file():
        bible = Image.open(custom_bible).convert("RGBA")
        bible_source = "project-authored custom item icon"
    else:
        bible = draw_bible_icon()
        bible_source = "project-authored procedural fallback"
    bible_path = ui / "bible-32.png"
    bible.save(bible_path, optimize=False)
    return [
        {
            "path": str(bible_path.relative_to(output)),
            "sha256": sha256(bible_path),
            "size": list(bible.size),
            "source": bible_source,
        }
    ]


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


def build_scribe_sheet(source: Path) -> tuple[Image.Image, str]:
    """Preserve the complete LPC action sheet, with a compatible open fallback."""
    private_scribe = source / "custom/scribe.png"
    expected_size = (
        SCRIBE_COLUMNS * SCRIBE_FRAME_SIZE,
        SCRIBE_ROWS * SCRIBE_FRAME_SIZE,
    )
    if private_scribe.is_file():
        sheet = Image.open(private_scribe).convert("RGBA")
        if sheet.size != expected_size:
            raise SystemExit(
                f"scribe sheet must be {expected_size[0]}x{expected_size[1]}: "
                f"{private_scribe} is {sheet.width}x{sheet.height}"
            )
        return sheet, "custom LPC-based character action sheet"

    frame = draw_scribe()
    sheet = Image.new("RGBA", expected_size, (0, 0, 0, 0))
    frame_offset = (
        (SCRIBE_FRAME_SIZE - frame.width) // 2,
        SCRIBE_FRAME_SIZE - frame.height,
    )
    for row in range(SCRIBE_ROWS):
        for column in range(SCRIBE_COLUMNS):
            sheet.alpha_composite(
                frame,
                (
                    column * SCRIBE_FRAME_SIZE + frame_offset[0],
                    row * SCRIBE_FRAME_SIZE + frame_offset[1],
                ),
            )
    return sheet, "project-authored procedural fallback"


def draw_fallback_hand_tool(kind: str, frame: int, direction: int) -> Image.Image:
    """Readable open-build stand-in for LPC foreground/background tool layers."""
    image = Image.new("RGBA", (SCRIBE_TOOL_FRAME_SIZE, SCRIBE_TOOL_FRAME_SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    phase = min(frame, SCRIBE_TOOL_COLUMNS - 1)
    x = 69 + (phase - 2) * (1 if direction in {0, 2} else 4)
    y = 48 + phase * 5
    handle = "#8f653c"
    metal = "#a9adb0"
    draw.line((x - 20, y + 34, x + 12, y - 12), fill=handle, width=4)
    if kind == "hammer":
        draw.rectangle((x + 6, y - 17, x + 22, y - 10), fill=metal)
    else:
        draw.polygon([(x + 4, y - 15), (x + 25, y - 24), (x + 18, y - 6)], fill=metal)
    return image


def build_scribe_tool_action(
    source: Path, scribe: Image.Image, kind: str
) -> tuple[Image.Image, str]:
    """Compose one LPC 128px work animation around the Scribe's slash cycle."""
    expected_overlay_size = (
        SCRIBE_TOOL_COLUMNS * SCRIBE_TOOL_FRAME_SIZE,
        SCRIBE_TOOL_ROWS * SCRIBE_TOOL_FRAME_SIZE,
    )
    overlay_root = source / "custom/lpc-tools"
    background_path = overlay_root / f"{kind}-bg.png"
    foreground_path = overlay_root / f"{kind}-fg.png"
    licensed_layers = background_path.is_file() and foreground_path.is_file()
    if licensed_layers:
        background = Image.open(background_path).convert("RGBA")
        foreground = Image.open(foreground_path).convert("RGBA")
        if background.size != expected_overlay_size or foreground.size != expected_overlay_size:
            raise SystemExit(
                f"LPC {kind} layers must be {expected_overlay_size[0]}x{expected_overlay_size[1]}"
            )
    action = Image.new("RGBA", expected_overlay_size, (0, 0, 0, 0))
    body_offset = (SCRIBE_TOOL_FRAME_SIZE - SCRIBE_FRAME_SIZE) // 2
    for direction in range(SCRIBE_TOOL_ROWS):
        for frame in range(SCRIBE_TOOL_COLUMNS):
            destination = (frame * SCRIBE_TOOL_FRAME_SIZE, direction * SCRIBE_TOOL_FRAME_SIZE)
            work_frame = Image.new(
                "RGBA", (SCRIBE_TOOL_FRAME_SIZE, SCRIBE_TOOL_FRAME_SIZE), (0, 0, 0, 0)
            )
            if licensed_layers:
                box = (
                    destination[0],
                    destination[1],
                    destination[0] + SCRIBE_TOOL_FRAME_SIZE,
                    destination[1] + SCRIBE_TOOL_FRAME_SIZE,
                )
                work_frame.alpha_composite(background.crop(box))
            body_box = (
                frame * SCRIBE_FRAME_SIZE,
                (SCRIBE_SLASH_FIRST_ROW + direction) * SCRIBE_FRAME_SIZE,
                (frame + 1) * SCRIBE_FRAME_SIZE,
                (SCRIBE_SLASH_FIRST_ROW + direction + 1) * SCRIBE_FRAME_SIZE,
            )
            work_frame.alpha_composite(scribe.crop(body_box), (body_offset, body_offset))
            if licensed_layers:
                work_frame.alpha_composite(foreground.crop(box))
            else:
                work_frame.alpha_composite(draw_fallback_hand_tool(kind, frame, direction))
            action.alpha_composite(work_frame, destination)
    provenance = (
        "LPC Scribe plus LPC hand-tool action layers"
        if licensed_layers
        else "LPC Scribe plus project-authored procedural tool fallback"
    )
    return action, provenance


def draw_fallback_long_tool(kind: str, frame: int, direction: int) -> Image.Image:
    """Open-build stand-in for one LPC long-handled tool overlay frame."""
    image = Image.new("RGBA", (SCRIBE_THRUST_FRAME_SIZE, SCRIBE_THRUST_FRAME_SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    # The thrust cycle winds up, drives forward, and recovers; the reach follows.
    reach = (0, 1, 3, 8, 11, 9, 5, 2)[min(frame, SCRIBE_THRUST_COLUMNS - 1)]
    facing = (0, -1, 1, 1)[direction]
    x = 32 + reach * facing
    y = 42 + (reach // 2 if direction == 0 else -reach // 3)
    handle = "#8f653c"
    metal = "#a9adb0"
    draw.line((x - 9 * facing, y + 16, x + 6 * facing, y - 8), fill=handle, width=3)
    head = (x + 6 * facing, y - 10, x + 6 * facing, y - 10)
    if kind == "hoe":
        draw.rectangle((head[0] - 6, head[1] - 1, head[0] + 6, head[1] + 3), fill=metal)
    elif kind == "shovel":
        draw.polygon(
            [(head[0] - 5, head[1]), (head[0] + 5, head[1]), (head[0], head[1] + 9)], fill=metal
        )
    else:
        draw.rectangle((head[0] - 5, head[1] - 4, head[0] + 4, head[1] + 4), fill=metal)
        draw.line((head[0] + 4, head[1] - 2, head[0] + 10, head[1] + 4), fill=metal, width=2)
    return image


def build_scribe_thrust_action(
    source: Path, scribe: Image.Image, kind: str
) -> tuple[Image.Image, str]:
    """Compose one LPC long-handled tool action around the Scribe's thrust cycle.

    Unlike the hammer and axe swings these overlays share the body's 64px frame,
    so the layers stack without an offset and the atlas keeps eight columns.
    """
    expected_overlay_size = (
        SCRIBE_THRUST_COLUMNS * SCRIBE_THRUST_FRAME_SIZE,
        SCRIBE_THRUST_ROWS * SCRIBE_THRUST_FRAME_SIZE,
    )
    overlay_root = source / "custom/lpc-tools"
    background_path = overlay_root / f"{kind}-bg.png"
    foreground_path = overlay_root / f"{kind}-fg.png"
    licensed_layers = background_path.is_file() and foreground_path.is_file()
    if licensed_layers:
        background = Image.open(background_path).convert("RGBA")
        foreground = Image.open(foreground_path).convert("RGBA")
        if background.size != expected_overlay_size or foreground.size != expected_overlay_size:
            raise SystemExit(
                f"LPC {kind} thrust layers must be "
                f"{expected_overlay_size[0]}x{expected_overlay_size[1]}"
            )
    action = Image.new("RGBA", expected_overlay_size, (0, 0, 0, 0))
    for direction in range(SCRIBE_THRUST_ROWS):
        for frame in range(SCRIBE_THRUST_COLUMNS):
            destination = (
                frame * SCRIBE_THRUST_FRAME_SIZE,
                direction * SCRIBE_THRUST_FRAME_SIZE,
            )
            box = (
                destination[0],
                destination[1],
                destination[0] + SCRIBE_THRUST_FRAME_SIZE,
                destination[1] + SCRIBE_THRUST_FRAME_SIZE,
            )
            work_frame = Image.new(
                "RGBA", (SCRIBE_THRUST_FRAME_SIZE, SCRIBE_THRUST_FRAME_SIZE), (0, 0, 0, 0)
            )
            if licensed_layers:
                work_frame.alpha_composite(background.crop(box))
            body_box = (
                frame * SCRIBE_FRAME_SIZE,
                (SCRIBE_THRUST_FIRST_ROW + direction) * SCRIBE_FRAME_SIZE,
                (frame + 1) * SCRIBE_FRAME_SIZE,
                (SCRIBE_THRUST_FIRST_ROW + direction + 1) * SCRIBE_FRAME_SIZE,
            )
            work_frame.alpha_composite(scribe.crop(body_box))
            if licensed_layers:
                work_frame.alpha_composite(foreground.crop(box))
            else:
                work_frame.alpha_composite(draw_fallback_long_tool(kind, frame, direction))
            action.alpha_composite(work_frame, destination)
    provenance = (
        "LPC Scribe plus LPC long-handled tool action layers"
        if licensed_layers
        else "LPC Scribe plus project-authored procedural tool fallback"
    )
    return action, provenance


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


def draw_forage_and_tool_props() -> dict[str, Image.Image]:
    """Small world props used by the first restoration-gathering loop."""
    fallen_log = Image.new("RGBA", (64, 32), (0, 0, 0, 0))
    log_draw = ImageDraw.Draw(fallen_log)
    draw_fallback_log(log_draw, (3, 9, 58, 27))
    px(log_draw, (14, 5, 18, 12), "#77543b")
    px(log_draw, (38, 3, 42, 11), "#77543b")

    plank = Image.new("RGBA", (80, 24), (0, 0, 0, 0))
    plank_draw = ImageDraw.Draw(plank)
    plank_draw.polygon([(2, 8), (74, 3), (78, 15), (5, 21)], fill="#9d7748")
    plank_draw.line([(6, 11), (72, 6)], fill="#c39a5b", width=2)
    plank_draw.line([(8, 18), (74, 12)], fill="#77543b", width=1)

    ladder = Image.new("RGBA", (44, 112), (0, 0, 0, 0))
    ladder_draw = ImageDraw.Draw(ladder)
    px(ladder_draw, (7, 3, 12, 108), "#77543b")
    px(ladder_draw, (31, 3, 36, 108), "#77543b")
    px(ladder_draw, (9, 3, 10, 108), "#c39a5b")
    px(ladder_draw, (33, 3, 34, 108), "#c39a5b")
    for y in range(13, 103, 15):
        px(ladder_draw, (9, y, 34, y + 4), "#9d7748")
        px(ladder_draw, (11, y, 32, y + 1), "#c39a5b")

    sawbuck = Image.new("RGBA", (120, 96), (0, 0, 0, 0))
    saw_draw = ImageDraw.Draw(sawbuck)
    px(saw_draw, (6, 40, 114, 56), "#77543b")
    px(saw_draw, (6, 40, 114, 44), "#9d7748")
    for leg in (16, 96):
        px(saw_draw, (leg, 56, leg + 8, 90), "#77543b")
        px(saw_draw, (leg + 2, 56, leg + 4, 90), "#c39a5b")
    saw_draw.line([(24, 40), (58, 22)], fill="#c39a5b", width=5)
    saw_draw.line([(62, 24), (100, 34)], fill="#8b8f96", width=3)
    px(saw_draw, (58, 20, 66, 30), "#34322a")

    outcrop = Image.new("RGBA", (96, 96), (0, 0, 0, 0))
    rock_draw = ImageDraw.Draw(outcrop)
    rock_draw.polygon([(16, 88), (28, 30), (50, 16), (70, 34), (80, 88)], fill="#6c6a63")
    rock_draw.polygon([(28, 32), (49, 19), (60, 42), (36, 50)], fill="#8b8880")
    rock_draw.polygon([(54, 50), (76, 44), (80, 86), (56, 86)], fill="#55534d")
    px(rock_draw, (10, 84, 88, 92), "#4a4842")

    return {
        "fallen_log.png": fallen_log,
        "plank.png": plank,
        "ladder.png": ladder,
        "sawbuck.png": sawbuck,
        "stone_outcrop.png": outcrop,
    }


FARM_TERRAIN_SHEET = "Modern_Farm_v1.2/48x48/1_Terrains_48x48.png"
# An interior cell of the ploughed field in the licensed terrain sheet: furrows
# that tile in both directions with no grass edge.
TILLED_FURROW_BOX = (48, 816, 96, 864)
FARM_SINGLES = "Modern_Farm_v1.2/48x48/Single_Files_48x48/0_Complete_Tileset_48x48"
VILLAGE_PROPS = "Post-Apocalyptic Village Tileset Pack/tile-B-02.png"
PARKING_SHEET = "parking/2.png"
# The torn-out bay in the parking sheet: bare ground with the kerb still framing
# it. `content/repair-pairs.json` uses this crop as the parking bays' repaired
# state, and the frame is lifted off it here so a bed keeps the outline of the
# space it used to be through tilling, sowing, and harvest.
TORN_BAY_BOX = (288, 288, 384, 384)
KERB_THICKNESS = 3
# A bed is one bay: the Scribe tears out the slab and works what is underneath,
# so every later state has to fill the same square footprint the asphalt did.
GARDEN_PLOT_SIZE = (96, 96)
GARDEN_SOIL_TILE = 48
# Only the worked states are generated. Paved and freshly-broken are the two
# faces of an authored repair pair, so the lot stays editable.
GARDEN_PLOT_STATES = {
    "garden_plot_tilled.png": ("tilled", None),
    "garden_plot_sown.png": ("tilled", "Seed_Grain_48x48.png"),
    "garden_plot_sprouting.png": ("tilled", "Crop_Grain_Sprout_48x48.png"),
    "garden_plot_growing.png": ("tilled", "Crop_Grain_Stage_2_48x48.png"),
    "garden_plot_ripe.png": ("tilled", "Crop_Grain_Ripe_48x48.png"),
}


def tile_surface(texture: Image.Image) -> Image.Image:
    """Repeat one soil texture across a whole bed."""
    bed = Image.new("RGBA", GARDEN_PLOT_SIZE, (0, 0, 0, 0))
    for row in range(GARDEN_PLOT_SIZE[1] // GARDEN_SOIL_TILE):
        for column in range(GARDEN_PLOT_SIZE[0] // GARDEN_SOIL_TILE):
            bed.alpha_composite(texture, (column * GARDEN_SOIL_TILE, row * GARDEN_SOIL_TILE))
    return bed


def draw_fallback_kerb() -> Image.Image:
    """Open-build stand-in for the concrete edge around a torn-out bay."""
    frame = Image.new("RGBA", GARDEN_PLOT_SIZE, (0, 0, 0, 0))
    draw = ImageDraw.Draw(frame)
    draw.rectangle((0, 0, GARDEN_PLOT_SIZE[0] - 1, GARDEN_PLOT_SIZE[1] - 1), outline="#8f8d88",
                   width=KERB_THICKNESS)
    return frame


def draw_fallback_plot_surface(kind: str) -> Image.Image:
    """A readable open-build stand-in for one licensed bed surface."""
    texture = Image.new("RGBA", (GARDEN_SOIL_TILE, GARDEN_SOIL_TILE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(texture)
    draw.rectangle((0, 0, GARDEN_SOIL_TILE - 1, GARDEN_SOIL_TILE - 1), fill="#5d452f")
    if kind == "tilled":
        for row in range(4, GARDEN_SOIL_TILE, 7):
            draw.line((0, row, GARDEN_SOIL_TILE - 1, row), fill="#77543b", width=2)
    else:
        for step in range(26):
            x = (step * 29) % GARDEN_SOIL_TILE
            y = (step * 17) % GARDEN_SOIL_TILE
            px(draw, (x, y, x + 1, y + 1), "#77543b")
    return tile_surface(texture)


def draw_fallback_crop(stage: str) -> Image.Image:
    """Open-build grain: scattered seed, a shoot, standing green, ripe heads."""
    image = Image.new("RGBA", (GARDEN_SOIL_TILE, GARDEN_SOIL_TILE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    if "Seed" in stage:
        for step in range(9):
            x = 12 + (step * 13) % 26
            y = 16 + (step * 7) % 18
            px(draw, (x, y, x + 1, y + 1), "#c39a5b")
        return image
    height, colour = {
        "Crop_Grain_Sprout_48x48.png": (10, "#78966b"),
        "Crop_Grain_Stage_2_48x48.png": (24, "#526e42"),
        "Crop_Grain_Ripe_48x48.png": (26, "#c39a5b"),
    }[stage]
    for stalk in range(5):
        x = 12 + stalk * 6
        draw.line((x, 40, x - 2, 40 - height), fill=colour, width=2)
        if colour == "#c39a5b":
            px(draw, (x - 4, 40 - height - 4, x, 40 - height + 2), "#f1dfad")
    return image


def lift_kerb(torn_bay: Image.Image) -> Image.Image:
    """Keep only the concrete edge of the torn-out bay, as an overlay."""
    frame = torn_bay.copy()
    inner = Image.new("RGBA", GARDEN_PLOT_SIZE, (0, 0, 0, 0))
    frame.paste(
        inner.crop(
            (
                KERB_THICKNESS,
                KERB_THICKNESS,
                GARDEN_PLOT_SIZE[0] - KERB_THICKNESS,
                GARDEN_PLOT_SIZE[1] - KERB_THICKNESS,
            )
        ),
        (KERB_THICKNESS, KERB_THICKNESS),
    )
    return frame


def build_garden_plots(source: Path) -> list[tuple[str, Image.Image, str]]:
    """Build the worked bed states, from tilled rows through standing grain."""
    terrain_path = source / FARM_TERRAIN_SHEET
    singles = source / FARM_SINGLES
    parking_path = source / PARKING_SHEET
    licensed = terrain_path.is_file() and singles.is_dir() and parking_path.is_file()
    if licensed:
        with Image.open(terrain_path) as sheet:
            furrows = sheet.convert("RGBA").crop(TILLED_FURROW_BOX)
        with Image.open(singles / "Topsoil_48x48.png") as topsoil:
            soil = topsoil.convert("RGBA")
        with Image.open(parking_path) as sheet:
            torn = sheet.convert("RGBA").crop(TORN_BAY_BOX)
        kerb = lift_kerb(torn)
        surfaces = {"soil": tile_surface(soil), "tilled": tile_surface(furrows)}
    else:
        kerb = draw_fallback_kerb()
        surfaces = {kind: draw_fallback_plot_surface(kind) for kind in ("soil", "tilled")}

    plots = []
    for name, (surface, crop_file) in GARDEN_PLOT_STATES.items():
        plot = surfaces[surface].copy()
        if crop_file is not None:
            if licensed:
                with Image.open(singles / crop_file) as crop_image:
                    crop = crop_image.convert("RGBA")
            else:
                crop = draw_fallback_crop(crop_file)
            # Four clumps, one per quarter of the bed, so a whole bay reads as
            # planted rather than as one tuft sitting in a square of dirt.
            for row in range(2):
                for column in range(2):
                    plot.alpha_composite(
                        crop,
                        (
                            column * GARDEN_SOIL_TILE + (GARDEN_SOIL_TILE - crop.width) // 2,
                            (row + 1) * GARDEN_SOIL_TILE - crop.height,
                        ),
                    )
        # The bed never stops being a parking space.
        plot.alpha_composite(kerb)
        plots.append(
            (
                name,
                plot,
                "licensed Modern Farm and parking runtime extraction"
                if licensed
                else "project-authored procedural fallback",
            )
        )
    return plots


# Props the garden and forage loops add. The valley hands over nothing
# manufactured: the barrel is the motel's own staved-in rain butt in two states,
# and everything else the Scribe eats before the first harvest is growing wild.
VILLAGE_GROUND = "Post-Apocalyptic Village Tileset Pack/tile-B-01.png"
GARDEN_PROP_ART = {
    "rain_cistern_damaged.png": (VILLAGE_PROPS, (10, 3), (48, 48)),
    "rain_cistern.png": (VILLAGE_PROPS, (11, 3), (48, 48)),
    "seed_sack.png": (VILLAGE_PROPS, (11, 4), (48, 48)),
    "forage_fungus.png": (VILLAGE_GROUND, (6, 9), (48, 48)),
    "forage_greens.png": (VILLAGE_GROUND, (2, 11), (48, 48)),
    "forage_agave.png": (VILLAGE_GROUND, (2, 10), (48, 48)),
}


def draw_fallback_garden_prop(name: str) -> Image.Image:
    image = Image.new("RGBA", GARDEN_PROP_ART[name][2], (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    if name == "seed_sack.png":
        draw.polygon([(12, 44), (10, 20), (18, 12), (30, 12), (38, 20), (36, 44)], fill="#c39a5b")
        px(draw, (18, 10, 30, 15), "#9d7748")
        for step in range(6):
            px(draw, (16 + step * 3, 44 - step % 3, 18 + step * 3, 46 - step % 3), "#f1dfad")
    elif name.startswith("rain_cistern"):
        draw.rounded_rectangle((9, 10, 39, 45), radius=6, fill="#77543b")
        draw.ellipse((9, 6, 39, 18), fill="#9d7748")
        if name == "rain_cistern.png":
            draw.ellipse((12, 8, 36, 16), fill="#2f4a50")
        else:
            draw.ellipse((12, 8, 36, 16), fill="#4f3a28")
            # A staved-in side is the whole reason it holds nothing.
            draw.polygon([(14, 26), (26, 22), (30, 38), (16, 40)], fill="#34322a")
        for band in (20, 34):
            px(draw, (9, band, 39, band + 2), "#8b8f96")
    elif name == "forage_fungus.png":
        for cap, (x, y, r) in enumerate([(16, 30, 7), (28, 24, 6), (23, 38, 5)]):
            px(draw, (x - 1, y, x + 1, y + r), "#c39a5b")
            draw.ellipse((x - r, y - r + 2, x + r, y + 3), fill="#9d5f55" if cap % 2 else "#77543b")
    elif name == "forage_greens.png":
        for leaf in range(5):
            x = 12 + leaf * 6
            draw.line((24, 42, x, 42 - 14 - (leaf % 3) * 4), fill="#526e42", width=3)
    else:
        for blade in range(7):
            x = 10 + blade * 5
            draw.line((24, 43, x, 43 - 20 - (blade % 4) * 3), fill="#78966b", width=2)
    return image


def build_garden_props(source: Path) -> list[tuple[str, Image.Image, str]]:
    props = []
    sheets: dict[str, Image.Image] = {}
    for name, (relative, (cell_x, cell_y), size) in GARDEN_PROP_ART.items():
        path = source / relative
        if not path.is_file():
            props.append(
                (name, draw_fallback_garden_prop(name), "project-authored procedural fallback")
            )
            continue
        if relative not in sheets:
            with Image.open(path) as sheet:
                sheets[relative] = sheet.convert("RGBA")
        crop = sheets[relative].crop(
            (cell_x * 48, cell_y * 48, (cell_x + 1) * 48, (cell_y + 1) * 48)
        )
        if crop.size != size:
            crop = crop.resize(size, Image.Resampling.NEAREST)
        props.append((name, crop, "licensed runtime extraction"))
    return props


WORKED_STATION_ART = {
    # The Scribe returns to these, so they use pack art at the runtime size when
    # the licensed source is present and fall back to the drawn props otherwise.
    "sawbuck.png": (
        "Modern_Farm_v1.2/48x48/Single_Files_48x48/"
        "0_Complete_Tileset_48x48/Woodwork_Crafting_Table_Full_48x48.png",
        (120, 96),
    ),
    "stone_outcrop.png": (
        "Modern_Farm_v1.2/48x48/Single_Files_48x48/"
        "0_Complete_Tileset_48x48/Rock_Big_48x48.png",
        (96, 96),
    ),
}


WAY_STATION_SIGN_ART = "components/way-station-sign.png"
WAY_STATION_SIGN_SIZE = (101, 100)


def draw_way_station_sign() -> Image.Image:
    """A blank board on two posts, for builds without the painted sign."""
    image = Image.new("RGBA", WAY_STATION_SIGN_SIZE, (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    for post in (30, 64):
        px(draw, (post, 56, post + 6, 98), "#55534d")
        px(draw, (post + 1, 56, post + 2, 98), "#8b8880")
    px(draw, (6, 4, 94, 54), "#34322a")
    px(draw, (9, 7, 91, 51), "#4f6a86")
    px(draw, (14, 14, 86, 32), "#8b8880")
    px(draw, (18, 62, 82, 82), "#5d4632")
    px(draw, (21, 65, 79, 79), "#77543b")
    return image


def build_way_station_sign(source: Path) -> tuple[Image.Image, str]:
    """The painted motel sign, with a plain board on the same posts otherwise."""
    painted = source / WAY_STATION_SIGN_ART
    if painted.is_file():
        with Image.open(painted) as art:
            return art.convert("RGBA"), "project-authored custom sign art"
    return draw_way_station_sign(), "project-authored procedural fallback"


def build_worked_stations(source: Path) -> list[tuple[str, Image.Image, str]]:
    """Prefer licensed station art, keeping the drawn prop as the open build."""
    stations = []
    drawn = draw_forage_and_tool_props()
    for name, (relative, size) in WORKED_STATION_ART.items():
        licensed = source / relative
        if licensed.is_file():
            with Image.open(licensed) as sheet:
                image = sheet.convert("RGBA")
            if image.size != size:
                image = image.resize(size, Image.Resampling.NEAREST)
            stations.append((name, image, "licensed Modern Farm runtime extraction"))
        else:
            stations.append((name, drawn[name], "project-authored procedural fallback"))
    return stations


def write_world_art(source: Path, output: Path) -> list[dict[str, object]]:
    world = output / "world"
    world.mkdir(parents=True, exist_ok=True)
    for stale in world.glob("*.png"):
        stale.unlink()
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
    for name, image in tiles.items():
        image.save(world / name, optimize=False)
        sources[name] = tile_source

    props_path = source / PROPS_SHEET
    kindling_source = "licensed Modern Farm runtime extraction" if props_path.is_file() else fallback
    for name, image in build_kindling_piles(props_path).items():
        image.save(world / name, optimize=False)
        sources[name] = kindling_source
    for name, image in draw_forage_and_tool_props().items():
        image.save(world / name, optimize=False)
        sources[name] = fallback
    for name, image, station_source in build_worked_stations(source):
        image.save(world / name, optimize=False)
        sources[name] = station_source
    for name, image, garden_source in build_garden_plots(source) + build_garden_props(source):
        image.save(world / name, optimize=False)
        sources[name] = garden_source

    sign, sign_source = build_way_station_sign(source)
    sign.save(world / "way_station_sign.png", optimize=False)
    sources["way_station_sign.png"] = sign_source

    private_tree = source / "THE NATURAL/Props/Tree 08.png"
    tree = Image.open(private_tree).convert("RGBA") if private_tree.is_file() else draw_tree()
    tree.save(world / "tree.png", optimize=False)
    sources["tree.png"] = "licensed runtime extraction" if private_tree.is_file() else fallback
    terrain_atlas, terrain_atlas_source = build_terrain_atlas(source)
    terrain_atlas.save(world / "terrain.png", optimize=False)
    sources["terrain.png"] = terrain_atlas_source
    scribe, scribe_source = build_scribe_sheet(source)
    scribe.save(world / "scribe.png", optimize=False)
    sources["scribe.png"] = scribe_source
    for tool_kind in ("hammer", "axe"):
        action, action_source = build_scribe_tool_action(source, scribe, tool_kind)
        action_name = f"scribe-{tool_kind}.png"
        action.save(world / action_name, optimize=False)
        sources[action_name] = action_source
    for tool_kind in ("hoe", "watering-can", "shovel"):
        action, action_source = build_scribe_thrust_action(source, scribe, tool_kind)
        action_name = f"scribe-{tool_kind}.png"
        action.save(world / action_name, optimize=False)
        sources[action_name] = action_source
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


def walk_rows(sheet: Image.Image) -> Image.Image:
    """The four walk rows of a full action sheet, and nothing else.

    Visitors are drawn from the walk cycle alone — they approach, they stand on
    frame 0, they leave — and generated travellers are composited at that size
    because compositing fifty unused rows would cost fifty times as much. Both
    kinds of visitor therefore share one atlas shape, which is the only reason
    the fallback and the generated art are interchangeable at all.
    """
    top = VISITOR_WALK_FIRST_ROW * SCRIBE_FRAME_SIZE
    return sheet.crop(
        (
            0,
            top,
            VISITOR_COLUMNS * SCRIBE_FRAME_SIZE,
            top + VISITOR_ROWS * SCRIBE_FRAME_SIZE,
        )
    )


def write_people_art(source: Path, output: Path) -> list[dict[str, object]]:
    """Visitor walk sheets, with a recognisable open fallback for each."""
    people = output / "people"
    people.mkdir(parents=True, exist_ok=True)
    expected = (SCRIBE_COLUMNS * SCRIBE_FRAME_SIZE, SCRIBE_ROWS * SCRIBE_FRAME_SIZE)
    records = []
    for name, relative in VISITOR_SHEETS.items():
        private = source / relative
        if private.is_file():
            sheet = Image.open(private).convert("RGBA")
            if sheet.size != expected:
                raise SystemExit(
                    f"visitor sheet must be {expected[0]}x{expected[1]}: "
                    f"{private} is {sheet.width}x{sheet.height}"
                )
            origin = "custom LPC-based character action sheet"
        else:
            sheet = draw_fallback_person(VISITOR_FALLBACK_TINTS[name], expected)
            origin = "generated fallback"
        sheet = walk_rows(sheet)
        destination = people / name
        sheet.save(destination, optimize=False)
        records.append(
            {
                "path": str(destination.relative_to(output)),
                "sha256": sha256(destination),
                "size": list(sheet.size),
                "source": origin,
            }
        )
    return records


def draw_fallback_person(tint: str, size: tuple[int, int]) -> Image.Image:
    """A body the same shape as the Scribe's, in a different coat."""
    frame = draw_scribe()
    coat = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    ImageDraw.Draw(coat).rectangle((0, 0, frame.width, frame.height), fill=tint)
    frame = Image.composite(Image.blend(frame, coat, 0.45), frame, frame.split()[3])
    sheet = Image.new("RGBA", size, (0, 0, 0, 0))
    offset = (
        (SCRIBE_FRAME_SIZE - frame.width) // 2,
        SCRIBE_FRAME_SIZE - frame.height,
    )
    for row in range(SCRIBE_ROWS):
        for column in range(SCRIBE_COLUMNS):
            sheet.alpha_composite(
                frame,
                (
                    column * SCRIBE_FRAME_SIZE + offset[0],
                    row * SCRIBE_FRAME_SIZE + offset[1],
                ),
            )
    return sheet


def write_print_cards(source: Path, output: Path) -> list[dict[str, object]]:
    """Composed block-print cards, one per catalog entry.

    The catalog is the authority on which cards exist. An entry whose card has
    not been composed yet still gets a readable placeholder, so authoring a verse
    is never blocked on generating its illustration.
    """
    catalog = json.loads(PRINT_CATALOG_PATH.read_text(encoding="utf-8"))
    prints = output / "prints"
    prints.mkdir(parents=True, exist_ok=True)
    records = []
    for entry in catalog["prints"]:
        composed = ROOT / entry["card"]
        destination = prints / f"{entry['id']}-card.png"
        if composed.is_file():
            card = Image.open(composed).convert("RGBA")
            origin = "composed print card"
        else:
            card = draw_placeholder_card(entry)
            origin = "generated placeholder pending illustration"
        card.save(destination, optimize=False)
        records.append(
            {
                "path": str(destination.relative_to(output)),
                "sha256": sha256(destination),
                "size": list(card.size),
                "source": origin,
            }
        )
        # The unlettered composite goes with it. A card handed to somebody who
        # does not read English has to be lettered at runtime, and there is no
        # talking a PNG out of the words already in it.
        blank_source = ROOT / entry["card"].replace("-card.png", "-blank.png")
        if not blank_source.is_file():
            continue
        blank_destination = prints / f"{entry['id']}-blank.png"
        blank = Image.open(blank_source).convert("RGBA")
        blank.save(blank_destination, optimize=False)
        records.append(
            {
                "path": str(blank_destination.relative_to(output)),
                "sha256": sha256(blank_destination),
                "size": list(blank.size),
                "source": "print card with its panel left for runtime lettering",
            }
        )
    return records


def draw_placeholder_card(entry: dict[str, object]) -> Image.Image:
    """Paper, a border, and the reference. Enough to know which card this is."""
    card = Image.new("RGBA", PRINT_CARD_SIZE, "#d8c8a4")
    draw = ImageDraw.Draw(card)
    margin = 18
    draw.rectangle(
        (margin, margin, PRINT_CARD_SIZE[0] - margin, PRINT_CARD_SIZE[1] - margin),
        outline="#221a12",
        width=6,
    )
    # A blank plate where the illustration will go, in the same proportion the
    # generated art uses, so a placeholder card reads at the same size.
    plate = (margin + 26, margin + 26, PRINT_CARD_SIZE[0] - margin - 26, int(PRINT_CARD_SIZE[1] * 0.67))
    draw.rectangle(plate, outline="#221a12", width=3)
    draw.line((plate[0], plate[1], plate[2], plate[3]), fill="#221a12", width=2)
    draw.line((plate[0], plate[3], plate[2], plate[1]), fill="#221a12", width=2)
    draw.text((margin + 34, plate[3] + 40), str(entry["title"]), fill="#221a12")
    draw.text((margin + 34, plate[3] + 60), str(entry["reference"]), fill="#3b2c1c")
    draw.text((margin + 34, plate[3] + 96), "block not yet cut", fill="#5a4630")
    return card


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


def placement_occludes_player(placement: dict[str, object]) -> bool:
    return placement.get("occludes_player") is True


def scene_placement_stamps(
    level: dict[str, object],
    source: Path,
    tile_size: int,
    layer: str | None = None,
) -> list[tuple[dict[str, object], Image.Image, tuple[int, int]]]:
    """Resolve authored placements to their final stamps, in authored order."""
    stamps = []
    for placement in level["placements"]:
        if layer is not None and placement["layer"] != layer:
            continue
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
        stamps.append((placement, stamp, position))
    return stamps


def composite_scene_placements(
    image: Image.Image,
    level: dict[str, object],
    source: Path,
    tile_size: int,
    layer: str | None = None,
    skip_occluders: bool = False,
) -> Image.Image:
    stamps = sorted(
        scene_placement_stamps(level, source, tile_size, layer),
        key=lambda entry: INTERIOR_LAYER_ORDER[entry[0]["layer"]],
    )
    for placement, stamp, position in stamps:
        if skip_occluders and placement_occludes_player(placement):
            continue
        image.alpha_composite(stamp, position)
    return image


def render_interior(level: dict[str, object], source: Path) -> Image.Image:
    layers = render_scene_layers(level, source, interior=True)
    image = Image.new("RGBA", layers["floor"].size, (0, 0, 0, 0))
    for layer in SCENE_LAYERS:
        image.alpha_composite(layers[layer])
    return image


def render_building(level: dict[str, object], source: Path) -> Image.Image:
    layers = render_scene_layers(level, source, interior=False)
    image = Image.new("RGBA", layers["floor"].size, (0, 0, 0, 0))
    for layer in SCENE_LAYERS:
        image.alpha_composite(layers[layer])
    return image


def render_scene_layers(
    level: dict[str, object],
    source: Path,
    *,
    interior: bool,
    extract_occluders: bool = False,
) -> dict[str, Image.Image]:
    """Render independent caches so baked and mutable art can share layer order."""
    grid = level["grid"]
    tile_size = int(grid["tile_size"])
    size = (int(grid["width"]) * tile_size, int(grid["height"]) * tile_size)
    layers = {
        layer: Image.new("RGBA", size, (0, 0, 0, 0)) for layer in SCENE_LAYERS
    }
    if interior:
        floor = layers["floor"]
        floor.alpha_composite(
            Image.new("RGBA", size, str(level.get("background", "#100c09")))
        )
        floor_draw = ImageDraw.Draw(floor)
        floor_line = str(level.get("floor_line", "#2d2119"))
        for y in range(tile_size, size[1], tile_size):
            floor_draw.line((0, y, size[0], y), fill=floor_line, width=1)
        for row, y in enumerate(range(0, size[1], tile_size)):
            offset = tile_size if row % 2 == 0 else tile_size * 2
            for x in range(offset, size[0], tile_size * 2):
                floor_draw.line(
                    (x, y, x, min(y + tile_size, size[1])),
                    fill=floor_line,
                    width=1,
                )
    for layer, image in layers.items():
        composite_scene_placements(
            image, level, source, tile_size, layer, skip_occluders=extract_occluders
        )
    return layers


def write_interior_occluder_art(
    level: dict[str, object], source: Path, output: Path, interiors: Path
) -> list[dict[str, object]]:
    """Extract walk-behind scenery so the runtime can re-sort it against the player.

    Authored order is the shared index between this writer and the engine, so
    the crop never has to be recovered from the flattened layer at runtime.
    """
    records = []
    tile_size = int(level["grid"]["tile_size"])
    room_directory = interiors / str(level["id"])
    room_directory.mkdir(parents=True, exist_ok=True)
    occluders = [
        (placement, stamp)
        for placement, stamp, _ in scene_placement_stamps(level, source, tile_size)
        if placement_occludes_player(placement)
    ]
    for index, (_, stamp) in enumerate(occluders):
        destination = room_directory / f"occluder--{index:02d}.png"
        stamp.save(destination, optimize=False)
        records.append(
            {
                "path": str(destination.relative_to(output)),
                "sha256": sha256(destination),
                "size": list(stamp.size),
                "source": "authored walk-behind placement extracted from its scene layer",
            }
        )
    return records


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


def write_portable_item_art(source: Path, output: Path) -> list[dict[str, object]]:
    """Extract portable scene items separately so pickup never erases baked art."""
    item_root = output / "items"
    if item_root.exists():
        shutil.rmtree(item_root)
    records = []
    for scene_root in (INTERIOR_ROOT, BUILDING_ROOT):
        for level_path in sorted(scene_root.glob("*.json")):
            level = json.loads(level_path.read_text(encoding="utf-8"))
            scene_id = str(level["id"])
            for item in level.get("items", []):
                item_id = str(item["id"])
                if INTERIOR_ID.fullmatch(item_id) is None:
                    raise SystemExit(f"invalid portable item id: {item_id}")
                stamp = load_interior_stamp(item["source"], source, item["layer"])
                stamp = transform_interior_stamp(stamp, item.get("transform"))
                destination = item_root / scene_id / f"{item_id}.png"
                destination.parent.mkdir(parents=True, exist_ok=True)
                stamp.save(destination, optimize=False)
                records.append(
                    {
                        "path": str(destination.relative_to(output)),
                        "sha256": sha256(destination),
                        "size": list(stamp.size),
                        "source": "authored portable item flattened from a private crop or public fallback",
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
        if level.get("schema_version") not in {1, 2, 3, 4, 5} or level.get("id") != level_path.stem:
            raise SystemExit(f"invalid interior identity in {level_path}")
        layers = render_scene_layers(
            level, source, interior=True, extract_occluders=True
        )
        for layer, image in layers.items():
            destination = interiors / f"{level['id']}--{layer}.png"
            image.save(destination, optimize=False)
            records.append(
                {
                    "path": str(destination.relative_to(output)),
                    "sha256": sha256(destination),
                    "size": list(image.size),
                    "source": "authored interior layer flattened from private stamps or public fallbacks",
                }
            )
        records.extend(
            write_mutable_interior_art(level, source, output, interiors, repair_pairs)
        )
        records.extend(write_interior_occluder_art(level, source, output, interiors))
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
            level.get("schema_version") not in {4, 5}
            or level.get("scene_type") != "building"
            or level.get("id") != level_path.stem
        ):
            raise SystemExit(f"invalid building identity in {level_path}")
        for layer, image in render_scene_layers(level, source, interior=False).items():
            destination = buildings / f"{level['id']}--{layer}.png"
            image.save(destination, optimize=False)
            records.append(
                {
                    "path": str(destination.relative_to(output)),
                    "sha256": sha256(destination),
                    "size": list(image.size),
                    "source": "authored building layer flattened from private stamps or public fallbacks",
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


def write_licensed_audio(
    manifest: dict[str, object], output: Path, source_root: Path, strict: bool
) -> list[dict[str, str]]:
    """Copy only selected private audio into the runtime-only asset boundary."""
    audio_output = output / "audio"
    if audio_output.exists():
        shutil.rmtree(audio_output)

    records = []
    copied_licenses: set[str] = set()
    for audio in manifest.get("licensed_audio", []):
        source = source_root / audio["source_file"]
        if not source.is_file():
            if strict:
                raise SystemExit(f"required licensed audio is missing: {source}")
            continue

        destination = output / audio["runtime_file"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        transcode = audio.get("transcode")
        if transcode:
            ffmpeg = shutil.which("ffmpeg")
            if ffmpeg is None:
                raise SystemExit(
                    "ffmpeg is required to prepare selected licensed runtime audio"
                )
            command = [
                ffmpeg,
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                str(source),
                "-t",
                str(transcode["duration_seconds"]),
            ]
            filters = []
            if "lowpass_hz" in transcode:
                filters.append(f"lowpass=f={transcode['lowpass_hz']}")
            if "normalize_lufs" in transcode:
                filters.append(
                    f"loudnorm=I={transcode['normalize_lufs']}:TP=-2:LRA=7"
                )
            if filters:
                command.extend(["-af", ",".join(filters)])
            command.extend(
                [
                    "-codec:a",
                    "libmp3lame",
                    "-b:a",
                    transcode["bitrate"],
                    str(destination),
                ]
            )
            subprocess.run(command, check=True)
        else:
            shutil.copyfile(source, destination)

        license_file = audio["license_file"]
        license_source = source_root / license_file
        if strict and not license_source.is_file():
            raise SystemExit(f"licensed audio attribution is missing: {license_source}")
        if license_source.is_file() and license_file not in copied_licenses:
            license_destination = output / "audio/licenses" / Path(license_file).name
            license_destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(license_source, license_destination)
            copied_licenses.add(license_file)

        records.append(
            {
                "role": audio["role"],
                "creator": audio["creator"],
                "path": audio["runtime_file"],
                "source_sha256": sha256(source),
                "sha256": sha256(destination),
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
    generated.extend(write_ui_art(args.source, args.output))
    generated.extend(write_world_art(args.source, args.output))
    generated.extend(write_interior_art(args.source, args.output))
    generated.extend(write_building_art(args.source, args.output))
    generated.extend(write_portable_item_art(args.source, args.output))
    generated.extend(write_people_art(args.source, args.output))
    generated.extend(write_print_cards(args.source, args.output))
    fonts = write_bundled_fonts(manifest, args.output)
    audio = write_licensed_audio(manifest, args.output, ROOT, args.strict_private)
    report = {
        "schema_version": 1,
        "private_checks": checks,
        "generated": generated,
        "bundled_fonts": fonts,
        "licensed_audio": audio,
    }
    report_path = args.output / "provenance.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {len(generated)} generated assets, {len(fonts)} bundled fonts, "
        f"{len(audio)} licensed audio files, "
        f"and {report_path}"
    )


if __name__ == "__main__":
    main()
