from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image


def _load(name: str, filename: str):
    script = Path(__file__).with_name(filename)
    spec = importlib.util.spec_from_file_location(name, script)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SPLIT = _load("waystation_lpc_art_split", "lpc-art-split.py")
CHECK = _load("waystation_check_lpc_art", "check-lpc-art.py")

RAMP = [(0x26, 0x0D, 0x14), (0x6A, 0x11, 0x08), (0xA4, 0x26, 0x00),
        (0xBF, 0x40, 0x00), (0xE5, 0x56, 0x00), (0xFF, 0x8A, 0x00)]


def sprite() -> Image.Image:
    """A miniature of the real thing: opaque ramp colours, a partial-alpha
    shadow, and transparency."""
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    for index, colour in enumerate(RAMP):
        image.putpixel((index, 0), (*colour, 255))
    image.putpixel((0, 1), (0, 0, 0, 64))
    image.putpixel((1, 1), (0, 0, 0, 64))
    return image


class SplitTests(unittest.TestCase):
    def test_opaque_and_partial_pixels_go_to_different_layers(self) -> None:
        opaque, partial = SPLIT.split_image(sprite())

        self.assertEqual(opaque.getpixel((0, 0)), (*RAMP[0], 255))
        self.assertEqual(opaque.getpixel((0, 1)), (0, 0, 0, 0), "shadow leaked into the hair layer")
        self.assertEqual(partial.getpixel((0, 1)), (0, 0, 0, 64))
        self.assertEqual(partial.getpixel((0, 0)), (0, 0, 0, 0), "hair leaked into the shadow layer")

    def test_the_layers_rejoin_exactly(self) -> None:
        original = sprite()
        opaque, partial = SPLIT.split_image(original)

        self.assertEqual(SPLIT.join_images(opaque, partial).tobytes(), original.tobytes())

    def test_a_sheet_with_no_shadow_still_round_trips(self) -> None:
        # `climb` has no cast shadow at all, and must not become a special case.
        plain = Image.new("RGBA", (8, 8), (0, 0, 0, 0))
        plain.putpixel((0, 0), (*RAMP[2], 255))
        opaque, partial = SPLIT.split_image(plain)

        self.assertEqual(SPLIT.join_images(opaque, partial).tobytes(), plain.tobytes())

    def test_split_writes_both_layers_and_join_restores_the_original(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, work, back = Path(tmp) / "src", Path(tmp) / "work", Path(tmp) / "back"
            source.mkdir()
            sprite().save(source / "walk.png")

            SPLIT.split(source, work)
            self.assertTrue((work / SPLIT.OPAQUE / "walk.png").is_file())
            self.assertTrue((work / SPLIT.PARTIAL / "walk.png").is_file())

            SPLIT.join(work, back)
            rebuilt = Image.open(back / "walk.png").convert("RGBA")

        self.assertEqual(rebuilt.tobytes(), sprite().tobytes())

    def test_a_missing_shadow_layer_is_treated_as_empty(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work, out = Path(tmp) / "work", Path(tmp) / "out"
            (work / SPLIT.OPAQUE).mkdir(parents=True)
            (work / SPLIT.PARTIAL).mkdir(parents=True)
            only = Image.new("RGBA", (8, 8), (0, 0, 0, 0))
            only.putpixel((3, 3), (*RAMP[1], 255))
            only.save(work / SPLIT.OPAQUE / "climb.png")

            SPLIT.join(work, out)
            rebuilt = Image.open(out / "climb.png").convert("RGBA")

        self.assertEqual(rebuilt.tobytes(), only.tobytes())

    def test_mismatched_layer_sizes_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work, out = Path(tmp) / "work", Path(tmp) / "out"
            (work / SPLIT.OPAQUE).mkdir(parents=True)
            (work / SPLIT.PARTIAL).mkdir(parents=True)
            Image.new("RGBA", (8, 8)).save(work / SPLIT.OPAQUE / "walk.png")
            Image.new("RGBA", (16, 8)).save(work / SPLIT.PARTIAL / "walk.png")

            with self.assertRaisesRegex(SystemExit, "different sizes"):
                SPLIT.join(work, out)


class RampTests(unittest.TestCase):
    def _lpc(self, tmp: str) -> Path:
        lpc = Path(tmp)
        hair = lpc / "palette_definitions/hair"
        hair.mkdir(parents=True)
        (hair / "meta_hair.json").write_text(json.dumps({"default": "ulpc", "base": "orange"}))
        (hair / "hair_ulpc.json").write_text(
            json.dumps({"orange": [f"#{r:02x}{g:02x}{b:02x}" for r, g, b in RAMP], "black": ["#000000"] * 6})
        )
        return lpc

    def test_the_ramp_comes_from_the_materials_own_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(CHECK.ramp_for(self._lpc(tmp), "hair"), RAMP)

    def test_an_unknown_material_is_an_error_not_an_empty_ramp(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(SystemExit, "unknown material"):
                CHECK.ramp_for(self._lpc(tmp), "chainmail")


class CheckTests(unittest.TestCase):
    def _check(self, image: Image.Image) -> list[str]:
        complaints: list[str] = []
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "walk.png"
            image.save(path)
            CHECK.check_sheet(path, set(RAMP), complaints)
        return complaints

    def test_art_drawn_on_the_ramp_passes(self) -> None:
        self.assertEqual(self._check(sprite()), [])

    def test_an_off_ramp_opaque_pixel_is_caught(self) -> None:
        image = sprite()
        image.putpixel((5, 5), (0xC0, 0x50, 0x10, 255))

        complaints = self._check(image)

        self.assertEqual(len(complaints), 1)
        self.assertIn("#c05010", complaints[0])
        self.assertIn("will not recolour", complaints[0])

    def test_an_anti_aliased_edge_is_caught_by_its_alpha(self) -> None:
        image = sprite()
        image.putpixel((6, 6), (*RAMP[0], 200))

        complaints = self._check(image)

        self.assertEqual(len(complaints), 1)
        self.assertIn("alpha 200", complaints[0])

    def test_the_cast_shadow_is_allowed_to_sit_off_the_ramp(self) -> None:
        # Black at alpha 64 is off-ramp on purpose, so that a shadow stays a
        # shadow rather than turning orange on a redhead.
        self.assertEqual(self._check(sprite()), [])

    def test_frames_that_are_not_whole_are_caught(self) -> None:
        odd = Image.new("RGBA", (63, 64), (0, 0, 0, 0))

        self.assertTrue(any("whole number of 64px frames" in c for c in self._check(odd)))


class ReferenceComparisonTests(unittest.TestCase):
    def _dirs(self, tmp: str) -> tuple[Path, Path]:
        art, reference = Path(tmp) / "art", Path(tmp) / "reference"
        art.mkdir(); reference.mkdir()
        return art, reference

    def test_a_missing_animation_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            art, reference = self._dirs(tmp)
            Image.new("RGBA", (64, 64)).save(reference / "walk.png")
            Image.new("RGBA", (64, 64)).save(reference / "sit.png")
            Image.new("RGBA", (64, 64)).save(art / "walk.png")
            complaints: list[str] = []
            CHECK.compare_to_reference(art, reference, complaints)

        self.assertTrue(any("missing sit.png" in c for c in complaints))

    def test_a_resized_sheet_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            art, reference = self._dirs(tmp)
            Image.new("RGBA", (576, 256)).save(reference / "walk.png")
            Image.new("RGBA", (512, 256)).save(art / "walk.png")
            complaints: list[str] = []
            CHECK.compare_to_reference(art, reference, complaints)

        self.assertTrue(any("walk.png" in c and "(512, 256)" in c for c in complaints))

    def test_a_misnamed_sheet_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            art, reference = self._dirs(tmp)
            Image.new("RGBA", (64, 64)).save(reference / "walk.png")
            Image.new("RGBA", (64, 64)).save(art / "walk.png")
            Image.new("RGBA", (64, 64)).save(art / "wlak.png")
            complaints: list[str] = []
            CHECK.compare_to_reference(art, reference, complaints)

        self.assertTrue(any("wlak.png" in c for c in complaints))


if __name__ == "__main__":
    unittest.main()
