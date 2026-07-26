#!/usr/bin/env python3
"""Index private art for the local level editor without copying source pixels."""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from PIL import Image, UnidentifiedImageError

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ASSET_ROOT = ROOT / "assets"
TAG_RULES = ROOT / "meta/asset-tags.json"
IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".webp"}
WORD_BOUNDARY = re.compile(r"(?<=[a-z0-9])(?=[A-Z])|[^A-Za-z0-9]+")

SYNONYMS = {
    "bed": {"bedroom", "sleep", "furniture"},
    "book": {"books", "bookshelf", "library", "reading"},
    "cabinet": {"cupboard", "storage", "furniture"},
    "chair": {"seat", "furniture"},
    "desk": {"office", "writing", "table", "furniture"},
    "door": {"doorway", "entrance", "exit"},
    "floor": {"flooring", "ground", "tile"},
    "icon": {"inventory", "item", "ui"},
    "lamp": {"light", "lighting"},
    "plant": {"nature", "foliage"},
    "road": {"street", "path"},
    "roof": {"building", "structure"},
    "shelf": {"shelves", "storage", "furniture"},
    "table": {"desk", "furniture"},
    "wall": {"walls", "structure", "interior tile"},
    "window": {"building", "glass"},
}


def load_tag_rules(path: Path = TAG_RULES) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise ValueError(f"unsupported asset tag schema in {path}")
    return data["rules"]


def path_words(relative_path: str) -> set[str]:
    words = {
        word.lower()
        for word in WORD_BOUNDARY.split(Path(relative_path).with_suffix("").as_posix())
        if len(word) > 1
    }
    expanded = set(words)
    for word in words:
        expanded.update(SYNONYMS.get(word, ()))
    return expanded


def suggested_grid(relative_path: str, width: int, height: int) -> int:
    normalized = relative_path.lower()
    for size in (16, 32, 48):
        if f"/{size}x{size}/" in f"/{normalized}":
            return size
    if normalized.startswith(("motel/", "horror hotel/")):
        return 48
    if width % 48 == 0 and height % 48 == 0 and max(width, height) >= 384:
        return 48
    if width % 32 == 0 and height % 32 == 0:
        return 32
    if width % 16 == 0 and height % 16 == 0:
        return 16
    return 32


def rule_tags(relative_path: str, rules: list[dict[str, Any]]) -> set[str]:
    tags: set[str] = set()
    for rule in rules:
        if any(fnmatch.fnmatchcase(relative_path, pattern) for pattern in rule["patterns"]):
            tags.update(tag.lower() for tag in rule["tags"])
    return tags


def catalog_assets(
    asset_root: Path = DEFAULT_ASSET_ROOT,
    rules_path: Path = TAG_RULES,
) -> dict[str, Any]:
    rules = load_tag_rules(rules_path)
    records = []
    for path in sorted(asset_root.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in IMAGE_EXTENSIONS:
            continue
        try:
            with Image.open(path) as image:
                width, height = image.size
        except (OSError, UnidentifiedImageError):
            continue
        relative = path.relative_to(asset_root).as_posix()
        tags = path_words(relative) | rule_tags(relative, rules)
        pack = relative.split("/", maxsplit=1)[0]
        records.append(
            {
                "id": hashlib.sha1(relative.encode("utf-8"), usedforsecurity=False).hexdigest()[:12],
                "path": relative,
                "name": path.stem,
                "pack": pack,
                "width": width,
                "height": height,
                "grid": suggested_grid(relative, width, height),
                "tags": sorted(tags),
                "redistributable": pack == "custom",
            }
        )
    return {
        "schema_version": 1,
        "asset_root": asset_root.name,
        "count": len(records),
        "packs": sorted({record["pack"] for record in records}),
        "assets": records,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=Path, default=DEFAULT_ASSET_ROOT)
    parser.add_argument("--rules", type=Path, default=TAG_RULES)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    catalog = catalog_assets(args.assets, args.rules)
    serialized = json.dumps(catalog, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized, encoding="utf-8")
        print(f"indexed {catalog['count']} images in {args.output}")
    else:
        print(serialized, end="")


if __name__ == "__main__":
    main()
