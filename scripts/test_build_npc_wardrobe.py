from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("build-npc-wardrobe.py")
SPEC = importlib.util.spec_from_file_location("waystation_build_npc_wardrobe", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
WARDROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WARDROBE)

BUILT = Path(__file__).parents[1] / "crates/npcgen/data/wardrobe.json"


class CuratedListTests(unittest.TestCase):
    """The allowlist is the whole point; these guard its shape, not its taste."""

    def test_every_slot_has_something_in_it(self) -> None:
        for name, spec in WARDROBE.SLOTS.items():
            self.assertTrue(spec["items"], f"{name} is empty")
            self.assertGreater(spec["chance"], 0.0, name)
            self.assertLessEqual(spec["chance"], 1.0, name)

    def test_no_item_is_listed_twice(self) -> None:
        seen: set[str] = set()
        for name, spec in WARDROBE.SLOTS.items():
            for item_id, *_ in spec["items"]:
                self.assertNotIn(item_id, seen, f"{item_id} appears twice ({name})")
                seen.add(item_id)

    def test_every_cloth_colour_is_one_the_palettes_know(self) -> None:
        for table in (WARDROBE.CLOTH_MUTED, WARDROBE.CLOTH_BRIGHT):
            for colour, weight in table:
                self.assertIn(colour, WARDROBE.KNOWN_COLOURS, colour)
                self.assertGreater(weight, 0, colour)

    def test_the_dyed_era_only_adds_to_the_scavenged_one(self) -> None:
        muted = {colour for colour, _ in WARDROBE.CLOTH_MUTED}
        bright = {colour for colour, _ in WARDROBE.CLOTH_BRIGHT}
        self.assertTrue(muted <= bright, "a settlement that can dye still owns brown")

    def test_grey_hair_is_kept_for_old_heads(self) -> None:
        young = {colour for colour, _ in WARDROBE.HAIR}
        for old in ("gray", "white", "platinum"):
            self.assertNotIn(old, young, f"{old} hair on the young reads as a costume")
            self.assertIn(old, {colour for colour, _ in WARDROBE.HAIR_OLD}, old)

    def test_the_purple_and_neon_hair_colours_stay_out(self) -> None:
        # ULPC's `ash` runs plum to cream and `ginger` tops out at neon yellow.
        for trap in ("ash", "ginger"):
            self.assertNotIn(trap, {colour for colour, _ in WARDROBE.HAIR})
            self.assertNotIn(trap, {colour for colour, _ in WARDROBE.HAIR_OLD})


class PathExpansionTests(unittest.TestCase):
    def test_a_plain_path_is_left_alone(self) -> None:
        self.assertEqual(WARDROBE.expand_placeholders("body/bodies/male/", {}), ["body/bodies/male/"])

    def test_a_templated_path_becomes_one_path_per_substitution(self) -> None:
        definition = {
            "replace_in_path": {
                "head": {"Human_Male": "male", "Human_Male_Small": "male", "Human_Female": "female"}
            }
        }
        expanded = WARDROBE.expand_placeholders("head/faces/${head}/sad/", definition)

        self.assertEqual(
            sorted(expanded), ["head/faces/female/sad/", "head/faces/male/sad/"]
        )

    def test_a_placeholder_with_no_mapping_resolves_to_nothing(self) -> None:
        self.assertEqual(WARDROBE.expand_placeholders("head/faces/${head}/sad/", {}), [])


class ColourFieldTests(unittest.TestCase):
    def test_baked_colours_land_in_the_variant_field(self) -> None:
        field, variants = WARDROBE.colour_field({"variants": ["brown", "tan"]})

        self.assertEqual(field, "variant")
        self.assertEqual(variants, ["brown", "tan"])

    def test_palette_colours_land_in_the_recolor_field(self) -> None:
        field, variants = WARDROBE.colour_field({"recolors": {"material": "cloth"}})

        self.assertEqual(field, "recolor")
        self.assertEqual(variants, [])

    def test_a_piece_with_one_appearance_has_neither(self) -> None:
        self.assertEqual(WARDROBE.colour_field({}), ("none", []))


class StrayColourTests(unittest.TestCase):
    """A second colour slot nobody picks renders in whatever the artist drew."""

    def test_a_hard_coded_hair_tie_is_refused(self) -> None:
        definition = {
            "recolors": {
                "color_1": {"material": "hair", "palettes": ["ulpc"]},
                "color_2": {
                    "type_name": "hair_tie",
                    "material": "cloth",
                    "source": ["#8c288b", "#b75ea5"],
                },
            }
        }
        with self.assertRaisesRegex(SystemExit, "hair_tie"):
            WARDROBE.check_stray_colours("hair_long_tied", definition)

    def test_default_eye_colour_is_allowed(self) -> None:
        definition = {
            "recolors": {
                "color_1": {"material": "body", "palettes": ["ulpc"]},
                "color_2": {"type_name": "eyes", "material": "eye", "source": ["#4b6cc1"]},
            }
        }
        WARDROBE.check_stray_colours("face_sad", definition)

    def test_a_single_slot_piece_is_allowed(self) -> None:
        WARDROBE.check_stray_colours("torso_clothes_longsleeve", {"recolors": {"material": "cloth"}})


class SupportedBodiesTests(unittest.TestCase):
    """`bodies` has to mean "draws", not "the definition mentions it"."""

    def _definition(self) -> dict:
        return {"layer_1": {"zPos": 35, "male": "shirt/male/", "female": "shirt/female/"}}

    def test_a_body_type_with_no_sprite_on_disk_is_dropped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lpc = Path(tmp)
            (lpc / "spritesheets/shirt/male").mkdir(parents=True)
            (lpc / "spritesheets/shirt/male/walk.png").write_bytes(b"")

            bodies = WARDROBE.supported_bodies(lpc, self._definition(), "shirt", "recolor", [])

        self.assertEqual(bodies, ["male"])

    def test_a_variant_piece_needs_every_colour_it_offers(self) -> None:
        definition = {"layer_1": {"zPos": 65, "male": "sash/male/"}}
        with tempfile.TemporaryDirectory() as tmp:
            lpc = Path(tmp)
            (lpc / "spritesheets/sash/male/walk").mkdir(parents=True)
            (lpc / "spritesheets/sash/male/walk/brown.png").write_bytes(b"")

            complete = WARDROBE.supported_bodies(lpc, definition, "sash", "variant", ["brown"])
            partial = WARDROBE.supported_bodies(lpc, definition, "sash", "variant", ["brown", "tan"])

        self.assertEqual(complete, ["male"])
        self.assertEqual(partial, [])

    def test_layout_only_keys_are_not_body_types(self) -> None:
        declared = WARDROBE.declared_bodies(
            {"layer_1": {"zPos": 100, "male": "a/", "is_mask": "b/", "custom_animation": "c/"}}
        )

        self.assertEqual(declared, {"male"})


class LicenceTests(unittest.TestCase):
    """Waystation is not open source, so GPL is not a licence it can pick."""

    def test_licence_strings_are_matched_by_family(self) -> None:
        # The catalogue spells these inconsistently — trailing `+`, a hyphenated
        # `OGA-BY-3.0` — so literal comparison would silently miss offers.
        self.assertEqual(WARDROBE.licence_family("OGA-BY 3.0"), "OGA-BY")
        self.assertEqual(WARDROBE.licence_family("OGA-BY-3.0"), "OGA-BY")
        self.assertEqual(WARDROBE.licence_family("CC-BY 3.0+"), "CC-BY")
        self.assertEqual(WARDROBE.licence_family("CC-BY-SA 4.0"), "CC-BY-SA")
        self.assertEqual(WARDROBE.licence_family("GPL 2.0"), "GPL")
        self.assertEqual(WARDROBE.licence_family("CC0"), "CC0")

    def test_cc_by_sa_is_not_mistaken_for_cc_by(self) -> None:
        self.assertNotEqual(WARDROBE.licence_family("CC-BY-SA 3.0"), "CC-BY")

    def test_something_unheard_of_is_never_silently_allowed(self) -> None:
        family = WARDROBE.licence_family("WTFPL")

        self.assertTrue(family.startswith("unrecognised:"))
        for allowed in WARDROBE.LICENCE_POLICIES.values():
            self.assertNotIn(family, allowed)

    def test_gpl_is_in_no_policy(self) -> None:
        for name, allowed in WARDROBE.LICENCE_POLICIES.items():
            self.assertNotIn("GPL", allowed, name)

    def test_the_attribution_policy_excludes_share_alike(self) -> None:
        self.assertNotIn("CC-BY-SA", WARDROBE.LICENCE_POLICIES["attribution"])
        self.assertIn("CC-BY-SA", WARDROBE.LICENCE_POLICIES["permissive"])

    def _definition(self, *licences: list[str]) -> dict:
        return {
            "credits": [
                {"file": f"art/part{n}", "licenses": lic, "authors": ["A"]}
                for n, lic in enumerate(licences)
            ]
        }

    def test_triple_licensed_art_is_accepted_and_gpl_is_not_recorded(self) -> None:
        definition = self._definition(["OGA-BY 3.0", "CC-BY-SA 3.0", "GPL 3.0"])

        families, refused = WARDROBE.check_licences(
            "shirt", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["permissive"]
        )

        self.assertEqual(families, {"OGA-BY", "CC-BY-SA"})
        self.assertEqual(refused, [])

    def test_gpl_only_art_is_refused(self) -> None:
        definition = self._definition(["GPL 2.0", "GPL 3.0"])

        _, refused = WARDROBE.check_licences(
            "shirt", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["permissive"]
        )

        self.assertEqual(len(refused), 1)
        self.assertIn("GPL", refused[0])

    def test_one_gpl_only_layer_is_caught_even_when_another_is_clean(self) -> None:
        # This is exactly what the web app's own filter misses: it keeps an item
        # when any single credit entry passes.
        definition = self._definition(["OGA-BY 3.0"], ["GPL 3.0"])

        _, refused = WARDROBE.check_licences(
            "shirt",
            definition,
            ["art/part0", "art/part1"],
            WARDROBE.LICENCE_POLICIES["permissive"],
        )

        self.assertEqual(len(refused), 1)
        self.assertIn("art/part1", refused[0])

    def test_uncredited_art_is_refused_because_it_cannot_be_attributed(self) -> None:
        _, refused = WARDROBE.check_licences(
            "shirt", self._definition(["CC0"]), ["art/elsewhere"], {"CC0"}
        )

        self.assertEqual(len(refused), 1)
        self.assertIn("no credit entry", refused[0])

    def test_art_with_an_unnamed_author_is_refused(self) -> None:
        definition = {
            "credits": [
                {
                    "file": "art/part0",
                    "licenses": ["CC-BY-SA 3.0"],
                    "authors": ["Stephen Challener (Redshrike)", "??"],
                }
            ]
        }

        _, refused = WARDROBE.check_licences(
            "plump", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["permissive"]
        )

        self.assertEqual(len(refused), 1)
        self.assertIn("cannot name", refused[0])

    def test_cc0_art_needs_no_author(self) -> None:
        # CC0 waives the attribution requirement, so an unnamed author is fine.
        definition = {"credits": [{"file": "art/part0", "licenses": ["CC0"], "authors": ["??"]}]}

        _, refused = WARDROBE.check_licences(
            "thing", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["permissive"]
        )

        self.assertEqual(refused, [])

    def test_share_alike_passes_permissive_but_not_attribution(self) -> None:
        definition = self._definition(["CC-BY-SA 3.0", "GPL 3.0"])

        _, permissive = WARDROBE.check_licences(
            "hair", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["permissive"]
        )
        _, attribution = WARDROBE.check_licences(
            "hair", definition, ["art/part0"], WARDROBE.LICENCE_POLICIES["attribution"]
        )

        self.assertEqual(permissive, [])
        self.assertEqual(len(attribution), 1)


class HandmadeSheetTests(unittest.TestCase):
    """The Scribe and the named visitors were drawn by hand, and ship today."""

    DEFINITION = {
        "layer_1": {"zPos": 101, "male": "head/faces/${head}/sad/", "female": "head/faces/x/"},
        "replace_in_path": {"head": {"Human_Male_Small": "male", "Human_Female": "female"}},
    }

    def test_a_placeholder_resolves_from_the_characters_own_head(self) -> None:
        selections = {"head": {"name": "Human Male Small (light)"}}

        paths = WARDROBE.pinned_paths(self.DEFINITION, "male", selections)

        self.assertEqual(paths, ["head/faces/male/sad"])

    def test_one_character_does_not_pick_up_every_heads_art(self) -> None:
        # The wardrobe expands `${head}` to all substitutions because the
        # generator pairs any head with any expression; a fixed character has
        # exactly one, and crediting the others would be wrong.
        selections = {"head": {"name": "Human Male Small (light)"}}

        pinned = WARDROBE.pinned_paths(self.DEFINITION, "male", selections)
        expanded = WARDROBE.expand_placeholders(
            self.DEFINITION["layer_1"]["male"], self.DEFINITION
        )

        self.assertEqual(len(pinned), 1)
        self.assertEqual(len(expanded), 2)

    def test_a_head_with_no_art_for_this_expression_resolves_to_nothing(self) -> None:
        selections = {"head": {"name": "Human Child (taupe)"}}

        self.assertEqual(WARDROBE.pinned_paths(self.DEFINITION, "male", selections), [])

    def test_a_layer_this_base_does_not_draw_is_skipped(self) -> None:
        definition = {"layer_1": {"zPos": 35, "male": "shirt/male/"}}

        self.assertEqual(WARDROBE.pinned_paths(definition, "child", {}), [])

    def test_every_shipped_action_sheet_has_a_provenance_record(self) -> None:
        from PIL import Image

        for art in sorted(WARDROBE.HANDMADE_DIR.glob("*.png")):
            with Image.open(art) as image:
                if image.size != WARDROBE.ACTION_SHEET_SIZE:
                    continue
            self.assertTrue(
                art.with_suffix(".txt").is_file(),
                f"{art.name} is a character sheet with no LPC export beside it",
            )


class OverlayTests(unittest.TestCase):
    """Project-drawn pieces load beside upstream ones and are checked the same."""

    def _roots(self, tmp: str) -> tuple[Path, Path]:
        lpc, overlay = Path(tmp) / "lpc", Path(tmp) / "overlay"
        (lpc / "sheet_definitions/hair").mkdir(parents=True)
        (lpc / "sheet_definitions/hair/hair_plain.json").write_text(
            json.dumps({"name": "Plain", "type_name": "hair"})
        )
        (overlay / "sheet_definitions/hair").mkdir(parents=True)
        return lpc, overlay

    def test_overlay_pieces_load_with_their_own_sprite_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lpc, overlay = self._roots(tmp)
            (overlay / "sheet_definitions/hair/hair_mine.json").write_text(
                json.dumps({"name": "Mine", "type_name": "hair"})
            )
            found = WARDROBE.load_definitions(lpc, overlay)

        self.assertEqual(found["hair_plain"][1], lpc)
        self.assertEqual(found["hair_plain"][2], "lpc")
        self.assertEqual(found["hair_mine"][1], overlay)
        self.assertEqual(found["hair_mine"][2], "overlay")

    def test_an_overlay_piece_replaces_the_upstream_one_of_the_same_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lpc, overlay = self._roots(tmp)
            (overlay / "sheet_definitions/hair/hair_plain.json").write_text(
                json.dumps({"name": "Plain Redrawn", "type_name": "hair"})
            )
            found = WARDROBE.load_definitions(lpc, overlay)

        self.assertEqual(found["hair_plain"][0]["name"], "Plain Redrawn")
        self.assertEqual(found["hair_plain"][2], "overlay")

    def test_a_missing_overlay_directory_is_not_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lpc, _ = self._roots(tmp)
            found = WARDROBE.load_definitions(lpc, Path(tmp) / "nothing-here")

        self.assertIn("hair_plain", found)

    def test_overlay_art_is_licence_checked_like_anything_else(self) -> None:
        definition = {
            "credits": [{"file": "hair/mine", "licenses": ["GPL 3.0"], "authors": ["Me"]}]
        }
        _, refused = WARDROBE.check_licences(
            "hair_mine", definition, ["hair/mine"], WARDROBE.LICENCE_POLICIES["permissive"]
        )

        self.assertEqual(len(refused), 1)


class BuiltWardrobeTests(unittest.TestCase):
    """The checked-in file the game compiles in, as it currently stands."""

    def setUp(self) -> None:
        self.wardrobe = json.loads(BUILT.read_text(encoding="utf-8"))

    def test_every_piece_draws_for_at_least_one_body_type(self) -> None:
        for name, slot in self.wardrobe["slots"].items():
            for item in slot["items"]:
                self.assertTrue(item["bodies"], f"{item['id']} ({name}) draws for nothing")

    def test_the_required_slots_cover_every_body_type(self) -> None:
        for base in self.wardrobe["body_types"]:
            for name in ("body", "head", "clothes", "legs"):
                items = self.wardrobe["slots"][name]["items"]
                self.assertTrue(
                    any(base["id"] in item["bodies"] for item in items),
                    f"no {name} draws for the {base['id']} base",
                )

    def test_a_pieces_slot_matches_its_lpc_type(self) -> None:
        for name, slot in self.wardrobe["slots"].items():
            for item in slot["items"]:
                self.assertEqual(item["type"], name, item["id"])

    def test_no_shipped_piece_is_gpl_only(self) -> None:
        self.assertEqual(self.wardrobe["license_policy"], "permissive")
        for name, slot in self.wardrobe["slots"].items():
            for item in slot["items"]:
                self.assertTrue(item["licenses"], f"{item['id']} ({name}) records no licence")
                for licence in item["licenses"]:
                    self.assertNotEqual(licence, "GPL", item["id"])

    def test_every_piece_of_art_in_use_can_be_attributed(self) -> None:
        credits = json.loads(
            (BUILT.parent / "credits.json").read_text(encoding="utf-8")
        )
        used = {
            item["id"] for slot in self.wardrobe["slots"].values() for item in slot["items"]
        }
        named = {item for entry in credits["entries"] for item in entry["used_by"]}

        self.assertEqual(used - named, set(), "pieces with no credit record")
        for entry in credits["entries"]:
            self.assertTrue(entry["authors"], f"{entry['file']} names nobody")
            self.assertTrue(entry["licenses"], f"{entry['file']} states no licence")

    def test_everyone_who_must_be_named_can_be(self) -> None:
        credits = json.loads((BUILT.parent / "credits.json").read_text(encoding="utf-8"))
        for entry in credits["entries"]:
            if [l for l in entry["licenses"] if l.strip() == "CC0"]:
                continue
            for author in entry["authors"]:
                self.assertNotIn(
                    author.strip().lower(),
                    WARDROBE.UNKNOWN_AUTHORS,
                    f"{entry['file']} cannot be attributed",
                )

    def test_the_hand_made_sheets_are_credited_too(self) -> None:
        credits = json.loads((BUILT.parent / "credits.json").read_text(encoding="utf-8"))
        sheets = {s for e in credits["entries"] for s in e["sources"] if s != "generated"}

        # Art used by a hand-made sheet and nothing else is exactly what a
        # wardrobe-only credits pass would miss.
        self.assertIn("scribe.png", sheets)
        self.assertTrue(
            any("generated" not in e["sources"] for e in credits["entries"]),
            "no credit entry is unique to a hand-made sheet, which is suspicious",
        )

    def test_nobody_is_armed_with_anything_but_a_cane(self) -> None:
        carried = [item["id"] for item in self.wardrobe["slots"]["weapon"]["items"]]

        self.assertEqual(carried, ["weapon_polearm_cane"])


if __name__ == "__main__":
    unittest.main()
