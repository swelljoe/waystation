from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

from PIL import Image


SCRIPT = Path(__file__).with_name("preview-npcs.py")
SPEC = importlib.util.spec_from_file_location("waystation_preview_npcs", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
PREVIEW = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREVIEW)


class NameTests(unittest.TestCase):
    def test_the_colour_is_stripped_from_an_exported_name(self) -> None:
        selection = {"name": "Human Male Elderly (amber)"}

        self.assertEqual(PREVIEW.name_without_color(selection), "Human Male Elderly")

    def test_a_name_with_no_colour_survives_whole(self) -> None:
        self.assertEqual(PREVIEW.name_without_color({"name": "Wrinkles"}), "Wrinkles")


class SubstituteTests(unittest.TestCase):
    """Faces live in a directory named for the head above them."""

    DEFINITION = {
        "replace_in_path": {
            "head": {"Human_Male_Elderly": "elderly", "Human_Female": "female"},
        }
    }

    def test_the_head_names_the_directory(self) -> None:
        selections = {"head": {"name": "Human Male Elderly (amber)"}}

        resolved = PREVIEW.substitute("head/faces/${head}/sad/", selections, self.DEFINITION)

        self.assertEqual(resolved, "head/faces/elderly/sad/")

    def test_a_head_with_no_faces_drawn_for_it_resolves_to_nothing(self) -> None:
        selections = {"head": {"name": "Human Child (taupe)"}}

        self.assertIsNone(
            PREVIEW.substitute("head/faces/${head}/sad/", selections, self.DEFINITION)
        )

    def test_a_missing_selection_resolves_to_nothing(self) -> None:
        self.assertIsNone(PREVIEW.substitute("head/faces/${head}/sad/", {}, self.DEFINITION))

    def test_a_plain_path_is_left_alone(self) -> None:
        self.assertEqual(PREVIEW.substitute("legs/pants/male/", {}, self.DEFINITION), "legs/pants/male/")


class RecolorTests(unittest.TestCase):
    def test_palette_colours_are_swapped_and_others_left_alone(self) -> None:
        image = Image.new("RGBA", (3, 1))
        image.putpixel((0, 0), (0x27, 0x19, 0x20, 255))  # in the source palette
        image.putpixel((1, 0), (0x11, 0x22, 0x33, 255))  # not in it
        image.putpixel((2, 0), (0, 0, 0, 0))  # transparent

        out = PREVIEW.recolor(image, [(["#271920"], ["#281716"])])

        self.assertEqual(out.getpixel((0, 0)), (0x28, 0x17, 0x16, 255))
        self.assertEqual(out.getpixel((1, 0)), (0x11, 0x22, 0x33, 255))
        self.assertEqual(out.getpixel((2, 0)), (0, 0, 0, 0))

    def test_alpha_is_kept(self) -> None:
        image = Image.new("RGBA", (1, 1), (0x27, 0x19, 0x20, 128))

        out = PREVIEW.recolor(image, [(["#271920"], ["#FFFFFF"])])

        self.assertEqual(out.getpixel((0, 0)), (0xFF, 0xFF, 0xFF, 128))

    def test_no_mapping_leaves_the_image_untouched(self) -> None:
        image = Image.new("RGBA", (1, 1), (1, 2, 3, 255))

        self.assertEqual(PREVIEW.recolor(image, []).getpixel((0, 0)), (1, 2, 3, 255))


class ContactSheetTests(unittest.TestCase):
    def test_the_grid_has_a_row_for_every_leftover(self) -> None:
        people = [(str(n), Image.new("RGBA", (PREVIEW.FRAME, PREVIEW.FRAME))) for n in range(7)]

        sheet = PREVIEW.contact_sheet(people, columns=3, scale=1)

        self.assertEqual(sheet.width, 3 * PREVIEW.FRAME)
        self.assertEqual(sheet.height, 3 * (PREVIEW.FRAME + 12))


if __name__ == "__main__":
    unittest.main()
