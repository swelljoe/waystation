from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

from level_editor import safe_child, validate_level


class LevelEditorValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.assets = Path(self.temporary.name)
        Image.new("RGBA", (96, 96)).save(self.assets / "sheet.png")
        self.level = {
            "schema_version": 1,
            "id": "test-room",
            "grid": {"width": 8, "height": 6, "tile_size": 32},
            "entry": {"x": 3, "y": 4},
            "exits": [{"x": 3, "y": 5}],
            "collision": [{"x": 0, "y": 0}],
            "templates": {},
            "structures": [],
            "fixtures": [],
            "placements": [
                {
                    "layer": "object",
                    "x": 1,
                    "y": 1,
                    "width": 2,
                    "height": 2,
                    "source": {
                        "path": "sheet.png",
                        "grid": 48,
                        "x": 0,
                        "y": 0,
                        "width": 2,
                        "height": 2,
                    },
                }
            ],
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_valid_level_passes(self) -> None:
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_stamp_may_overhang_room_edge(self) -> None:
        self.level["placements"][0]["x"] = 7
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_stamp_may_be_outside_after_room_is_resized(self) -> None:
        self.level["placements"][0]["x"] = -3
        self.level["placements"][0]["y"] = 12
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_stamp_dimensions_remain_bounded(self) -> None:
        self.level["placements"][0]["width"] = 129
        self.assertIn(
            "placements[0] width and height must be from 1 to 128",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_asset_path_cannot_escape_private_root(self) -> None:
        self.assertIsNone(safe_child(self.assets, "../secret.png"))

    def test_mutable_fixture_with_native_state_crops_passes(self) -> None:
        self.level["schema_version"] = 2
        source = {"path": "sheet.png", "grid": 48, "x": 0, "y": 0, "width": 1, "height": 2}
        self.level["templates"]["door"] = {
            "label": "room door",
            "kind": "door",
            "layer": "object",
            "states": {"damaged": {"source": source}, "repaired": {"source": source}},
        }
        self.level["fixtures"].append(
            {
                "id": "room-door-01",
                "template": "door",
                "x": 2,
                "y": 1,
                "width": 2,
                "height": 3,
                "initial_state": "damaged",
            }
        )
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_mutable_ids_are_unique_across_structures_and_fixtures(self) -> None:
        self.level["schema_version"] = 2
        source = {"path": "sheet.png", "grid": 48, "x": 0, "y": 0, "width": 1, "height": 1}
        self.level["templates"]["wood"] = {
            "label": "wood",
            "kind": "floor",
            "layer": "floor",
            "states": {"damaged": {"source": source}, "repaired": {"source": source}},
        }
        element = {
            "id": "wood-01",
            "template": "wood",
            "x": 0,
            "y": 0,
            "width": 2,
            "height": 2,
            "initial_state": "damaged",
        }
        self.level["structures"].append(element)
        self.level["fixtures"].append(element.copy())
        self.assertIn(
            "fixtures[0] has a duplicate id",
            validate_level(self.level, "test-room", self.assets),
        )


if __name__ == "__main__":
    unittest.main()
