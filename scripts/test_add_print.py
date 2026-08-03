from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("add-print.py")
SPEC = importlib.util.spec_from_file_location("waystation_add_print", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
ADD_PRINT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADD_PRINT)


class AddPrintTests(unittest.TestCase):
    def entry(self) -> dict[str, str]:
        return ADD_PRINT.make_entry(
            print_id="early-welcome",
            title="A Place at the Table",
            theme="hospitality",
            reference="Romans 12:13",
            verse="Distributing to the necessity of saints; given to hospitality.",
            art_prompt="An ordinary host makes room at a rough table for a traveler.",
        )

    def test_entry_derives_consistent_asset_paths(self) -> None:
        entry = self.entry()

        self.assertEqual(entry["art"], "assets/prints/early-welcome-art.png")
        self.assertEqual(entry["card"], "assets/prints/early-welcome-card.png")
        self.assertEqual(entry["stage"], "early_monochrome")

    def test_entry_carries_the_id_the_server_asks_a_translation_for(self) -> None:
        self.assertEqual(self.entry()["passage_id"], "ROM.12.13")

    def test_a_card_cannot_be_added_citing_half_a_verse(self) -> None:
        # The excerpt is the `verse` field now; a part marker in the reference
        # would be a second, disagreeing account of the same cut.
        with self.assertRaisesRegex(ValueError, "not half of it"):
            ADD_PRINT.make_entry(
                print_id="early-welcome",
                title="A Place at the Table",
                theme="hospitality",
                reference="Romans 12:13a",
                verse="given to hospitality",
                art_prompt="An ordinary host makes room at a rough table.",
            )

    def test_append_rejects_duplicate_identity_or_paths(self) -> None:
        entry = self.entry()
        manifest: dict[str, object] = {"prints": [entry.copy()]}

        with self.assertRaisesRegex(ValueError, "duplicate id"):
            ADD_PRINT.append_entry(manifest, entry.copy())

    def test_atomic_writer_preserves_valid_json(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "prints.json"
            manifest: dict[str, object] = {"schema_version": 1, "prints": []}
            ADD_PRINT.append_entry(manifest, self.entry())

            ADD_PRINT.write_json_atomic(path, manifest)

            written = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(written, manifest)

    def test_slug_rejects_spaces_and_path_characters(self) -> None:
        for invalid in ("Early Welcome", "../welcome", "welcome/card"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(Exception):
                    ADD_PRINT.slug(invalid)


if __name__ == "__main__":
    unittest.main()
