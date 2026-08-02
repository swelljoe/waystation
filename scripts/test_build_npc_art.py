from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

SCRIPT = Path(__file__).with_name("build-npc-art.py")
SPEC = importlib.util.spec_from_file_location("waystation_build_npc_art", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
ART = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ART)

# A hair ramp and what it becomes, in the shape the wardrobe records them.
DRAWN_IN = ["#260d14", "#6a1108", "#a42600"]
BECOMES = ["#101010", "#202020", "#303030"]


def wardrobe() -> dict:
    """A miniature of the real thing: a templated face, a recoloured piece, and
    a piece that ships baked colours."""
    return {
        "lpc_revision": "test",
        "license_policy": "permissive",
        "body_types": [{"id": "male", "weight": 1}],
        "palettes": {
            "skin": [{"color": "light", "weight": 1}, {"color": "brown", "weight": 1}],
            "hair": [{"color": "black", "weight": 1}, {"color": "sandy", "weight": 1}],
            "hair_old": [{"color": "gray", "weight": 1}],
            "cloth_muted": [{"color": "brown", "weight": 1}, {"color": "tan", "weight": 1}],
            "cloth_bright": [{"color": "brown", "weight": 1}],
        },
        "materials": {"hair": {"black": BECOMES, "sandy": ["#aa0000"] * 3}},
        "slots": {
            "head": {
                "chance": 1.0,
                "items": [
                    {
                        "id": "heads_human_male",
                        "name": "Human Male",
                        "type": "head",
                        "bodies": ["male"],
                        "field": "none",
                        "source": "none",
                        "weight": 1,
                        "licenses": ["CC0"],
                        "layers": [{"z": 100, "paths": {"male": "head/male/"}}],
                    }
                ],
            },
            "expression": {
                "chance": 1.0,
                "items": [
                    {
                        "id": "face_sad",
                        "name": "Sad",
                        "type": "expression",
                        "bodies": ["male"],
                        "field": "none",
                        "source": "none",
                        "weight": 1,
                        "licenses": ["CC0"],
                        "layers": [{"z": 101, "paths": {"male": "head/faces/${head}/sad/"}}],
                        "replace": {"head": {"Human_Male": "male", "Human_Female": "female"}},
                    }
                ],
            },
            "hair": {
                "chance": 1.0,
                "items": [
                    {
                        "id": "hair_plain",
                        "name": "Plain",
                        "type": "hair",
                        "bodies": ["male"],
                        "field": "recolor",
                        "source": "hair",
                        "weight": 1,
                        "licenses": ["CC0"],
                        "layers": [{"z": 120, "paths": {"male": "hair/plain/"}}],
                        "recolor": {"material": "hair", "from": DRAWN_IN},
                    }
                ],
            },
            "shoes": {
                "chance": 1.0,
                "items": [
                    {
                        "id": "feet_shoes",
                        "name": "Basic Shoes",
                        "type": "shoes",
                        "bodies": ["male"],
                        "field": "variant",
                        "source": "cloth",
                        "weight": 1,
                        "licenses": ["CC0"],
                        "options": ["brown", "tan"],
                        "layers": [{"z": 110, "paths": {"male": "feet/shoes/"}}],
                    }
                ],
            },
        },
    }


class WantedFileTests(unittest.TestCase):
    def test_a_templated_path_asks_for_every_head_it_could_sit_under(self) -> None:
        item = wardrobe()["slots"]["expression"]["items"][0]

        self.assertEqual(
            sorted(ART.substitutions(item, "head/faces/${head}/sad/")),
            ["head/faces/female/sad/", "head/faces/male/sad/"],
        )

    def test_a_baked_colour_piece_asks_for_one_sheet_per_colour(self) -> None:
        item = wardrobe()["slots"]["shoes"]["items"][0]

        self.assertEqual(
            ART.sheets_for(item, "feet/shoes/"),
            ["feet/shoes/walk/brown.png", "feet/shoes/walk/tan.png"],
        )

    def test_a_recoloured_piece_asks_for_one_sheet_however_many_colours(self) -> None:
        item = wardrobe()["slots"]["hair"]["items"][0]

        self.assertEqual(ART.sheets_for(item, "hair/plain/"), ["hair/plain/walk.png"])

    def test_every_sheet_the_wardrobe_names_is_gathered_once(self) -> None:
        wanted = ART.wanted_files(wardrobe())

        self.assertEqual(
            sorted(wanted),
            [
                "feet/shoes/walk/brown.png",
                "feet/shoes/walk/tan.png",
                "hair/plain/walk.png",
                "head/faces/female/sad/walk.png",
                "head/faces/male/sad/walk.png",
                "head/male/walk.png",
            ],
        )
        self.assertEqual(set(wanted.values()), {"lpc"}, "nothing here is ours")

    def test_a_project_drawn_piece_is_copied_from_our_own_tree(self) -> None:
        data = wardrobe()
        data["slots"]["hair"]["items"][0]["origin"] = "overlay"

        self.assertEqual(ART.wanted_files(data)["hair/plain/walk.png"], "overlay")


class RecolourTests(unittest.TestCase):
    def test_the_swap_maps_the_ramp_a_piece_was_drawn_in_to_the_one_rolled(self) -> None:
        item = wardrobe()["slots"]["hair"]["items"][0]

        table = ART.swap_table(wardrobe(), item, "black")

        self.assertEqual(table[(0x26, 0x0D, 0x14)], (0x10, 0x10, 0x10))
        self.assertEqual(len(table), 3)

    def test_a_colour_the_material_does_not_have_swaps_nothing(self) -> None:
        item = wardrobe()["slots"]["hair"]["items"][0]

        self.assertEqual(ART.swap_table(wardrobe(), item, "chartreuse"), {})

    def test_a_piece_with_no_palette_swaps_nothing(self) -> None:
        item = wardrobe()["slots"]["head"]["items"][0]

        self.assertEqual(ART.swap_table(wardrobe(), item, "black"), {})

    def test_recolouring_leaves_transparency_and_unnamed_colours_alone(self) -> None:
        image = Image.new("RGBA", (3, 1), (0, 0, 0, 0))
        image.putpixel((0, 0), (0x26, 0x0D, 0x14, 255))
        image.putpixel((1, 0), (0x77, 0x77, 0x77, 255))
        # Off-ramp because it is transparent, not because of its colour.
        image.putpixel((2, 0), (0x26, 0x0D, 0x14, 0))

        out = ART.recolour(image, {(0x26, 0x0D, 0x14): (0x10, 0x10, 0x10)})

        self.assertEqual(out.getpixel((0, 0)), (0x10, 0x10, 0x10, 255))
        self.assertEqual(out.getpixel((1, 0)), (0x77, 0x77, 0x77, 255))
        self.assertEqual(out.getpixel((2, 0))[3], 0)


class ResolveTests(unittest.TestCase):
    def test_a_face_path_is_named_for_the_head_wearing_it(self) -> None:
        data = wardrobe()
        item = data["slots"]["expression"]["items"][0]
        character = [{"slot": "head", "item": "heads_human_male", "color": ""}]

        resolved = ART.resolve("head/faces/${head}/sad/", item, character, data)

        self.assertEqual(resolved, "head/faces/male/sad/")

    def test_a_face_with_no_head_above_it_draws_nothing(self) -> None:
        data = wardrobe()
        item = data["slots"]["expression"]["items"][0]

        self.assertIsNone(ART.resolve("head/faces/${head}/sad/", item, [], data))

    def test_an_unknown_piece_is_refused_rather_than_skipped(self) -> None:
        with self.assertRaisesRegex(SystemExit, "no wardrobe item named"):
            ART.item_named(wardrobe(), "hair", "hair_of_flame")


class ReferenceCastTests(unittest.TestCase):
    def test_the_cast_covers_every_body_type_from_both_ends(self) -> None:
        cast = ART.reference_characters(wardrobe())

        self.assertEqual([person["name"] for person in cast], ["male-first", "male-last"])
        for person in cast:
            slots = [piece["slot"] for piece in person["pieces"]]
            self.assertEqual(slots, ["head", "hair", "expression", "shoes"], "wrong draw order")

    def test_a_piece_is_only_given_a_colour_it_actually_ships(self) -> None:
        data = wardrobe()
        shoes = data["slots"]["shoes"]["items"][0]

        for end in (0, -1):
            self.assertIn(ART.colour_for(data, shoes, end), shoes["options"])

    def test_a_piece_with_no_colour_is_given_none(self) -> None:
        data = wardrobe()
        head = data["slots"]["head"]["items"][0]

        self.assertEqual(ART.colour_for(data, head, 0), "")


class CopyTests(unittest.TestCase):
    def _lpc(self, tmp: Path, names: list[str]) -> Path:
        lpc = tmp / "lpc"
        for name in names:
            path = lpc / "spritesheets" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            Image.new("RGBA", ART.SHEET_SIZE, (0, 0, 0, 0)).save(path)
        return lpc

    def test_only_the_named_sheets_are_copied(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            named = sorted(ART.wanted_files(wardrobe()))
            lpc = self._lpc(tmp, [*named, "wizard/hat/walk.png"])
            out = tmp / "out"

            wanted, copied, removed = ART.copy_art(wardrobe(), lpc, out)

            self.assertEqual((wanted, copied, removed), (len(named), len(named), 0))
            self.assertFalse((out / "wizard/hat/walk.png").exists())

    def test_art_no_longer_named_is_pruned_rather_than_left_shipping(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            named = sorted(ART.wanted_files(wardrobe()))
            lpc = self._lpc(tmp, named)
            out = tmp / "out"
            ART.copy_art(wardrobe(), lpc, out)
            # A piece cut from the allowlist for a licensing reason would keep
            # shipping its art, credited to nobody, until somebody noticed.
            stale = out / "hat/wizard/walk.png"
            stale.parent.mkdir(parents=True, exist_ok=True)
            Image.new("RGBA", ART.SHEET_SIZE).save(stale)

            _, _, removed = ART.copy_art(wardrobe(), lpc, out)

            self.assertEqual(removed, 1)
            self.assertFalse(stale.exists())

    def test_a_sheet_the_wardrobe_names_but_the_checkout_lacks_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            lpc = self._lpc(tmp, ["head/male/walk.png"])

            with self.assertRaisesRegex(SystemExit, "missing from"):
                ART.copy_art(wardrobe(), lpc, tmp / "out")


class RenderTests(unittest.TestCase):
    def test_layers_stack_by_z_and_the_palette_is_applied(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            art = Path(raw)
            for name, colour in (
                ("head/male/walk.png", (9, 9, 9, 255)),
                ("head/faces/male/sad/walk.png", (0, 0, 0, 0)),
                ("hair/plain/walk.png", (0x26, 0x0D, 0x14, 255)),
                ("feet/shoes/walk/brown.png", (0, 0, 0, 0)),
            ):
                path = art / name
                path.parent.mkdir(parents=True, exist_ok=True)
                Image.new("RGBA", ART.SHEET_SIZE, colour).save(path)

            character = {
                "body": "male",
                "pieces": [
                    {"slot": "head", "item": "heads_human_male", "color": ""},
                    {"slot": "expression", "item": "face_sad", "color": ""},
                    {"slot": "hair", "item": "hair_plain", "color": "black"},
                    {"slot": "shoes", "item": "feet_shoes", "color": "brown"},
                ],
            }
            sheet = ART.render(wardrobe(), art, character)

            self.assertEqual(sheet.size, ART.SHEET_SIZE)
            # Hair sits at z 120, above the head at 100, and arrives recoloured.
            self.assertEqual(sheet.getpixel((0, 0)), (0x10, 0x10, 0x10, 255))

    def test_a_sheet_missing_from_the_runtime_tree_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            character = {
                "body": "male",
                "pieces": [{"slot": "head", "item": "heads_human_male", "color": ""}],
            }

            with self.assertRaisesRegex(SystemExit, "not in the runtime tree"):
                ART.render(wardrobe(), Path(raw), character)


class ExportShapeTests(unittest.TestCase):
    """The comparison against the LPC catalogue goes through the web app's own
    character shape, so it has to be built the way the app builds it."""

    def test_a_colour_lands_in_the_field_its_asset_generation_uses(self) -> None:
        data = wardrobe()
        character = {
            "body": "male",
            "pieces": [
                {"slot": "hair", "item": "hair_plain", "color": "black"},
                {"slot": "shoes", "item": "feet_shoes", "color": "brown"},
            ],
        }

        exported = ART.as_ulpc_character(data, character)["selections"]

        self.assertEqual(exported["hair"]["recolor"], "black")
        self.assertEqual(exported["hair"]["variant"], "")
        self.assertEqual(exported["hair"]["name"], "Plain (black)")
        self.assertEqual(exported["shoes"]["variant"], "brown")
        self.assertEqual(exported["shoes"]["recolor"], "")

    def test_a_colourless_piece_keeps_its_plain_name(self) -> None:
        data = wardrobe()
        character = {
            "body": "male",
            "pieces": [{"slot": "head", "item": "heads_human_male", "color": ""}],
        }

        exported = ART.as_ulpc_character(data, character)["selections"]

        self.assertEqual(exported["head"]["name"], "Human Male")


class BundledWardrobeTests(unittest.TestCase):
    """The committed wardrobe has to stay readable by this script; it is the
    only thing standing between a wardrobe edit and a traveller with no legs."""

    def test_every_item_records_somewhere_to_draw_it_from(self) -> None:
        data = json.loads(ART.WARDROBE.read_text())

        for name, slot in data["slots"].items():
            for item in slot["items"]:
                self.assertTrue(item["layers"], f"{item['id']} ({name}) draws nothing")
                for layer in item["layers"]:
                    self.assertIsInstance(layer["z"], int)
                    self.assertTrue(layer["paths"], f"{item['id']} has a layer with no paths")
                    for body, path in layer["paths"].items():
                        self.assertIn(body, item["bodies"])
                        self.assertTrue(path.endswith("/"), f"{path} is not a directory")

    def test_every_recoloured_piece_can_reach_every_colour_it_may_be_rolled(self) -> None:
        data = json.loads(ART.WARDROBE.read_text())
        reachable = {
            "skin": [entry["color"] for entry in data["palettes"]["skin"]],
            "hair": [entry["color"] for entry in data["palettes"]["hair"]]
            + [entry["color"] for entry in data["palettes"]["hair_old"]],
            "cloth": [entry["color"] for entry in data["palettes"]["cloth_bright"]],
        }

        for slot in data["slots"].values():
            for item in slot["items"]:
                recolor = item.get("recolor")
                if not recolor:
                    continue
                for colour in reachable.get(item["source"], []):
                    ramp = data["materials"].get(recolor["material"], {}).get(colour)
                    self.assertIsNotNone(
                        ramp, f"{item['id']} can be rolled {colour} and has no ramp for it"
                    )
                    self.assertEqual(
                        len(ramp),
                        len(recolor["from"]),
                        f"{item['id']}: the {colour} ramp is a different length "
                        "from the one it was drawn in",
                    )

    def test_every_templated_path_has_a_substitution_table(self) -> None:
        data = json.loads(ART.WARDROBE.read_text())

        for slot in data["slots"].values():
            for item in slot["items"]:
                for layer in item["layers"]:
                    for path in layer["paths"].values():
                        if "${" not in path:
                            continue
                        key = path[path.index("${") + 2 : path.index("}")]
                        self.assertIn(
                            key,
                            item.get("replace", {}),
                            f"{item['id']} templates ${{{key}}} with nothing to fill it",
                        )


if __name__ == "__main__":
    unittest.main()
