#!/usr/bin/env python3
"""Checks for the headless web smoke runner that do not need a browser."""

import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import web_smoke  # noqa: E402


class ExpectedNoise(unittest.TestCase):
    """The filter is the whole reason the runner can fail on console output.

    Too loose and a real exception rides through as noise; too tight and every
    run fails on complaints that are normal for a Bevy build in a browser.
    """

    def test_normal_browser_and_bevy_complaints_are_ignored(self):
        for message in [
            "Failed to load resource: 404 http://host/runtime-assets/world/tree.png.meta",
            "Failed to load resource: 404 http://host/favicon.ico",
            "The AudioContext was not allowed to start. It must be resumed",
            "The `integrity` attribute is currently ignored for preload destinations",
        ]:
            self.assertTrue(web_smoke.is_expected(message), message)

    def test_real_faults_are_never_ignored(self):
        for message in [
            "Failed to load resource: 404 http://host/runtime-assets/world/garden_plot_ripe.png",
            "RuntimeError: unreachable executed",
            "WebGL: CONTEXT_LOST_WEBGL",
            "panicked at crates/game/src/main.rs",
        ]:
            self.assertFalse(web_smoke.is_expected(message), message)


class Arguments(unittest.TestCase):
    def test_a_missing_bundle_is_reported_rather_than_launching_a_browser(self):
        result = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).resolve().parent / "web_smoke.py"),
                "--dist",
                "/nonexistent-bundle",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("make web", result.stderr)

    def test_every_key_the_walk_syntax_accepts_maps_to_a_real_code(self):
        for name, (code, key_code) in web_smoke.KEYS.items():
            self.assertTrue(code)
            self.assertGreater(key_code, 0, name)


class Interpretation(unittest.TestCase):
    """The stub stands in for Gloo and YouVersion.

    Without it a completed visit ends in a 501 the runner reports as a fault,
    which is both wrong and loud enough to hide a real one. Its shape has to
    match `waystation_shared::InterpretResponse` or the game silently falls back
    to its own fixture and the stub proves nothing.
    """

    def test_the_stub_carries_every_field_the_game_deserializes(self):
        reply = web_smoke.STUB_INTERPRETATION
        for field in ("vignette_id", "need_id", "need_label", "reflection"):
            self.assertIn(field, reply)
        for field in ("id", "reference", "content", "version", "youversion_deep_link"):
            self.assertIn(field, reply["passage"])
        for field in ("gloo_model", "routing", "scripture_source"):
            self.assertIn(field, reply["provenance"])

    def test_the_stub_says_plainly_that_it_is_not_a_live_answer(self):
        provenance = web_smoke.STUB_INTERPRETATION["provenance"]
        self.assertIn("smoke", provenance["gloo_model"])
        self.assertEqual(provenance["scripture_source"], "fixture")


if __name__ == "__main__":
    unittest.main()
