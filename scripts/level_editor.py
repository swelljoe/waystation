#!/usr/bin/env python3
"""Serve the private, local-only Waystation scene editor."""

from __future__ import annotations

import argparse
import json
import mimetypes
import re
import tempfile
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from threading import Lock
from typing import Any
from urllib.parse import unquote, urlparse

from asset_catalog import DEFAULT_ASSET_ROOT, catalog_assets

ROOT = Path(__file__).resolve().parent.parent
EDITOR_ROOT = ROOT / "tools/level-editor"
LEVEL_ROOT = ROOT / "content/interiors"
BUILDING_ROOT = ROOT / "content/buildings"
REPAIR_PAIR_PATH = ROOT / "content/repair-pairs.json"
LEVEL_ID = re.compile(r"^[a-z0-9][a-z0-9_-]{0,63}$")
MAX_REQUEST_BYTES = 5 * 1024 * 1024


def safe_child(root: Path, relative: str) -> Path | None:
    parts = PurePosixPath(relative).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        return None
    candidate = root.joinpath(*parts).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError:
        return None
    return candidate


def validate_transform(transform: Any, label: str) -> list[str]:
    if transform is None:
        return []
    if not isinstance(transform, dict):
        return [f"{label}.transform must be an object"]
    errors = []
    unknown = sorted(set(transform) - {"flip_x", "flip_y"})
    if unknown:
        errors.append(f"{label}.transform has unknown fields: {', '.join(unknown)}")
    for key in ("flip_x", "flip_y"):
        if key in transform and not isinstance(transform[key], bool):
            errors.append(f"{label}.transform.{key} must be a boolean")
    return errors


def validate_pixel_position(position: Any, label: str) -> list[str]:
    if not isinstance(position, dict):
        return [f"{label}.position must be an object"]
    errors = []
    if not isinstance(position.get("grid"), int) or not 1 <= position["grid"] <= 256:
        errors.append(f"{label}.position.grid must be an integer from 1 to 256")
    for axis in ("x", "y"):
        if not isinstance(position.get(axis), int):
            errors.append(f"{label}.position.{axis} must be an integer")
    return errors


def validate_background_key(source: Any, label: str) -> list[str]:
    if not isinstance(source, dict) or "background_key" not in source:
        return []
    key = source["background_key"]
    if not isinstance(key, dict):
        return [f"{label}.background_key must be an object"]
    errors = []
    color = key.get("color")
    if (
        not isinstance(color, list)
        or len(color) != 3
        or not all(isinstance(channel, int) and 0 <= channel <= 255 for channel in color)
    ):
        errors.append(f"{label}.background_key.color must contain three bytes")
    tolerance = key.get("tolerance")
    if not isinstance(tolerance, int) or not 0 <= tolerance <= 255:
        errors.append(f"{label}.background_key.tolerance must be an integer from 0 to 255")
    softness = key.get("softness")
    if not isinstance(softness, int) or not 1 <= softness <= 255:
        errors.append(f"{label}.background_key.softness must be an integer from 1 to 255")
    return errors


def validate_repair_pair(pair: Any, pair_id: str, asset_root: Path) -> list[str]:
    errors = []
    label = f"repair pair {pair_id!r}"
    if LEVEL_ID.fullmatch(pair_id) is None:
        errors.append(f"{label} has an invalid id")
    if not isinstance(pair, dict):
        return [f"{label} must be an object"]
    if not isinstance(pair.get("label"), str) or not pair["label"].strip():
        errors.append(f"{label} needs a label")
    kind = pair.get("kind")
    if not isinstance(kind, str) or LEVEL_ID.fullmatch(kind) is None:
        errors.append(f"{label} has an invalid kind")
    if pair.get("layer") not in {"floor", "wall", "object", "overlay"}:
        errors.append(f"{label} has an invalid layer")
    errors.extend(validate_task(pair.get("task"), label))
    states = pair.get("states")
    if not isinstance(states, dict):
        return [*errors, f"{label} needs damaged and repaired visual states"]
    for required_state in ("damaged", "repaired"):
        if required_state not in states:
            errors.append(f"{label} needs a {required_state} state")
    for state_name, visual in states.items():
        state_label = f"{label}.states[{state_name!r}]"
        if not isinstance(state_name, str) or LEVEL_ID.fullmatch(state_name) is None:
            errors.append(f"{state_label} has an invalid name")
        if not isinstance(visual, dict):
            errors.append(f"{state_label} must be an object")
            continue
        if visual.get("visible", True) is False:
            continue
        source = visual.get("source")
        source_path = source.get("path") if isinstance(source, dict) else None
        private_path = safe_child(asset_root, source_path) if isinstance(source_path, str) else None
        if private_path is None or not private_path.is_file():
            errors.append(f"{state_label} has an invalid source path")
        source_numeric = ("grid", "x", "y", "width", "height")
        if not isinstance(source, dict) or not all(
            isinstance(source.get(key), int) for key in source_numeric
        ):
            errors.append(f"{state_label} needs an integer source rectangle")
        elif (
            source["grid"] < 1
            or source["x"] < 0
            or source["y"] < 0
            or source["width"] < 1
            or source["height"] < 1
        ):
            errors.append(f"{state_label} has an invalid source rectangle")
        errors.extend(validate_background_key(source, state_label))
    return errors


def validate_task(task: Any, label: str) -> list[str]:
    """Validate optional gameplay requirements while legacy pairs remain usable."""
    if task is None:
        return []
    task_label = f"{label}.task"
    if not isinstance(task, dict):
        return [f"{task_label} must be an object"]
    errors = []
    if task.get("action") not in {"clean", "repair", "clear", "restore", "light"}:
        errors.append(f"{task_label} has an invalid action")
    if task.get("skill") not in {"upkeep", "carpentry", "masonry", "roofing"}:
        errors.append(f"{task_label} has an invalid skill")
    level = task.get("level", 0)
    if not isinstance(level, int) or not 0 <= level <= 3:
        errors.append(f"{task_label}.level must be an integer from 0 to 3")
    xp = task.get("xp", 1)
    if not isinstance(xp, int) or not 0 <= xp <= 20:
        errors.append(f"{task_label}.xp must be an integer from 0 to 20")
    tools = task.get("tools", [])
    if not isinstance(tools, list) or any(
        tool not in {"hammer", "hatchet", "trowel", "ladder"} for tool in tools
    ):
        errors.append(f"{task_label}.tools contains an unknown tool")
    supplies = task.get("supplies", [])
    if not isinstance(supplies, list):
        errors.append(f"{task_label}.supplies must be an array")
    else:
        for index, cost in enumerate(supplies):
            if (
                not isinstance(cost, dict)
                or cost.get("item")
                not in {"kindling", "log", "plank", "nails", "stone", "cloth"}
                or not isinstance(cost.get("amount"), int)
                or not 1 <= cost["amount"] <= 99
            ):
                errors.append(f"{task_label}.supplies[{index}] is invalid")
    return errors


def validate_level(
    level: Any,
    level_id: str,
    asset_root: Path,
    repair_pairs: dict[str, Any] | None = None,
    expected_scene_type: str = "interior",
) -> list[str]:
    errors = []
    if not isinstance(level, dict):
        return ["level must be a JSON object"]
    schema_version = level.get("schema_version")
    if schema_version not in {1, 2, 3, 4}:
        errors.append("schema_version must be 1, 2, 3, or 4")
    if level.get("id") != level_id:
        errors.append("level id must match the save name")
    scene_type = level.get("scene_type", "interior")
    if scene_type != expected_scene_type:
        errors.append(f"scene_type must be {expected_scene_type!r}")
    grid = level.get("grid")
    if not isinstance(grid, dict):
        errors.append("grid is required")
    else:
        for key, low, high in (("width", 4, 128), ("height", 4, 128), ("tile_size", 8, 128)):
            value = grid.get(key)
            if not isinstance(value, int) or not low <= value <= high:
                errors.append(f"grid.{key} must be an integer from {low} to {high}")
    width = grid.get("width") if isinstance(grid, dict) else None
    height = grid.get("height") if isinstance(grid, dict) else None
    placements = level.get("placements")
    if not isinstance(placements, list):
        errors.append("placements must be an array")
    else:
        for index, placement in enumerate(placements):
            if not isinstance(placement, dict):
                errors.append(f"placements[{index}] must be an object")
                continue
            source = placement.get("source", {})
            source_path = source.get("path") if isinstance(source, dict) else None
            private_path = safe_child(asset_root, source_path) if isinstance(source_path, str) else None
            if private_path is None or not private_path.is_file():
                errors.append(f"placements[{index}] has an invalid source path")
            if placement.get("layer") not in {"floor", "wall", "object", "overlay"}:
                errors.append(f"placements[{index}] has an invalid layer")
            if not all(isinstance(placement.get(key), int) for key in ("width", "height")):
                errors.append(f"placements[{index}] needs integer size")
            elif not 1 <= placement["width"] <= 128 or not 1 <= placement["height"] <= 128:
                errors.append(f"placements[{index}] width and height must be from 1 to 128")
            position = placement.get("position")
            if position is None:
                if not all(isinstance(placement.get(key), int) for key in ("x", "y")):
                    errors.append(f"placements[{index}] needs integer x and y or a pixel position")
            else:
                errors.extend(validate_pixel_position(position, f"placements[{index}]"))
            errors.extend(validate_transform(placement.get("transform"), f"placements[{index}]"))
            source_numeric = ("grid", "x", "y", "width", "height")
            if not isinstance(source, dict) or not all(
                isinstance(source.get(key), int) for key in source_numeric
            ):
                errors.append(f"placements[{index}] needs an integer source rectangle")
            elif (
                source["grid"] < 1
                or source["x"] < 0
                or source["y"] < 0
                or source["width"] < 1
                or source["height"] < 1
            ):
                errors.append(f"placements[{index}] has an invalid source rectangle")
            errors.extend(validate_background_key(source, f"placements[{index}].source"))

    templates = level.get("templates", {} if schema_version == 1 else None)
    if not isinstance(templates, dict):
        errors.append("templates must be an object")
        templates = {}
    for template_id, template in templates.items():
        label = f"templates[{template_id!r}]"
        if not isinstance(template_id, str) or LEVEL_ID.fullmatch(template_id) is None:
            errors.append(f"{label} has an invalid id")
        if not isinstance(template, dict):
            errors.append(f"{label} must be an object")
            continue
        if not isinstance(template.get("label"), str) or not template["label"].strip():
            errors.append(f"{label} needs a label")
        kind = template.get("kind")
        if not isinstance(kind, str) or LEVEL_ID.fullmatch(kind) is None:
            errors.append(f"{label} has an invalid kind")
        if template.get("layer") not in {"floor", "wall", "object", "overlay"}:
            errors.append(f"{label} has an invalid layer")
        errors.extend(validate_task(template.get("task"), label))
        states = template.get("states")
        if not isinstance(states, dict) or not states:
            errors.append(f"{label} needs visual states")
            continue
        if "repaired" not in states:
            errors.append(f"{label} needs a repaired state")
        for state_name, visual in states.items():
            state_label = f"{label}.states[{state_name!r}]"
            if not isinstance(state_name, str) or LEVEL_ID.fullmatch(state_name) is None:
                errors.append(f"{state_label} has an invalid name")
            if not isinstance(visual, dict):
                errors.append(f"{state_label} must be an object")
                continue
            if visual.get("visible", True) is False:
                continue
            source = visual.get("source")
            source_path = source.get("path") if isinstance(source, dict) else None
            private_path = (
                safe_child(asset_root, source_path) if isinstance(source_path, str) else None
            )
            if private_path is None or not private_path.is_file():
                errors.append(f"{state_label} has an invalid source path")
            source_numeric = ("grid", "x", "y", "width", "height")
            if not isinstance(source, dict) or not all(
                isinstance(source.get(key), int) for key in source_numeric
            ):
                errors.append(f"{state_label} needs an integer source rectangle")
            elif (
                source["grid"] < 1
                or source["x"] < 0
                or source["y"] < 0
                or source["width"] < 1
                or source["height"] < 1
            ):
                errors.append(f"{state_label} has an invalid source rectangle")
            errors.extend(validate_background_key(source, state_label))

    mutable_ids: set[str] = set()
    for collection_name in ("structures", "fixtures"):
        elements = level.get(collection_name, [] if schema_version == 1 else None)
        if not isinstance(elements, list):
            errors.append(f"{collection_name} must be an array")
            continue
        for index, element in enumerate(elements):
            label = f"{collection_name}[{index}]"
            if not isinstance(element, dict):
                errors.append(f"{label} must be an object")
                continue
            element_id = element.get("id")
            if not isinstance(element_id, str) or LEVEL_ID.fullmatch(element_id) is None:
                errors.append(f"{label} has an invalid id")
            elif element_id in mutable_ids:
                errors.append(f"{label} has a duplicate id")
            else:
                mutable_ids.add(element_id)
            template_id = element.get("template")
            shared_template = (
                (repair_pairs or {}).get(template_id) if isinstance(template_id, str) else None
            )
            if not isinstance(template_id, str) or (
                template_id not in templates and shared_template is None
            ):
                errors.append(f"{label} references an unknown template")
            if not all(isinstance(element.get(key), int) for key in ("width", "height")):
                errors.append(f"{label} needs integer size")
            elif not 1 <= element["width"] <= 128 or not 1 <= element["height"] <= 128:
                errors.append(f"{label} width and height must be from 1 to 128")
            position = element.get("position")
            if position is None:
                if not all(isinstance(element.get(key), int) for key in ("x", "y")):
                    errors.append(f"{label} needs integer x and y or a pixel position")
            else:
                errors.extend(validate_pixel_position(position, label))
            errors.extend(validate_transform(element.get("transform"), label))
            if "layer" in element and element.get("layer") not in {
                "floor",
                "wall",
                "object",
                "overlay",
            }:
                errors.append(f"{label} has an invalid layer override")

            initial_state = element.get("initial_state")
            if isinstance(schema_version, int) and schema_version >= 3:
                template = shared_template or templates.get(template_id, {})
            else:
                template = templates.get(template_id, shared_template or {})
            template_states = template.get("states", {}) if isinstance(template, dict) else {}
            if not isinstance(initial_state, str) or initial_state not in template_states:
                errors.append(f"{label} initial_state must name one of its states")
    required_cell_lists = ("collision", "exits") if expected_scene_type == "interior" else ("collision",)
    for key in required_cell_lists:
        cells = level.get(key)
        if not isinstance(cells, list):
            errors.append(f"{key} must be an array")
        elif isinstance(width, int) and isinstance(height, int):
            for index, cell in enumerate(cells):
                if (
                    not isinstance(cell, dict)
                    or not isinstance(cell.get("x"), int)
                    or not isinstance(cell.get("y"), int)
                    or not 0 <= cell["x"] < width
                    or not 0 <= cell["y"] < height
                ):
                    errors.append(f"{key}[{index}] lies outside the scene")
    if expected_scene_type == "interior":
        entry = level.get("entry")
        if not isinstance(entry, dict) or not all(
            isinstance(entry.get(axis), int) for axis in ("x", "y")
        ):
            errors.append("entry must contain integer x and y")
        elif isinstance(width, int) and isinstance(height, int) and not (
            0 <= entry["x"] < width and 0 <= entry["y"] < height
        ):
            errors.append("entry lies outside the room")
    return errors


class EditorServer(ThreadingHTTPServer):
    asset_root: Path
    level_root: Path
    building_root: Path
    catalog: dict[str, Any]
    catalog_lock: Lock
    repair_pair_path: Path
    repair_pairs: dict[str, Any]
    repair_pair_lock: Lock


def refresh_asset_catalog(server: EditorServer) -> dict[str, Any]:
    """Rescan private assets and atomically replace the served catalog."""
    with server.catalog_lock:
        catalog = catalog_assets(server.asset_root)
        server.catalog = catalog
        return catalog


class EditorHandler(BaseHTTPRequestHandler):
    server: EditorServer

    def log_message(self, message_format: str, *args: object) -> None:
        if self.path.startswith("/asset/"):
            return
        super().log_message(message_format, *args)

    def send_json(self, value: Any, status: HTTPStatus = HTTPStatus.OK) -> None:
        body = (json.dumps(value, indent=2) + "\n").encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def send_path(self, path: Path, cache: bool = False) -> None:
        if not path.is_file():
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        data = path.read_bytes()
        media_type = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", media_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "private, max-age=3600" if cache else "no-store")
        self.end_headers()
        self.wfile.write(data)

    def read_json_body(self) -> Any | None:
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid content length")
            return None
        if not 0 < length <= MAX_REQUEST_BYTES:
            self.send_error(HTTPStatus.REQUEST_ENTITY_TOO_LARGE)
            return None
        try:
            return json.loads(self.rfile.read(length))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid JSON")
            return None

    def save_repair_pairs(self) -> None:
        document = {"schema_version": 1, "pairs": self.server.repair_pairs}
        destination = self.server.repair_pair_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        serialized = json.dumps(document, indent=2) + "\n"
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=destination.parent, delete=False
        ) as temporary:
            temporary.write(serialized)
            temporary_path = Path(temporary.name)
        temporary_path.replace(destination)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request_path = unquote(urlparse(self.path).path)
        if request_path == "/api/catalog":
            self.send_json(self.server.catalog)
            return
        if request_path == "/api/repair-pairs":
            self.send_json({"schema_version": 1, "pairs": self.server.repair_pairs})
            return
        if request_path == "/api/levels":
            levels = sorted(path.stem for path in self.server.level_root.glob("*.json"))
            self.send_json({"levels": levels})
            return
        if request_path.startswith("/api/levels/"):
            level_id = request_path.removeprefix("/api/levels/")
            if not LEVEL_ID.fullmatch(level_id):
                self.send_error(HTTPStatus.BAD_REQUEST, "invalid level id")
                return
            self.send_path(self.server.level_root / f"{level_id}.json")
            return
        if request_path == "/api/buildings":
            buildings = sorted(path.stem for path in self.server.building_root.glob("*.json"))
            self.send_json({"buildings": buildings})
            return
        if request_path.startswith("/api/buildings/"):
            building_id = request_path.removeprefix("/api/buildings/")
            if not LEVEL_ID.fullmatch(building_id):
                self.send_error(HTTPStatus.BAD_REQUEST, "invalid building id")
                return
            self.send_path(self.server.building_root / f"{building_id}.json")
            return
        if request_path.startswith("/asset/"):
            path = safe_child(self.server.asset_root, request_path.removeprefix("/asset/"))
            if path is None:
                self.send_error(HTTPStatus.BAD_REQUEST, "invalid asset path")
                return
            self.send_path(path, cache=True)
            return
        static_name = "index.html" if request_path in {"", "/"} else request_path.lstrip("/")
        path = safe_child(EDITOR_ROOT, static_name)
        if path is None:
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid editor path")
            return
        self.send_path(path)

    def do_PUT(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request_path = unquote(urlparse(self.path).path)
        if request_path.startswith("/api/repair-pairs/"):
            pair_id = request_path.removeprefix("/api/repair-pairs/")
            if not LEVEL_ID.fullmatch(pair_id):
                self.send_error(HTTPStatus.BAD_REQUEST, "invalid repair pair id")
                return
            pair = self.read_json_body()
            if pair is None:
                return
            errors = validate_repair_pair(pair, pair_id, self.server.asset_root)
            if errors:
                self.send_json({"saved": False, "errors": errors}, HTTPStatus.UNPROCESSABLE_ENTITY)
                return
            with self.server.repair_pair_lock:
                self.server.repair_pairs[pair_id] = pair
                self.save_repair_pairs()
            self.send_json(
                {
                    "saved": True,
                    "id": pair_id,
                    "path": str(self.server.repair_pair_path.relative_to(ROOT)),
                }
            )
            return
        if request_path.startswith("/api/levels/"):
            scene_id = request_path.removeprefix("/api/levels/")
            scene_type = "interior"
            destination_root = self.server.level_root
        elif request_path.startswith("/api/buildings/"):
            scene_id = request_path.removeprefix("/api/buildings/")
            scene_type = "building"
            destination_root = self.server.building_root
        else:
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        if not LEVEL_ID.fullmatch(scene_id):
            self.send_error(HTTPStatus.BAD_REQUEST, f"invalid {scene_type} id")
            return
        level = self.read_json_body()
        if level is None:
            return
        errors = validate_level(
            level,
            scene_id,
            self.server.asset_root,
            self.server.repair_pairs,
            expected_scene_type=scene_type,
        )
        if errors:
            self.send_json({"saved": False, "errors": errors}, HTTPStatus.UNPROCESSABLE_ENTITY)
            return
        destination_root.mkdir(parents=True, exist_ok=True)
        destination = destination_root / f"{scene_id}.json"
        serialized = json.dumps(level, indent=2) + "\n"
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=destination_root, delete=False
        ) as temporary:
            temporary.write(serialized)
            temporary_path = Path(temporary.name)
        temporary_path.replace(destination)
        self.send_json({"saved": True, "path": str(destination.relative_to(ROOT))})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request_path = unquote(urlparse(self.path).path)
        if request_path == "/api/catalog/refresh":
            self.send_json(refresh_asset_catalog(self.server))
            return
        self.send_error(HTTPStatus.NOT_FOUND)

    def do_DELETE(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request_path = unquote(urlparse(self.path).path)
        if not request_path.startswith("/api/repair-pairs/"):
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        pair_id = request_path.removeprefix("/api/repair-pairs/")
        if not LEVEL_ID.fullmatch(pair_id):
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid repair pair id")
            return
        used_by = []
        for scene_type, root in (
            ("interior", self.server.level_root),
            ("building", self.server.building_root),
        ):
            for level_path in sorted(root.glob("*.json")):
                level = json.loads(level_path.read_text(encoding="utf-8"))
                instances = [*level.get("structures", []), *level.get("fixtures", [])]
                if any(instance.get("template") == pair_id for instance in instances):
                    used_by.append(f"{scene_type}/{level_path.stem}")
        if used_by:
            self.send_json(
                {
                    "deleted": False,
                    "errors": [f"repair pair is used by: {', '.join(used_by)}"],
                },
                HTTPStatus.CONFLICT,
            )
            return
        with self.server.repair_pair_lock:
            if pair_id not in self.server.repair_pairs:
                self.send_error(HTTPStatus.NOT_FOUND, "unknown repair pair")
                return
            del self.server.repair_pairs[pair_id]
            self.save_repair_pairs()
        self.send_json({"deleted": True, "id": pair_id})


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=7790)
    parser.add_argument("--assets", type=Path, default=DEFAULT_ASSET_ROOT)
    args = parser.parse_args()

    print(f"Indexing private assets under {args.assets} …")
    catalog = catalog_assets(args.assets)
    server = EditorServer((args.bind, args.port), EditorHandler)
    server.asset_root = args.assets.resolve()
    server.level_root = LEVEL_ROOT
    server.building_root = BUILDING_ROOT
    server.catalog = catalog
    server.catalog_lock = Lock()
    server.repair_pair_path = REPAIR_PAIR_PATH
    if REPAIR_PAIR_PATH.is_file():
        repair_pair_document = json.loads(REPAIR_PAIR_PATH.read_text(encoding="utf-8"))
        server.repair_pairs = repair_pair_document.get("pairs", {})
    else:
        server.repair_pairs = {}
    server.repair_pair_lock = Lock()
    print(f"Indexed {catalog['count']} images.")
    print(f"Scene editor: http://{args.bind}:{args.port}")
    print("This server is local-only and serves licensed source art. Press Ctrl-C to stop.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
