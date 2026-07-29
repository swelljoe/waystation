from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

from asset_catalog import catalog_assets, path_words, should_catalog_image, suggested_grid


class AssetCatalogTests(unittest.TestCase):
    def test_path_words_add_search_synonyms(self) -> None:
        words = path_words("rooms/Old_Bed-01.png")
        self.assertTrue({"bed", "bedroom", "sleep", "furniture"} <= words)

    def test_motel_sheets_default_to_48_pixel_grid(self) -> None:
        self.assertEqual(suggested_grid("motel/tile-B-03.png", 768, 768), 48)

    def test_component_catalog_keeps_sources_and_ignores_engine_imports(self) -> None:
        self.assertTrue(should_catalog_image("components/damaged-house.png"))
        self.assertTrue(
            should_catalog_image("components/Village House/Texture/TX Village House.png")
        )
        self.assertFalse(
            should_catalog_image("components/Village House/UnityCache/hash/preview.png")
        )

    def test_catalog_applies_sidecar_tags(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets = root / "assets"
            sheet = assets / "motel" / "tile-B-03.png"
            sheet.parent.mkdir(parents=True)
            Image.new("RGBA", (96, 96)).save(sheet)
            rules = root / "rules.json"
            rules.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "rules": [
                            {"patterns": ["motel/tile-B-03.png"], "tags": ["bed", "desk"]}
                        ],
                    }
                ),
                encoding="utf-8",
            )
            catalog = catalog_assets(assets, rules)
            self.assertEqual(catalog["count"], 1)
            self.assertTrue({"bed", "desk"} <= set(catalog["assets"][0]["tags"]))


if __name__ == "__main__":
    unittest.main()
