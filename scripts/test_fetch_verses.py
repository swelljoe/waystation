from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = Path(__file__).with_name("fetch-verses.py")
SPEC = importlib.util.spec_from_file_location("waystation_fetch_verses", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
VERSES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERSES)


class ReferenceTests(unittest.TestCase):
    def test_ordinary_references_become_usfm(self) -> None:
        self.assertEqual(VERSES.usfm("Hebrews 13:2"), ("HEB.13.2", None))
        self.assertEqual(VERSES.usfm("Psalm 121:8"), ("PSA.121.8", None))
        self.assertEqual(VERSES.usfm("Mark 4:39"), ("MRK.4.39", None))
        self.assertEqual(VERSES.usfm("John 1:5"), ("JHN.1.5", None))

    def test_numbered_books_keep_their_number(self) -> None:
        self.assertEqual(VERSES.usfm("2 Timothy 4:16"), ("2TI.4.16", None))
        self.assertEqual(VERSES.usfm("2 Corinthians 12:9a"), ("2CO.12.9", "a"))
        self.assertEqual(VERSES.usfm("1 John 4:7"), ("1JN.4.7", None))

    def test_an_en_dash_range_survives(self) -> None:
        # The authored file uses an en dash, which is not valid in a USFM id.
        self.assertEqual(VERSES.usfm("Ecclesiastes 4:9–10"), ("ECC.4.9-10", None))
        self.assertEqual(VERSES.usfm("Ecclesiastes 4:9-10"), ("ECC.4.9-10", None))

    def test_part_markers_are_returned_separately(self) -> None:
        self.assertEqual(VERSES.usfm("Matthew 12:20a"), ("MAT.12.20", "a"))
        self.assertEqual(VERSES.usfm("Matthew 6:34b"), ("MAT.6.34", "b"))

    def test_singular_and_plural_psalms_agree(self) -> None:
        self.assertEqual(VERSES.usfm("Psalms 40:2"), VERSES.usfm("Psalm 40:2"))

    def test_a_reference_it_cannot_read_is_refused(self) -> None:
        for bad in ["Hezekiah 3:1", "Hebrews", "13:2", ""]:
            with self.assertRaises(VERSES.ReferenceError, msg=bad):
                VERSES.usfm(bad)


class CleanTests(unittest.TestCase):
    def test_bracketed_punctuation_marks_are_dropped(self) -> None:
        self.assertEqual(
            VERSES.clean("Today has enough trouble of its own.[’’]"),
            "Today has enough trouble of its own.",
        )

    def test_a_bracketed_clause_is_left_alone(self) -> None:
        # Brackets around real words mark a textually doubtful passage. Removing
        # those would change what the verse says.
        text = "and [the disciples were amazed] at his teaching"
        self.assertEqual(VERSES.clean(text), text)

    def test_whitespace_is_flattened(self) -> None:
        self.assertEqual(VERSES.clean("  two   are\n better  "), "two are better")


class ExcerptTests(unittest.TestCase):
    MULTI = (
        "Therefore do not worry about tomorrow, for tomorrow will worry about "
        "itself. Today has enough trouble of its own."
    )
    SINGLE = (
        "A bruised reed He will not break, and a smoldering wick He will not "
        "extinguish, till He leads justice to victory."
    )

    def test_a_whole_reference_takes_the_whole_verse(self) -> None:
        self.assertEqual(VERSES.excerpt(self.MULTI, None), (self.MULTI, False))

    def test_a_and_b_take_the_first_and_last_sentence(self) -> None:
        self.assertEqual(
            VERSES.excerpt(self.MULTI, "b"),
            ("Today has enough trouble of its own.", False),
        )
        self.assertEqual(
            VERSES.excerpt(self.MULTI, "a"),
            (
                "Therefore do not worry about tomorrow, for tomorrow will worry "
                "about itself.",
                False,
            ),
        )

    def test_a_single_sentence_verse_is_handed_back_for_review(self) -> None:
        text, needs_review = VERSES.excerpt(self.SINGLE, "a")

        self.assertEqual(text, self.SINGLE)
        self.assertTrue(needs_review)

    def test_a_reviewed_excerpt_survives_a_rerun(self) -> None:
        reviewed = "A bruised reed He will not break"

        self.assertEqual(VERSES.excerpt(self.SINGLE, "a", reviewed), (reviewed, False))

    def test_the_whole_verse_is_not_mistaken_for_a_reviewed_excerpt(self) -> None:
        # This is what the previous run wrote when it could not divide the
        # verse, not a cut anybody approved; it must still ask.
        self.assertEqual(VERSES.excerpt(self.SINGLE, "a", self.SINGLE)[1], True)

    def test_stale_text_from_another_translation_is_replaced(self) -> None:
        kjv = "A bruised reed shall he not break"

        text, _ = VERSES.excerpt(self.SINGLE, "a", kjv)

        self.assertNotEqual(text, kjv)


class CardExcerptTests(unittest.TestCase):
    """A card holds a phrase, so `span_only` never takes a whole verse quietly."""

    VERSE = (
        "Do not neglect to show hospitality to strangers, for by so doing some "
        "people have entertained angels without knowing it."
    )

    def test_a_reviewed_span_survives_a_rerun(self) -> None:
        span = "Do not neglect to show hospitality to strangers"

        self.assertEqual(
            VERSES.excerpt(self.VERSE, None, span, span_only=True), (span, False)
        )

    def test_a_whole_reference_no_longer_takes_the_whole_verse(self) -> None:
        # Without `span_only` this is a reading, and a reading wants the verse.
        self.assertEqual(VERSES.excerpt(self.VERSE, None), (self.VERSE, False))

        text, needs_review = VERSES.excerpt(self.VERSE, None, span_only=True)

        self.assertEqual(text, self.VERSE)
        self.assertTrue(needs_review, "a card was given a whole verse to hold")

    def test_text_that_is_not_a_span_is_handed_back_for_review(self) -> None:
        for existing in ["", "Show hospitality unto strangers", self.VERSE]:
            with self.subTest(existing=existing):
                self.assertTrue(
                    VERSES.excerpt(self.VERSE, None, existing, span_only=True)[1]
                )


class ShippedContentTests(unittest.TestCase):
    def documents(self) -> list[tuple[str, list[dict]]]:
        return [
            ("prints", json.loads(VERSES.PRINTS.read_text(encoding="utf-8"))["prints"]),
            ("readings", json.loads(VERSES.READINGS.read_text(encoding="utf-8"))["readings"]),
        ]

    def test_every_shipped_reference_can_be_resolved(self) -> None:
        for name, entries in self.documents():
            for entry in entries:
                with self.subTest(file=name, reference=entry["reference"]):
                    VERSES.usfm(entry["reference"])

    def test_every_shipped_verse_has_text(self) -> None:
        for name, entries in self.documents():
            for entry in entries:
                with self.subTest(file=name, id=entry["id"]):
                    self.assertTrue(entry["verse"].strip())

    def test_the_translation_block_names_its_source_and_licence(self) -> None:
        for path in (VERSES.PRINTS, VERSES.READINGS):
            translation = json.loads(path.read_text(encoding="utf-8"))["translation"]
            self.assertEqual(translation["source"], "YouVersion Platform API")
            self.assertIn("youversion_id", translation)
            self.assertTrue(translation["licence_note"].strip())

    def test_the_two_files_agree_on_shared_references(self) -> None:
        # Keyed by USFM rather than by the printed reference: a card cites the
        # whole verse it quotes from, so `Matthew 12:20` on a card and
        # `Matthew 12:20a` in the book are the same place in the same Bible.
        def keyed(entries: list[dict]) -> dict[str, str]:
            return {VERSES.usfm(e["reference"])[0]: e["verse"] for e in entries}

        prints = keyed(self.documents()[0][1])
        readings = keyed(self.documents()[1][1])

        for verse_id in set(prints) & set(readings):
            with self.subTest(verse=verse_id):
                self.assertIn(
                    prints[verse_id],
                    readings[verse_id],
                    "the card quotes words the book does not have",
                )

    def test_every_card_carries_the_usfm_id_the_server_will_ask_for(self) -> None:
        for entry in self.documents()[0][1]:
            with self.subTest(id=entry["id"]):
                self.assertEqual(
                    entry["passage_id"], VERSES.usfm(entry["reference"])[0]
                )

    def test_no_card_cites_half_a_verse(self) -> None:
        # A card carries a phrase, so the `a`/`b` distinction it once made is
        # gone — and a part marker has no rendering in the sixty-odd languages
        # a card can arrive in.
        for entry in self.documents()[0][1]:
            with self.subTest(id=entry["id"]):
                self.assertIsNone(VERSES.usfm(entry["reference"])[1])

    def test_every_card_holds_a_phrase_rather_than_a_verse(self) -> None:
        # Room for a translation that renders longer than the English does.
        for entry in self.documents()[0][1]:
            with self.subTest(id=entry["id"]):
                self.assertLessEqual(len(entry["verse"].split()), 8)


if __name__ == "__main__":
    unittest.main()
