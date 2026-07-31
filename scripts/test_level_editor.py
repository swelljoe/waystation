from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

from level_editor import refresh_asset_catalog, safe_child, validate_level, validate_repair_pair


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

    def test_searchable_discovery_hotspot_passes(self) -> None:
        self.level["interactions"] = [
            {
                "id": "bible-nightstand",
                "label": "dusty nightstand",
                "kind": "search",
                "discovery": "gideon_bible",
                "position": {"grid": 8, "x": 22, "y": 8},
                "width": 32,
                "height": 48,
            }
        ]
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_searchable_discovery_hotspots_require_unique_ids(self) -> None:
        interaction = {
            "id": "bible-nightstand",
            "label": "dusty nightstand",
            "kind": "search",
            "discovery": "gideon_bible",
            "position": {"grid": 8, "x": 22, "y": 8},
            "width": 32,
            "height": 48,
        }
        self.level["interactions"] = [interaction, interaction.copy()]
        self.assertIn(
            "interactions[1] has a duplicate id",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_collision_accepts_mixed_legacy_and_fine_pixel_grids(self) -> None:
        self.level["collision"].append({"grid": 8, "x": 17, "y": 9})
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_fine_collision_must_fit_inside_scene_pixels(self) -> None:
        self.level["collision"] = [{"grid": 16, "x": 16, "y": 0}]
        self.assertIn(
            "collision[0] lies outside the scene",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_collision_grid_is_bounded(self) -> None:
        self.level["collision"] = [{"grid": 0, "x": 0, "y": 0}]
        self.assertIn(
            "collision[0].grid must be an integer from 1 to 256",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_stamp_may_overhang_room_edge(self) -> None:
        self.level["placements"][0]["x"] = 7
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_placement_occlusion_flag_must_be_boolean(self) -> None:
        self.level["placements"][0]["occludes_player"] = "yes"
        self.assertIn(
            "placements[0].occludes_player must be a boolean",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_repairable_occlusion_flag_must_be_boolean(self) -> None:
        self.level["structures"] = [
            {
                "id": "gable-01",
                "template": "mirror",
                "x": 1,
                "y": 1,
                "width": 1,
                "height": 1,
                "initial_state": "damaged",
                "occludes_player": 1,
            }
        ]
        self.level["templates"] = {
            "mirror": {
                "label": "Mirror",
                "kind": "fixture",
                "layer": "object",
                "states": {
                    "damaged": {"source": self.level["placements"][0]["source"]},
                    "repaired": {"source": self.level["placements"][0]["source"]},
                },
            }
        }
        self.assertIn(
            "structures[0].occludes_player must be a boolean",
            validate_level(self.level, "test-room", self.assets),
        )

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

    def test_stamp_accepts_boolean_flips(self) -> None:
        self.level["placements"][0]["transform"] = {"flip_x": True, "flip_y": False}
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_transform_rejects_non_boolean_flip(self) -> None:
        self.level["placements"][0]["transform"] = {"flip_x": "yes"}
        self.assertIn(
            "placements[0].transform.flip_x must be a boolean",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_schema_four_accepts_per_placement_snap_grid(self) -> None:
        self.level["schema_version"] = 4
        placement = self.level["placements"][0]
        del placement["x"]
        del placement["y"]
        placement["position"] = {"grid": 16, "x": 3, "y": -1}

        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_schema_five_accepts_portable_tools_with_condition(self) -> None:
        self.level["schema_version"] = 5
        self.level["items"] = [
            {
                "id": "shed-hammer-01",
                "label": "claw hammer",
                "tool": "hammer",
                "condition": "broken",
                "layer": "object",
                "position": {"grid": 8, "x": 4, "y": 5},
                "width": 1,
                "height": 1,
                "source": {
                    "path": "sheet.png",
                    "grid": 48,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                },
            }
        ]

        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

    def test_portable_tool_ids_share_the_mutable_state_namespace(self) -> None:
        self.level["schema_version"] = 5
        self.level["items"] = [
            {
                "id": "hammer-01",
                "label": "hammer",
                "tool": "hammer",
                "condition": "serviceable",
                "layer": "object",
                "position": {"grid": 8, "x": 1, "y": 1},
                "width": 1,
                "height": 1,
                "source": {
                    "path": "sheet.png",
                    "grid": 48,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                },
            }
        ]
        self.level["structures"] = [
            {
                "id": "hammer-01",
                "template": "missing-pair",
                "x": 1,
                "y": 1,
                "width": 1,
                "height": 1,
                "initial_state": "damaged",
            }
        ]

        self.assertIn(
            "structures[0] has a duplicate id",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_pixel_position_rejects_invalid_grid(self) -> None:
        self.level["placements"][0]["position"] = {"grid": 0, "x": 1, "y": 2}
        self.assertIn(
            "placements[0].position.grid must be an integer from 1 to 256",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_building_scene_does_not_require_interior_entry_or_exits(self) -> None:
        self.level["scene_type"] = "building"
        del self.level["entry"]
        del self.level["exits"]

        self.assertEqual(
            validate_level(
                self.level,
                "test-room",
                self.assets,
                expected_scene_type="building",
            ),
            [],
        )

    def test_smart_slice_background_key_is_validated(self) -> None:
        self.level["placements"][0]["source"]["background_key"] = {
            "color": [253, 253, 253],
            "tolerance": 24,
            "softness": 16,
        }
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

        self.level["placements"][0]["source"]["background_key"]["color"] = [300]
        self.assertIn(
            "placements[0].source.background_key.color must contain three bytes",
            validate_level(self.level, "test-room", self.assets),
        )

    def test_asset_path_cannot_escape_private_root(self) -> None:
        self.assertIsNone(safe_child(self.assets, "../secret.png"))

    def test_catalog_refresh_discovers_new_assets_without_restarting(self) -> None:
        from threading import Lock
        from types import SimpleNamespace

        server = SimpleNamespace(
            asset_root=self.assets,
            catalog={"count": 0, "assets": []},
            catalog_lock=Lock(),
        )
        first = refresh_asset_catalog(server)
        Image.new("RGBA", (32, 48)).save(self.assets / "new-custom-piece.png")
        refreshed = refresh_asset_catalog(server)

        self.assertEqual(first["count"], 1)
        self.assertEqual(refreshed["count"], 2)
        self.assertIn("new-custom-piece.png", {asset["path"] for asset in refreshed["assets"]})
        self.assertIs(server.catalog, refreshed)

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

        self.level["fixtures"][0]["layer"] = "overlay"
        self.assertEqual(validate_level(self.level, "test-room", self.assets), [])

        self.level["fixtures"][0]["layer"] = "ceiling"
        self.assertIn(
            "fixtures[0] has an invalid layer override",
            validate_level(self.level, "test-room", self.assets),
        )

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

    def test_schema_three_resolves_a_shared_repair_pair(self) -> None:
        self.level["schema_version"] = 3
        self.level["templates"] = {}
        source = {"path": "sheet.png", "grid": 48, "x": 0, "y": 0, "width": 1, "height": 1}
        pair = {
            "label": "cracked plaster",
            "kind": "plaster",
            "layer": "wall",
            "states": {"damaged": {"source": source}, "repaired": {"source": source}},
        }
        self.level["structures"].append(
            {
                "id": "plaster-01",
                "template": "cracked-plaster",
                "x": 0,
                "y": 0,
                "width": 1,
                "height": 1,
                "initial_state": "damaged",
            }
        )

        self.assertEqual(
            validate_level(
                self.level,
                "test-room",
                self.assets,
                {"cracked-plaster": pair},
            ),
            [],
        )

    def test_repair_pairs_may_share_either_source_crop(self) -> None:
        shared = {"path": "sheet.png", "grid": 48, "x": 0, "y": 0, "width": 1, "height": 1}
        first = {
            "label": "broken wall a",
            "kind": "wall",
            "layer": "wall",
            "states": {"damaged": {"source": shared}, "repaired": {"source": shared}},
        }
        second = {
            "label": "broken wall b",
            "kind": "wall",
            "layer": "wall",
            "states": {"damaged": {"source": {**shared, "x": 1}}, "repaired": {"source": shared}},
        }

        self.assertEqual(validate_repair_pair(first, "wall-a", self.assets), [])
        self.assertEqual(validate_repair_pair(second, "wall-b", self.assets), [])

    def test_repair_pair_accepts_task_requirements_and_invisible_cleanup(self) -> None:
        pair = {
            "label": "trash pile",
            "kind": "debris",
            "layer": "object",
            "task": {
                "action": "clean",
                "skill": "upkeep",
                "level": 0,
                "tools": [],
                "supplies": [],
                "xp": 1,
            },
            "states": {
                "damaged": {
                    "source": {
                        "path": "sheet.png",
                        "grid": 48,
                        "x": 0,
                        "y": 0,
                        "width": 1,
                        "height": 1,
                    }
                },
                "repaired": {"visible": False},
            },
        }

        self.assertEqual(validate_repair_pair(pair, "trash-pile", self.assets), [])


if __name__ == "__main__":
    unittest.main()
