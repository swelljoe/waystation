from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

from PIL import Image


SCRIPT = Path(__file__).with_name("build-assets.py")
SPEC = importlib.util.spec_from_file_location("waystation_build_assets", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
BUILD_ASSETS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD_ASSETS)


class InteriorRenderingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.assets = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def level_with(self, placement: dict[str, object]) -> dict[str, object]:
        return {
            "grid": {"width": 4, "height": 4, "tile_size": 32},
            "background": "#000000",
            "floor_line": "#000000",
            "placements": [placement],
        }

    def test_source_grid_does_not_scale_stamp(self) -> None:
        Image.new("RGBA", (8, 8), "#ff0000").save(self.assets / "sheet.png")
        level = self.level_with(
            {
                "layer": "object",
                "x": 1,
                "y": 1,
                "width": 1,
                "height": 1,
                "source": {
                    "path": "sheet.png",
                    "grid": 4,
                    "x": 1,
                    "y": 1,
                    "width": 1,
                    "height": 1,
                },
            }
        )

        rendered = BUILD_ASSETS.render_interior(level, self.assets)

        self.assertEqual(rendered.getpixel((32, 32)), (255, 0, 0, 255))
        self.assertEqual(rendered.getpixel((35, 35)), (255, 0, 0, 255))
        self.assertEqual(rendered.getpixel((36, 32)), (0, 0, 0, 255))

    def test_repeated_stamp_tiles_without_scaling(self) -> None:
        source = Image.new("RGBA", (2, 2), "#ff0000")
        source.putpixel((1, 0), (0, 255, 0, 255))
        source.save(self.assets / "sheet.png")
        level = self.level_with(
            {
                "layer": "floor",
                "x": 0,
                "y": 0,
                "width": 1,
                "height": 1,
                "repeat": True,
                "source": {
                    "path": "sheet.png",
                    "grid": 2,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                },
            }
        )

        rendered = BUILD_ASSETS.render_interior(level, self.assets)

        self.assertEqual(rendered.getpixel((0, 0)), (255, 0, 0, 255))
        self.assertEqual(rendered.getpixel((1, 0)), (0, 255, 0, 255))
        self.assertEqual(rendered.getpixel((2, 0)), (255, 0, 0, 255))

    def test_baked_stamp_flips_both_axes_without_scaling(self) -> None:
        source = Image.new("RGBA", (2, 2))
        source.putdata(
            [
                (255, 0, 0, 255),
                (0, 255, 0, 255),
                (0, 0, 255, 255),
                (255, 255, 0, 255),
            ]
        )
        source.save(self.assets / "sheet.png")
        level = self.level_with(
            {
                "layer": "object",
                "x": 0,
                "y": 0,
                "width": 1,
                "height": 1,
                "transform": {"flip_x": True, "flip_y": True},
                "source": {
                    "path": "sheet.png",
                    "grid": 2,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                },
            }
        )

        rendered = BUILD_ASSETS.render_interior(level, self.assets)

        self.assertEqual(rendered.getpixel((0, 0)), (255, 255, 0, 255))
        self.assertEqual(rendered.getpixel((1, 0)), (0, 0, 255, 255))
        self.assertEqual(rendered.getpixel((0, 1)), (0, 255, 0, 255))
        self.assertEqual(rendered.getpixel((1, 1)), (255, 0, 0, 255))

    def test_per_placement_snap_grid_resolves_to_exact_pixels(self) -> None:
        Image.new("RGBA", (2, 2), "#ff0000").save(self.assets / "sheet.png")
        level = self.level_with(
            {
                "layer": "object",
                "position": {"grid": 16, "x": 1, "y": 2},
                "width": 1,
                "height": 1,
                "source": {
                    "path": "sheet.png",
                    "grid": 2,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                },
            }
        )

        rendered = BUILD_ASSETS.render_interior(level, self.assets)

        self.assertEqual(rendered.getpixel((16, 32)), (255, 0, 0, 255))
        self.assertEqual(rendered.getpixel((15, 32)), (0, 0, 0, 255))

    def test_mutable_states_are_extracted_instead_of_baked(self) -> None:
        Image.new("RGBA", (8, 4), "#ff0000").save(self.assets / "sheet.png")
        source = {"path": "sheet.png", "grid": 4, "x": 0, "y": 0, "width": 1, "height": 1}
        level = self.level_with({
            "layer": "floor",
            "x": 0,
            "y": 0,
            "width": 1,
            "height": 1,
            "source": source,
        })
        level.update(
            {
                "id": "test-room",
                "templates": {
                    "floor": {
                        "label": "floorboard",
                        "kind": "floor",
                        "layer": "floor",
                        "states": {
                            "damaged": {"source": source},
                            "repaired": {"source": {**source, "x": 1}},
                        },
                    }
                },
                "structures": [
                    {
                        "id": "floor-01",
                        "template": "floor",
                        "x": 1,
                        "y": 1,
                        "width": 1,
                        "height": 1,
                        "initial_state": "damaged",
                    }
                ],
                "fixtures": [],
            }
        )
        output = self.assets / "runtime"
        interiors = output / "interiors"
        interiors.mkdir(parents=True)

        records = BUILD_ASSETS.write_mutable_interior_art(
            level, self.assets, output, interiors
        )
        background = BUILD_ASSETS.render_interior({**level, "placements": []}, self.assets)

        self.assertEqual(background.getpixel((32, 32)), (0, 0, 0, 255))
        self.assertEqual(len(records), 2)
        with Image.open(interiors / "test-room" / "floor--damaged.png") as damaged:
            self.assertEqual(damaged.size, (4, 4))

    def test_distinct_shared_pairs_can_reuse_the_same_repaired_crop(self) -> None:
        Image.new("RGBA", (12, 4), "#ff0000").save(self.assets / "sheet.png")
        source = {"path": "sheet.png", "grid": 4, "x": 0, "y": 0, "width": 1, "height": 1}
        repaired = {**source, "x": 2}
        pair = {
            "label": "wall damage",
            "kind": "wall",
            "layer": "wall",
            "states": {"damaged": {"source": source}, "repaired": {"source": repaired}},
        }
        repair_pairs = {
            "wall-crack-a": pair,
            "wall-crack-b": {
                **pair,
                "states": {
                    "damaged": {"source": {**source, "x": 1}},
                    "repaired": {"source": repaired},
                },
            },
        }
        level = {
            **self.level_with({
                "layer": "floor",
                "x": 0,
                "y": 0,
                "width": 1,
                "height": 1,
                "source": source,
            }),
            "schema_version": 3,
            "id": "test-room",
            "templates": {},
            "structures": [
                {"id": "a-01", "template": "wall-crack-a"},
                {"id": "b-01", "template": "wall-crack-b"},
            ],
            "fixtures": [],
        }
        output = self.assets / "runtime"
        interiors = output / "interiors"
        interiors.mkdir(parents=True)

        records = BUILD_ASSETS.write_mutable_interior_art(
            level, self.assets, output, interiors, repair_pairs
        )

        self.assertEqual(len(records), 4)
        self.assertTrue((interiors / "test-room" / "wall-crack-a--repaired.png").is_file())
        self.assertTrue((interiors / "test-room" / "wall-crack-b--repaired.png").is_file())


if __name__ == "__main__":
    unittest.main()
