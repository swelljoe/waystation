#!/usr/bin/env python3
"""Serve the private, local-only Waystation interior level editor."""

from __future__ import annotations

import argparse
import json
import mimetypes
import re
import tempfile
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import unquote, urlparse

from asset_catalog import DEFAULT_ASSET_ROOT, catalog_assets

ROOT = Path(__file__).resolve().parent.parent
EDITOR_ROOT = ROOT / "tools/level-editor"
LEVEL_ROOT = ROOT / "content/interiors"
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


def validate_level(level: Any, level_id: str, asset_root: Path) -> list[str]:
    errors = []
    if not isinstance(level, dict):
        return ["level must be a JSON object"]
    schema_version = level.get("schema_version")
    if schema_version not in {1, 2}:
        errors.append("schema_version must be 1 or 2")
    if level.get("id") != level_id:
        errors.append("level id must match the save name")
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
            numeric = ("x", "y", "width", "height")
            if not all(isinstance(placement.get(key), int) for key in numeric):
                errors.append(f"placements[{index}] needs integer position and size")
            elif not 1 <= placement["width"] <= 128 or not 1 <= placement["height"] <= 128:
                errors.append(f"placements[{index}] width and height must be from 1 to 128")
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
            if not isinstance(template_id, str) or template_id not in templates:
                errors.append(f"{label} references an unknown template")
            numeric = ("x", "y", "width", "height")
            if not all(isinstance(element.get(key), int) for key in numeric):
                errors.append(f"{label} needs integer position and size")
            elif not 1 <= element["width"] <= 128 or not 1 <= element["height"] <= 128:
                errors.append(f"{label} width and height must be from 1 to 128")

            initial_state = element.get("initial_state")
            template_states = (
                templates.get(template_id, {}).get("states", {})
                if isinstance(template_id, str)
                else {}
            )
            if not isinstance(initial_state, str) or initial_state not in template_states:
                errors.append(f"{label} initial_state must name one of its states")
    for key in ("collision", "exits"):
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
                    errors.append(f"{key}[{index}] lies outside the room")
    entry = level.get("entry")
    if not isinstance(entry, dict) or not all(isinstance(entry.get(axis), int) for axis in ("x", "y")):
        errors.append("entry must contain integer x and y")
    elif isinstance(width, int) and isinstance(height, int) and not (
        0 <= entry["x"] < width and 0 <= entry["y"] < height
    ):
        errors.append("entry lies outside the room")
    return errors


class EditorServer(ThreadingHTTPServer):
    asset_root: Path
    level_root: Path
    catalog: dict[str, Any]


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

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        request_path = unquote(urlparse(self.path).path)
        if request_path == "/api/catalog":
            self.send_json(self.server.catalog)
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
        if not request_path.startswith("/api/levels/"):
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        level_id = request_path.removeprefix("/api/levels/")
        if not LEVEL_ID.fullmatch(level_id):
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid level id")
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid content length")
            return
        if not 0 < length <= MAX_REQUEST_BYTES:
            self.send_error(HTTPStatus.REQUEST_ENTITY_TOO_LARGE)
            return
        try:
            level = json.loads(self.rfile.read(length))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid JSON")
            return
        errors = validate_level(level, level_id, self.server.asset_root)
        if errors:
            self.send_json({"saved": False, "errors": errors}, HTTPStatus.UNPROCESSABLE_ENTITY)
            return
        self.server.level_root.mkdir(parents=True, exist_ok=True)
        destination = self.server.level_root / f"{level_id}.json"
        serialized = json.dumps(level, indent=2) + "\n"
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=self.server.level_root, delete=False
        ) as temporary:
            temporary.write(serialized)
            temporary_path = Path(temporary.name)
        temporary_path.replace(destination)
        self.send_json({"saved": True, "path": str(destination.relative_to(ROOT))})


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
    server.catalog = catalog
    print(f"Indexed {catalog['count']} images.")
    print(f"Level editor: http://{args.bind}:{args.port}")
    print("This server is local-only and serves licensed source art. Press Ctrl-C to stop.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
