from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image, ImageDraw, ImageFont


SCRIPT = Path(__file__).with_name("build-print-cards.py")
SPEC = importlib.util.spec_from_file_location("waystation_build_print_cards", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
BUILD_PRINTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD_PRINTS)


class PrintCardTests(unittest.TestCase):
    def test_catalog_has_unique_ids_and_outputs(self) -> None:
        manifest_path = Path(__file__).parents[1] / "content" / "prints.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        entries = manifest["prints"]

        self.assertEqual(len({entry["id"] for entry in entries}), len(entries))
        self.assertEqual(len({entry["card"] for entry in entries}), len(entries))
        self.assertTrue(all(entry["verse"] for entry in entries))
        self.assertTrue(all(entry["reference"] for entry in entries))

    def test_word_wrap_respects_the_requested_width(self) -> None:
        font = ImageFont.truetype(str(BUILD_PRINTS.DEFAULT_FONT), 17)
        draw = ImageDraw.Draw(Image.new("RGB", (512, 768)))
        width = 190

        lines = BUILD_PRINTS.wrap_words(
            draw,
            "Bear ye one another's burdens, and so fulfil the law of Christ.",
            font,
            width,
        )

        self.assertGreater(len(lines), 1)
        for line in lines:
            self.assertLessEqual(draw.textbbox((0, 0), line, font=font)[2], width)

    def test_renderer_keeps_the_low_resolution_working_canvas(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "assets" / "prints" / "test-art.png"
            source.parent.mkdir(parents=True)
            Image.new("RGB", (1024, 1536), "#f4ead2").save(source)
            entry = {
                "verse": "And the light shineth in darkness.",
                "reference": "John 1:5a",
                "art": "assets/prints/test-art.png",
                "card": "assets/prints/test-card.png",
            }

            with mock.patch.object(BUILD_PRINTS, "ROOT", root):
                BUILD_PRINTS.render_card(entry, BUILD_PRINTS.DEFAULT_FONT)

            with Image.open(root / entry["card"]) as card:
                self.assertEqual(card.size, BUILD_PRINTS.OUTPUT_SIZE)
                self.assertEqual(card.size, BUILD_PRINTS.WORK_SIZE)


if __name__ == "__main__":
    unittest.main()
