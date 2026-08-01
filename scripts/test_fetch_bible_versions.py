from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = Path(__file__).with_name("fetch-bible-versions.py")
SPEC = importlib.util.spec_from_file_location("waystation_fetch_bible_versions", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
FETCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FETCH)


def version(
    version_id: int,
    abbreviation: str,
    language: str,
    books: list[str],
    title: str = "",
) -> dict:
    return {
        "id": version_id,
        "abbreviation": abbreviation,
        "language_tag": language,
        "books": books,
        "title": title or abbreviation,
    }


FULL = ["GEN", "PSA", "ISA", "MAT", "LUK", "GAL"]
NEW_TESTAMENT = ["MAT", "LUK", "GAL"]


class RequiredBooksTests(unittest.TestCase):
    def test_books_come_from_the_authored_passages(self) -> None:
        ron = """[
            ( id: "PSA.34.18", need_id: "comfort" ),
            ( id: "MAT.11.28-30", need_id: "rest" ),
            ( id: "PSA.23.4", need_id: "presence" ),
        ]"""

        self.assertEqual(FETCH.required_books(ron), ["MAT", "PSA"])

    def test_an_empty_catalog_of_passages_is_refused(self) -> None:
        with self.assertRaisesRegex(ValueError, "no passage ids"):
            FETCH.required_books("[]")

    def test_the_shipped_passages_still_parse(self) -> None:
        ron = (ROOT / "content" / "passages.ron").read_text(encoding="utf-8")

        self.assertEqual(FETCH.required_books(ron), ["GAL", "ISA", "LUK", "MAT", "PSA"])


class BuildMapTests(unittest.TestCase):
    def base(self) -> list[dict]:
        return [version(3034, "BSB", "en", FULL), version(12, "ASV", "en", FULL)]

    def test_new_testament_only_versions_are_never_offered(self) -> None:
        versions = self.base() + [
            version(900, "ESNT", "es", NEW_TESTAMENT),
            version(901, "ESFULL", "es", FULL),
        ]

        mapping = FETCH.build_map(versions, ["PSA", "MAT"])
        spanish = next(item for item in mapping["languages"] if item["language"] == "es")

        self.assertEqual(spanish["abbreviation"], "ESFULL")
        self.assertEqual(spanish["alternatives"], [])

    def test_a_language_with_only_a_new_testament_is_dropped_entirely(self) -> None:
        versions = self.base() + [version(902, "PTNT", "pt", NEW_TESTAMENT)]

        mapping = FETCH.build_map(versions, ["PSA", "MAT"])

        self.assertNotIn("pt", [item["language"] for item in mapping["languages"]])

    def test_english_is_pinned_to_the_version_the_fixtures_use(self) -> None:
        # A fuller book list would otherwise win, and disagree with every
        # reviewed fixture in content/passages.ron.
        versions = self.base() + [version(42, "CPDV", "en", FULL + ["TOB", "SIR"])]

        mapping = FETCH.build_map(versions, ["PSA", "MAT"])
        english = next(item for item in mapping["languages"] if item["language"] == "en")

        self.assertEqual(english["abbreviation"], "BSB")
        self.assertIn("CPDV", [item["abbreviation"] for item in english["alternatives"]])

    def test_alternatives_carry_their_own_language(self) -> None:
        mapping = FETCH.build_map(self.base(), ["PSA", "MAT"])
        english = next(item for item in mapping["languages"] if item["language"] == "en")

        self.assertEqual([item["language"] for item in english["alternatives"]], ["en"])

    def test_losing_the_english_fallback_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "nothing to fall back to"):
            FETCH.build_map([version(901, "ESFULL", "es", FULL)], ["PSA", "MAT"])

    def test_rebuilding_the_same_catalog_does_not_churn_the_file(self) -> None:
        versions = self.base() + [
            version(901, "ESA", "es", FULL),
            version(902, "ESB", "es", FULL),
        ]

        first = FETCH.build_map(versions, ["PSA", "MAT"])
        second = FETCH.build_map(list(reversed(versions)), ["PSA", "MAT"])

        self.assertEqual(first, second)


class ShippedCatalogTests(unittest.TestCase):
    def test_the_committed_map_matches_the_committed_passages(self) -> None:
        mapping = json.loads((ROOT / "content" / "bible-versions.json").read_text(encoding="utf-8"))
        ron = (ROOT / "content" / "passages.ron").read_text(encoding="utf-8")

        self.assertEqual(mapping["required_books"], FETCH.required_books(ron))
        self.assertEqual(mapping["fallback"]["abbreviation"], FETCH.FALLBACK_ABBREVIATION)

        languages = {item["language"] for item in mapping["languages"]}
        self.assertIn(FETCH.FALLBACK_LANGUAGE, languages)
        for item in mapping["languages"]:
            self.assertTrue(item["abbreviation"], item)
            for alternative in item["alternatives"]:
                self.assertEqual(alternative["language"], item["language"])


if __name__ == "__main__":
    unittest.main()
