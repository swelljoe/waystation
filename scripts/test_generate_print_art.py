from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image


SCRIPT = Path(__file__).with_name("generate-print-art.py")
SPEC = importlib.util.spec_from_file_location("waystation_generate_print_art", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GENERATE_ART = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GENERATE_ART)


class PrintArtBatchTests(unittest.TestCase):
    def test_every_catalog_entry_has_a_subject_prompt(self) -> None:
        manifest_path = Path(__file__).parents[1] / "content" / "prints.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

        self.assertTrue(manifest["image_generation"]["common_prompt"])
        self.assertTrue(manifest["image_generation"]["reference"])
        self.assertTrue(all(entry["art_prompt"] for entry in manifest["prints"]))

    def test_codex_prompt_invokes_imagegen_and_limits_writes(self) -> None:
        entry = {
            "art": "assets/prints/example-art.png",
            "art_prompt": "A traveler shares a loaf.",
        }

        prompt = GENERATE_ART.build_prompt("Rough black block print.", entry)

        self.assertTrue(prompt.startswith("$imagegen"))
        self.assertIn(entry["art"], prompt)
        self.assertIn(entry["art_prompt"], prompt)
        self.assertIn("Do not modify any other project file", prompt)
        self.assertIn("absolutely no\ntext", prompt)

    def test_force_is_explicit_in_the_agent_prompt(self) -> None:
        entry = {"art": "assets/prints/example-art.png", "art_prompt": "A lamp."}

        prompt = GENERATE_ART.build_prompt("Block print.", entry, replace_existing=True)

        self.assertIn("explicitly used --force", prompt)

    def test_image_verifier_accepts_only_sufficiently_large_portraits(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            portrait = Path(temporary) / "portrait.png"
            landscape = Path(temporary) / "landscape.png"
            Image.new("RGB", (512, 768), "white").save(portrait)
            Image.new("RGB", (768, 512), "white").save(landscape)

            GENERATE_ART.verify_art(portrait)
            with self.assertRaisesRegex(RuntimeError, "not portrait-oriented"):
                GENERATE_ART.verify_art(landscape)

    def test_codex_command_is_ephemeral_and_workspace_scoped(self) -> None:
        command = GENERATE_ART.codex_command("codex", Path("reference.png"))

        self.assertIn("--ephemeral", command)
        self.assertIn("workspace-write", command)
        self.assertNotIn("danger-full-access", command)
        self.assertEqual(command[-1], "-")


if __name__ == "__main__":
    unittest.main()
