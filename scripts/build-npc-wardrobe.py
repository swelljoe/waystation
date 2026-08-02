#!/usr/bin/env python3
"""Rebuild the curated NPC wardrobe from the Universal LPC Spritesheet Generator.

The generator ships 656 sheet definitions covering lizards, jetpacks, plate
armour and wizard hats. A waystation in a dead valley wants none of that, so
this script keeps an explicit allowlist and copies only the facts the runtime
generator needs out of the LPC definitions: which body types an item actually
draws for, whether its colour lives in a `variant` or a palette `recolor`, and
which colours it offers.

Everything the runtime cannot discover for itself — weights, gendered head
families, which colours read as "cleared out of a ruin" rather than "bought new
in a bright era" — lives in the ALLOW tables below. Prune there, rerun, and the
game picks up the change with no Rust edits.

    python3 scripts/build-npc-wardrobe.py
    python3 scripts/build-npc-wardrobe.py --lpc ~/src/Universal-LPC-...
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LPC = Path.home() / "src" / "Universal-LPC-Spritesheet-Character-Generator"
OUTPUT = ROOT / "crates/npcgen/data/wardrobe.json"
CREDITS = ROOT / "crates/npcgen/data/credits.json"
CREDITS_MD = ROOT / "docs/NPC_ART_CREDITS.md"
# Character sheets drawn by hand in the LPC web tool rather than generated, and
# the geometry that identifies one: the full action sheet, 13 frames by 54 rows.
HANDMADE_DIR = ROOT / "assets/custom"
ACTION_SHEET_SIZE = (13 * 64, 54 * 64)
# Project-authored LPC pieces — art drawn here rather than downloaded. Laid out
# exactly like the upstream checkout (`sheet_definitions/` beside
# `spritesheets/`) so the same loader, the same sprite-existence check and the
# same licence gate apply to it without special cases, and so a finished piece
# can be handed back to OpenGameArt as-is.
OVERLAY = ROOT / "assets/custom/lpc"

# Body types the valley produces, and how often. Nobody arrives on a horse and
# nobody arrives as a skeleton, so the exotic LPC bases are simply absent.
#
# `muscular` and `pregnant` are absent for a duller reason: neither has a shirt.
# LPC draws no torso clothing for the muscular base beyond a pair of suspenders,
# and the pregnant shirts on disk are per-colour files where their definitions
# promise one recoloured sheet — so the app cannot draw them either, and a
# traveller on either base would arrive bare-chested every time. Both become
# usable the moment somebody draws or converts a shirt; add them back here and
# the checks below will say whether it worked. See docs/NPC_GENERATOR.md.
BODY_TYPES = [
    ("male", 34),
    ("female", 34),
    ("teen", 18),
    ("child", 14),
]

# Skin tones, from the `body` material palette. The LPC body palette also holds
# fur, zombie and fantasy greens; those are not people who walk down this road.
SKIN = [
    ("light", 4),
    ("amber", 4),
    ("olive", 4),
    ("taupe", 3),
    ("bronze", 3),
    ("brown", 4),
    ("black", 3),
]

# Hair, from the `hair` material palette. Dye is a later-era luxury and hair dye
# never becomes one, so the unnatural half of the palette is dropped entirely.
#
# Two of the plausible-sounding names are traps. ULPC's `ash` is not an ash
# blonde — it runs dark plum through mauve to cream, and reads as purple hair.
# `ginger` tops out at a neon yellow. Both are gone. Grey, white and platinum
# live only in HAIR_OLD, because a white-haired teenager reads as a costume.
HAIR = [
    ("black", 6),
    ("raven", 3),
    ("dark_brown", 6),
    ("chestnut", 5),
    ("light_brown", 5),
    ("sandy", 3),
    ("blonde", 3),
    ("strawberry", 2),
    ("carrot", 2),
    ("redhead", 2),
    # Black going grey. Plausible on anyone past thirty, so it stays here too.
    ("dark_gray", 1),
]

# What an old head keeps. Drawn instead of HAIR when the head reads as elderly,
# so a man with a lined face does not turn up with a jet-black beard.
HAIR_OLD = [
    ("gray", 6),
    ("dark_gray", 5),
    ("white", 5),
    ("platinum", 3),
    ("light_brown", 1),
    ("dark_brown", 1),
]

# Cloth, from the `cloth` material palette, in two eras. Early on, everything
# anyone wears was scavenged, mended, or dyed with what grows in a dry valley:
# browns, tans, black, undyed white, and the greens and oranges a plant dye can
# still reach. The story later brings real dye back, and BRIGHT is what that
# buys — it is layered on top of MUTED rather than replacing it, because a
# settlement that can afford red does not stop owning brown.
CLOTH_MUTED = [
    ("brown", 8),
    ("tan", 8),
    ("leather", 6),
    ("walnut", 6),
    ("charcoal", 5),
    ("black", 4),
    ("slate", 4),
    ("gray", 4),
    ("white", 3),
    ("forest", 3),
    ("green", 3),
    ("bluegray", 2),
    ("orange", 2),
    ("rose", 2),
    ("maroon", 2),
]
CLOTH_BRIGHT = CLOTH_MUTED + [
    ("red", 4),
    ("yellow", 4),
    ("teal", 3),
    ("blue", 3),
    ("navy", 3),
    ("purple", 2),
    ("lavender", 2),
    ("sky", 2),
    ("pink", 2),
]

# Colour sources a slot can draw from.
#   cloth  — the era table above, intersected with what the item offers
#   hair   — whatever hair colour this character already has
#   skin   — whatever skin tone this character already has
#   fixed  — the item's own vocabulary, unrelated to the palettes (e.g. "cane")
#   none   — the item has exactly one appearance
CLOTH, HAIR_SRC, SKIN_SRC, FIXED, NONE = "cloth", "hair", "skin", "fixed", "none"

# The allowlist. Each entry is (item_id, colour source, weight, tags).
#
# `weight` is relative within its slot. `tags` gate coherence in the Rust
# generator: "masc"/"fem"/"child" pair a head with a body, "elderly" pulls in
# grey hair, wrinkles and a better chance of a cane.
#
# A slot's `chance` is the probability it is filled at all; slots without one
# are always filled. Everything omitted here was deliberately cut — see
# docs/NPC_GENERATOR.md for what was cut and why.
SLOTS: dict[str, dict] = {
    "body": {
        "chance": 1.0,
        "items": [("body", SKIN_SRC, 1, [])],
    },
    "head": {
        "chance": 1.0,
        "items": [
            ("heads_human_male", SKIN_SRC, 10, ["masc"]),
            ("heads_human_male_small", SKIN_SRC, 5, ["masc"]),
            ("heads_human_male_gaunt", SKIN_SRC, 5, ["masc"]),
            # `heads_human_male_plump` is absent: the catalogue credits its
            # plump variant to "??" and offers it only as CC-BY-SA, so it cannot
            # be attributed as its licence requires. Restore it by naming the
            # artist upstream, or by adding it to KEEP_UNATTRIBUTED.
            ("heads_human_male_elderly", SKIN_SRC, 4, ["masc", "elderly"]),
            ("heads_human_female", SKIN_SRC, 10, ["fem"]),
            ("heads_human_female_small", SKIN_SRC, 6, ["fem"]),
            ("heads_human_female_elderly", SKIN_SRC, 4, ["fem", "elderly"]),
            ("heads_human_elderly_small", SKIN_SRC, 3, ["masc", "fem", "elderly"]),
            ("heads_human_child", SKIN_SRC, 1, ["child"]),
        ],
    },
    # Faces are drawn from a path built out of the head's name, so only the
    # human heads above have them at all — and the child head has none.
    "expression": {
        "chance": 1.0,
        "items": [
            ("face_neutral", SKIN_SRC, 20, []),
            ("face_sad", SKIN_SRC, 5, []),
            ("face_closed", SKIN_SRC, 3, []),
            ("face_closing", SKIN_SRC, 2, []),
            ("face_happy", SKIN_SRC, 4, []),
            ("face_shame", SKIN_SRC, 2, []),
        ],
    },
    "nose": {
        "chance": 0.55,
        "items": [
            ("head_nose_straight", SKIN_SRC, 8, []),
            ("head_nose_button", SKIN_SRC, 4, []),
            ("head_nose_big", SKIN_SRC, 3, []),
            ("head_nose_large", SKIN_SRC, 2, []),
            ("head_nose_elderly", SKIN_SRC, 4, ["elderly_only"]),
        ],
    },
    "wrinkles": {
        "chance": 1.0,
        "items": [("head_wrinkles", SKIN_SRC, 1, ["elderly_only"])],
    },
    "eyebrows": {
        "chance": 0.9,
        "items": [
            ("eyebrows_thick", HAIR_SRC, 5, []),
            ("eyebrows_thin", HAIR_SRC, 5, []),
        ],
    },
    # Practical cuts only: nothing that needs a mirror, a stylist, or a stage.
    # A bare head is rare on purpose — thinning hair is better told by the
    # balding and buzzcut styles than by nobody having any.
    "hair": {
        "chance": 0.97,
        "items": [
            ("hair_plain", HAIR_SRC, 6, []),
            ("hair_parted", HAIR_SRC, 5, []),
            ("hair_parted2", HAIR_SRC, 4, []),
            ("hair_parted3", HAIR_SRC, 4, []),
            ("hair_parted_side_bangs", HAIR_SRC, 4, []),
            ("hair_parted_side_bangs2", HAIR_SRC, 3, []),
            ("hair_messy1", HAIR_SRC, 5, []),
            ("hair_messy2", HAIR_SRC, 4, []),
            ("hair_messy3", HAIR_SRC, 4, []),
            ("hair_halfmessy", HAIR_SRC, 3, []),
            ("hair_bedhead", HAIR_SRC, 4, []),
            ("hair_unkempt", HAIR_SRC, 4, []),
            ("hair_mop", HAIR_SRC, 3, []),
            ("hair_bangs", HAIR_SRC, 4, []),
            ("hair_bangsshort", HAIR_SRC, 3, []),
            ("hair_page", HAIR_SRC, 3, []),
            ("hair_page2", HAIR_SRC, 3, []),
            ("hair_pixie", HAIR_SRC, 4, []),
            ("hair_swoop", HAIR_SRC, 3, []),
            ("hair_swoop_side", HAIR_SRC, 3, []),
            ("hair_cowlick", HAIR_SRC, 3, []),
            ("hair_cowlick_tall", HAIR_SRC, 2, []),
            ("hair_curtains", HAIR_SRC, 3, []),
            ("hair_single", HAIR_SRC, 2, []),
            ("hair_buzzcut", HAIR_SRC, 4, ["masc"]),
            ("hair_high_and_tight", HAIR_SRC, 3, ["masc"]),
            ("hair_balding", HAIR_SRC, 3, ["masc", "elderly"]),
            ("hair_spiked", HAIR_SRC, 2, []),
            ("hair_spiked2", HAIR_SRC, 2, []),
            ("hair_afro", HAIR_SRC, 4, []),
            ("hair_natural", HAIR_SRC, 4, []),
            ("hair_jewfro", HAIR_SRC, 3, []),
            ("hair_cornrows", HAIR_SRC, 4, []),
            ("hair_twists_straight", HAIR_SRC, 3, []),
            ("hair_twists_fade", HAIR_SRC, 3, []),
            ("hair_flat_top_fade", HAIR_SRC, 2, []),
            ("hair_flat_top_straight", HAIR_SRC, 2, []),
            ("hair_dreadlocks_short", HAIR_SRC, 3, []),
            ("hair_dreadlocks_long", HAIR_SRC, 3, []),
            ("hair_curly_short", HAIR_SRC, 4, []),
            ("hair_curly_short2", HAIR_SRC, 3, []),
            ("hair_curly_long", HAIR_SRC, 3, []),
            ("hair_curls_large", HAIR_SRC, 3, []),
            ("hair_curls_large_xlong", HAIR_SRC, 2, []),
            ("hair_bob", HAIR_SRC, 4, []),
            ("hair_bob_side_part", HAIR_SRC, 3, []),
            ("hair_lob", HAIR_SRC, 3, []),
            ("hair_relm_short", HAIR_SRC, 3, []),
            ("hair_wavy", HAIR_SRC, 4, []),
            ("hair_wavy_child", HAIR_SRC, 4, ["child"]),
            ("hair_long", HAIR_SRC, 4, []),
            ("hair_long_straight", HAIR_SRC, 3, []),
            ("hair_long_center_part", HAIR_SRC, 3, []),
            ("hair_long_messy", HAIR_SRC, 3, []),
            ("hair_long_messy2", HAIR_SRC, 3, []),
            ("hair_loose", HAIR_SRC, 3, []),
            ("hair_bangslong", HAIR_SRC, 3, []),
            ("hair_bangslong2", HAIR_SRC, 2, []),
            ("hair_curtains_long", HAIR_SRC, 2, []),
            ("hair_xlong", HAIR_SRC, 2, []),
            ("hair_xlong_wavy", HAIR_SRC, 2, []),
            ("hair_relm_xlong", HAIR_SRC, 2, []),
            ("hair_braid", HAIR_SRC, 3, []),
            ("hair_braid2", HAIR_SRC, 3, []),
            ("hair_half_up", HAIR_SRC, 3, []),
            ("hair_ponytail", HAIR_SRC, 4, []),
            ("hair_ponytail2", HAIR_SRC, 3, []),
            ("hair_high_ponytail", HAIR_SRC, 3, []),
            ("hair_relm_ponytail", HAIR_SRC, 3, []),
            ("hair_bangs_bun", HAIR_SRC, 3, []),
            ("hair_shoulderl", HAIR_SRC, 2, []),
            ("hair_shoulderr", HAIR_SRC, 2, []),
            ("hair_topknot_short", HAIR_SRC, 2, []),
            ("hair_topknot_short2", HAIR_SRC, 2, []),
            ("hair_topknot_long", HAIR_SRC, 2, []),
            ("hair_topknot_long2", HAIR_SRC, 2, []),
            ("hair_pigtails_bangs", HAIR_SRC, 2, []),
            ("hair_bunches", HAIR_SRC, 2, []),
        ],
    },
    "beard": {
        "chance": 0.45,
        "items": [
            ("beards_5oclock_shadow", HAIR_SRC, 6, ["masc"]),
            ("beards_trimmed", HAIR_SRC, 5, ["masc"]),
            ("beards_beard", HAIR_SRC, 5, ["masc"]),
            ("beards_medium", HAIR_SRC, 4, ["masc"]),
            ("beards_winter", HAIR_SRC, 3, ["masc", "elderly"]),
        ],
    },
    "mustache": {
        "chance": 0.18,
        "items": [
            ("beards_mustache", HAIR_SRC, 5, ["masc"]),
            ("beards_chevron", HAIR_SRC, 4, ["masc"]),
            ("beards_horseshoe", HAIR_SRC, 3, ["masc"]),
            ("beards_walrus", HAIR_SRC, 3, ["masc", "elderly"]),
            ("beards_lampshade", HAIR_SRC, 2, ["masc"]),
            ("beards_bigstache", HAIR_SRC, 2, ["masc"]),
        ],
    },
    "clothes": {
        "chance": 1.0,
        "items": [
            ("torso_clothes_longsleeve", CLOTH, 8, []),
            ("torso_clothes_longsleeve2", CLOTH, 7, []),
            ("torso_clothes_longsleeve2_buttoned", CLOTH, 5, []),
            ("torso_clothes_longsleeve2_scoop", CLOTH, 4, []),
            ("torso_clothes_longsleeve2_vneck", CLOTH, 4, []),
            ("torso_clothes_longsleeve2_cardigan", CLOTH, 4, []),
            ("torso_clothes_longsleeve_scoop", CLOTH, 4, []),
            ("torso_clothes_shortsleeve", CLOTH, 6, []),
            ("torso_clothes_shortsleeve_cardigan", CLOTH, 3, []),
            ("torso_clothes_tshirt", CLOTH, 5, []),
            ("torso_clothes_tshirt_buttoned", CLOTH, 4, []),
            ("torso_clothes_tshirt_scoop", CLOTH, 3, []),
            ("torso_clothes_tshirt_vneck", CLOTH, 3, []),
            ("torso_clothes_sleeveless1", CLOTH, 3, []),
            ("torso_clothes_sleeveless2", CLOTH, 3, []),
            ("torso_clothes_sleeveless2_buttoned", CLOTH, 2, []),
            ("torso_clothes_sleeveless", CLOTH, 2, []),
            ("torso_clothes_tunic", CLOTH, 5, []),
            ("torso_clothes_tunic_sara", CLOTH, 4, []),
            ("torso_clothes_blouse", CLOTH, 4, []),
            ("torso_clothes_blouse_longsleeve", CLOTH, 4, []),
            ("torso_clothes_child_shirt", CLOTH, 6, ["child"]),
        ],
    },
    "legs": {
        "chance": 1.0,
        "items": [
            ("legs_pants", CLOTH, 9, []),
            ("legs_pants2", CLOTH, 8, []),
            ("legs_cuffed", CLOTH, 6, []),
            ("legs_pantaloons", CLOTH, 5, []),
            ("legs_hose", CLOTH, 3, []),
            ("legs_leggings", CLOTH, 3, []),
            ("legs_leggings2", CLOTH, 3, []),
            ("legs_shorts", CLOTH, 3, []),
            ("legs_skirts_plain", CLOTH, 4, []),
            ("legs_skirt_straight", CLOTH, 3, []),
            ("legs_childpants", CLOTH, 6, ["child"]),
            ("legs_childskirts", CLOTH, 4, ["child"]),
        ],
    },
    # No child shoes exist in LPC, so children go barefoot; adults sometimes do
    # too, which is its own kind of characterisation.
    "shoes": {
        "chance": 0.9,
        "items": [
            ("feet_shoes_basic", CLOTH, 8, []),
            ("feet_shoes_revised", CLOTH, 6, []),
            ("feet_shoes_sara", CLOTH, 4, []),
            ("feet_boots_basic", CLOTH, 6, []),
            ("feet_boots_revised", CLOTH, 5, []),
            ("feet_boots_fold", CLOTH, 4, []),
            ("feet_boots_rim", CLOTH, 4, []),
            ("feet_sandals", CLOTH, 3, []),
        ],
    },
    "belt": {
        "chance": 0.3,
        "items": [
            ("belt_leather", CLOTH, 6, []),
            ("belt_leather2", CLOTH, 4, []),
            ("belt_loose", CLOTH, 4, []),
            ("belt_double", CLOTH, 2, []),
        ],
    },
    "sash": {
        "chance": 0.15,
        "items": [
            ("belt_sash", CLOTH, 4, []),
            ("belt_sash_narrow", CLOTH, 4, []),
            ("belt_waistband", CLOTH, 3, []),
        ],
    },
    "apron": {
        "chance": 0.12,
        "items": [
            ("torso_aprons_apron", CLOTH, 5, []),
            ("torso_aprons_apron_half", CLOTH, 4, []),
            ("torso_aprons_apron_full", CLOTH, 3, []),
        ],
    },
    "overalls": {
        "chance": 0.12,
        "items": [
            ("torso_aprons_overalls", CLOTH, 4, []),
            ("torso_aprons_suspenders", CLOTH, 4, []),
        ],
    },
    "neck": {
        "chance": 0.12,
        "items": [("neck_scarf", CLOTH, 1, [])],
    },
    "headcover": {
        "chance": 0.14,
        "items": [
            ("hat_headband_kerchief", CLOTH, 5, []),
            ("hat_headband_tied", CLOTH, 4, []),
            ("hat_headband_thick", CLOTH, 3, []),
        ],
    },
    "bandana": {
        "chance": 0.08,
        "items": [
            ("hat_bandana", CLOTH, 5, []),
            ("hat_bandana2", CLOTH, 3, []),
        ],
    },
    "hat": {
        "chance": 0.1,
        "items": [
            ("hat_hood_cloth", CLOTH, 5, []),
            ("hat_hood_sack_cloth", CLOTH, 3, []),
            ("hat_hood_hijab", CLOTH, 3, []),
        ],
    },
    # `backpack_basket` is deliberately absent: it is a back-carried pannier,
    # so from the front it is a bright wicker frame sticking out around the
    # head, in a gold that belongs to no palette here.
    "backpack": {
        "chance": 0.22,
        "items": [
            ("backpack", CLOTH, 6, []),
            ("backpack_squarepack", CLOTH, 4, []),
        ],
    },
    # The only thing anyone carries. A cane is a walking aid, not a weapon, and
    # it is the one entry from the LPC polearm category that belongs here.
    "weapon": {
        "chance": 0.07,
        "items": [("weapon_polearm_cane", FIXED, 1, ["elderly_favoured"])],
    },
}

# Colour names the era tables use. Any item variant outside this set is dropped
# rather than silently offered, so "lightblue" and "forest green" spellings from
# one-off LPC items cannot leak into a palette that has no such colour.
KNOWN_COLOURS = {name for name, _ in CLOTH_BRIGHT}


# --- Licensing -------------------------------------------------------------
#
# LPC art is typically offered under several licences at once and the user picks
# one. Waystation is not open source, so GPL is not a licence this project can
# pick; a file whose only offer is GPL cannot ship. Everything below exists to
# make that impossible to do by accident.
#
# Licence strings in the catalogue are inconsistent — `OGA-BY-3.0` beside
# `OGA-BY 3.0`, trailing `+` on several — so they are matched by family rather
# than compared literally.
LICENCE_FAMILIES = ("CC0", "CC-BY-SA", "CC-BY", "OGA-BY", "OGA-SA", "GPL")

# Two policies, because "not GPL" and "attribution only" are different bars.
#
#   permissive   — anything but GPL-only. CC-BY-SA is share-alike: it does not
#                  reach the game's own source the way GPL would, but it does
#                  attach to the sprite sheets themselves and to anything
#                  derived from them, which composited travellers are.
#   attribution  — CC0, CC-BY or OGA-BY only. No copyleft of any kind touches
#                  the art. Costs the cane, both backpacks and the scarf.
LICENCE_POLICIES = {
    "permissive": {"CC0", "CC-BY", "CC-BY-SA", "OGA-BY"},
    "attribution": {"CC0", "CC-BY", "OGA-BY"},
}

# Placeholders the catalogue uses where it does not know who drew something.
# Every licence here except CC0 requires naming the author, so art credited to
# one of these cannot be shipped correctly — the About screen would have a hole
# in it. Listing an id in KEEP_UNATTRIBUTED is a decision to accept that; the
# point of the list is that the decision has to be written down.
UNKNOWN_AUTHORS = {"", "?", "??", "???", "unknown", "anonymous"}
KEEP_UNATTRIBUTED: set[str] = set()


def licence_family(licence: str) -> str:
    """Which licence family a catalogue string belongs to."""
    text = licence.strip().replace("OGA-BY-3.0", "OGA-BY 3.0")
    for family in LICENCE_FAMILIES:
        if text.startswith(family):
            return family
    return f"unrecognised:{text}"


def credits_for(definition: dict) -> list[tuple[str, dict]]:
    """An item's credit entries, keyed by the path prefix each one covers."""
    return [(entry.get("file", "").rstrip("/"), entry) for entry in definition.get("credits", [])]


def check_licences(
    item_id: str, definition: dict, used: list[str], allowed: set[str]
) -> tuple[set[str], list[str]]:
    """Verify every sprite path this piece draws is licensed to us, and say how.

    The web app's own GPL filter keeps an item when *any one* of its credit
    entries has an enabled licence, which is too weak to rely on: a piece whose
    male art is triple-licensed and whose female art is GPL-only would pass.
    This checks each path against the credit entries that actually cover it, and
    refuses art that has no offer this project can accept.

    Returns the licence families this piece can be used under, plus one
    complaint per path that cannot be used at all. Complaints are collected
    rather than raised so a policy change reports its whole bill at once
    instead of one item per run.
    """
    entries = credits_for(definition)
    families: set[str] = set()
    refused: list[str] = []
    for path in used:
        covering = [
            entry for prefix, entry in entries if path == prefix or path.startswith(prefix + "/")
        ]
        if not covering:
            refused.append(f"{item_id}: {path} has no credit entry, so it cannot be attributed")
            continue
        for entry in covering:
            offered = {licence_family(name) for name in entry.get("licenses", [])}
            usable = offered & allowed
            if usable:
                families |= usable
            else:
                refused.append(
                    f"{item_id}: {path} is offered only as "
                    f"{', '.join(sorted(offered)) or 'nothing'}"
                )
            authors = [a.strip() for a in entry.get("authors", [])]
            unnamed = [a for a in authors if a.lower() in UNKNOWN_AUTHORS]
            if unnamed and item_id not in KEEP_UNATTRIBUTED and usable != {"CC0"}:
                refused.append(
                    f"{item_id}: {path} credits an author the catalogue cannot name "
                    f"({', '.join(unnamed)}), so it cannot be attributed as its licence requires"
                )
    return families, refused


def load_definitions(lpc: Path, overlay: Path = OVERLAY) -> dict[str, tuple[dict, Path, str]]:
    """Every sheet definition, from upstream and from our own art.

    Returns the definition, the sprite root its paths resolve against, and where
    it came from. Keeping the overlay in the same shape as upstream means a
    project-drawn piece is checked exactly as strictly as a downloaded one — it
    still has to have its sprites on disk, still has to carry a credit block,
    and still has to name a licence this project can use.
    """
    if not (lpc / "sheet_definitions").is_dir():
        raise SystemExit(f"no sheet_definitions under {lpc}")

    found: dict[str, tuple[dict, Path, str]] = {}
    for source, origin in ((lpc, "lpc"), (overlay, "overlay")):
        root = source / "sheet_definitions"
        if not root.is_dir():
            continue
        for path in sorted(root.rglob("*.json")):
            if path.name.startswith("meta_"):
                continue
            if path.stem in found and origin == "overlay":
                # Deliberate, but worth saying out loud: from here on the
                # project's own art is what the game draws for that id.
                print(f"  overlay replaces upstream {path.stem}")
            found[path.stem] = (json.loads(path.read_text()), source, origin)
    return found


# Animation folders a piece must actually have on disk to be usable.
#
# The game draws visitors from the walk rows and nothing else — they approach,
# they stand (walk frame 0), they leave — so that is the honest bar. LPC's own
# coverage past walk is patchy in ways that would prune useful pieces for no
# gain: the child body has six animations, and every nose is missing `climb`.
# When the game starts playing more of a visitor's sheet, widen this list and
# rerun; anything that can no longer be drawn will disappear from the wardrobe.
REQUIRED_ANIMATIONS = ["walk"]


def declared_bodies(definition: dict) -> set[str]:
    """Body types the item's layers claim art for."""
    bodies: set[str] = set()
    for index in range(1, 9):
        layer = definition.get(f"layer_{index}")
        if isinstance(layer, dict):
            bodies |= {key for key in layer if key != "zPos"}
    return bodies - {"is_mask", "custom_animation"}


def expand_placeholders(path: str, definition: dict) -> list[str]:
    """Every real path a `${type}`-templated layer path can become.

    Faces sit under a directory named for the head above them, so their layer
    path is a template. Since the generator pairs any head with any expression,
    every substitution the definition offers has to exist.
    """
    paths = [path]
    while any("${" in candidate for candidate in paths):
        grown = []
        for candidate in paths:
            if "${" not in candidate:
                grown.append(candidate)
                continue
            start = candidate.index("${")
            end = candidate.index("}", start)
            key = candidate[start + 2 : end]
            values = set(definition.get("replace_in_path", {}).get(key, {}).values())
            if not values:
                return []
            grown += [candidate[:start] + value + candidate[end + 1 :] for value in values]
        paths = grown
    return paths


def pinned_paths(definition: dict, body: str, selections: dict) -> list[str]:
    """Layer paths for one specific character, with placeholders resolved.

    The wardrobe expands `${head}` to every substitution because the generator
    pairs any head with any expression. A hand-made character has exactly one
    head, so expanding all of them would credit female face art to a male
    Scribe. Here the placeholder is resolved from the character's own
    selections, the way the app does when it draws them.
    """
    found: set[str] = set()
    for index in range(1, 9):
        layer = definition.get(f"layer_{index}")
        if not layer or body not in layer:
            continue
        path = layer[body]
        while "${" in path:
            start = path.index("${")
            end = path.index("}", start)
            key = path[start + 2 : end]
            chosen = selections.get(key)
            if not chosen:
                return []
            # Exported names carry the colour in brackets: `Human Male (light)`.
            name = chosen["name"].split(" (")[0].replace(" ", "_")
            value = definition.get("replace_in_path", {}).get(key, {}).get(name)
            if not value:
                return []
            path = path[:start] + value + path[end + 1 :]
        found.add(path.rstrip("/"))
    return sorted(found)


def load_handmade(definitions: dict, allowed: set[str]) -> tuple[list[tuple], list[str]]:
    """Attribution for the character sheets drawn by hand, not generated.

    The Scribe and the four named visitors were built in the LPC web tool and
    exported beside their PNGs. They ship today, so their art carries the same
    obligations as anything the generator produces — and being hand-made is
    exactly why they would otherwise be missed.
    """
    refused: list[str] = []
    records: list[tuple] = []

    # A character sheet is recognisable by its geometry: the full LPC action
    # sheet, 13 frames across by 54 rows. Anything that shape in the custom art
    # folder is a person, and a person with no export cannot be credited.
    for art in sorted(HANDMADE_DIR.glob("*.png")):
        with Image.open(art) as image:
            if image.size != ACTION_SHEET_SIZE:
                continue
        record = art.with_suffix(".txt")
        if not record.is_file():
            refused.append(
                f"{art.relative_to(ROOT)} is an LPC action sheet with no "
                f"{record.name} beside it, so nobody can say who drew it — "
                "re-export the character from the LPC generator and save the JSON"
            )
            continue

        character = json.loads(record.read_text())
        body = character.get("bodyType", "male")
        for selection in character.get("selections", {}).values():
            item_id = selection["itemId"]
            entry = definitions.get(item_id)
            if not entry:
                refused.append(f"{record.name}: no sheet definition named {item_id}")
                continue
            definition = entry[0]
            paths = pinned_paths(definition, body, character["selections"])
            if not paths:
                # Selected but undrawable on this base — the app allows it and
                # renders nothing. No art is used, so nothing to credit.
                continue
            _, complaints = check_licences(item_id, definition, paths, allowed)
            refused += [f"{art.name}: {c}" for c in complaints]
            records.append((art.name, item_id, definition, paths))
    return records, refused


def supported_bodies(lpc: Path, definition: dict, item_id: str, field: str, options: list[str]):
    """Body types the item actually draws for, confirmed against the sprites.

    Two different lies have to be caught here. A layer keyed only by `male`
    draws nothing on a female base, and the app happily lets you select it
    anyway. Worse, some layers name a body type whose sprites were never
    converted to the palette scheme and simply are not on disk — the pregnant
    shirts are per-colour files where the definition promises one recoloured
    sheet — so the app cannot draw them either. Both come out as an invisible
    garment, which reads as a rendering bug rather than a catalogue gap.
    """
    sheets = lpc / "spritesheets"
    kept = []
    for body in sorted(declared_bodies(definition)):
        complete = True
        for index in range(1, 9):
            layer = definition.get(f"layer_{index}")
            if not layer or body not in layer:
                continue
            for base in expand_placeholders(layer[body], definition) or ["\0"]:
                for folder in REQUIRED_ANIMATIONS:
                    if field == "variant":
                        wanted = [f"{base}{folder}/{v.replace(' ', '_')}.png" for v in options]
                    else:
                        wanted = [f"{base}{folder}.png"]
                    complete &= all((sheets / name).is_file() for name in wanted)
        if complete:
            kept.append(body)
        else:
            print(f"  dropped {item_id} for {body}: sprites missing on disk")
    return kept


def layer_paths(definition: dict, bodies: list[str]) -> list[str]:
    """Every sprite directory this piece draws from, for the given body types.

    This is what the licence check runs against, so it is deliberately narrowed
    to bases the game generates. `body.json` has art for six bases and only four
    are ever rolled; the two that are not should not drag their licensing in.
    """
    found: set[str] = set()
    for index in range(1, 9):
        layer = definition.get(f"layer_{index}")
        if not layer:
            continue
        for body in bodies:
            if body in layer:
                found |= {path.rstrip("/") for path in expand_placeholders(layer[body], definition)}
    return sorted(found)


def colour_field(definition: dict) -> tuple[str, list[str]]:
    """Where this item's colour goes, and what colours it accepts.

    Older LPC assets ship one baked PNG per colour and name it in `variants`;
    newer ones ship a single sheet recoloured at draw time against a palette
    named in `recolors`. The exported character JSON puts the chosen colour in
    a different field for each, so the runtime has to know which it is.
    """
    variants = definition.get("variants")
    if variants:
        return "variant", list(variants)
    if definition.get("recolors"):
        return "recolor", []
    return "none", []


# --- Drawing ---------------------------------------------------------------
#
# Everything above decides *what* a traveller wears. What follows records how to
# draw it, because the game composites travellers itself at runtime rather than
# handing selections to the web tool.
#
# Two facts are needed and neither is derivable from the wardrobe as it stood:
# the sprite directory each layer reads from, in draw order, and the palette
# swap a recoloured piece needs. Both are copied out of the LPC catalogue here
# so the runtime never has to parse a sheet definition.


def load_materials(lpc: Path) -> dict[str, dict]:
    """Every palette material, with its versions, base colour and ramps."""
    materials: dict[str, dict] = {}
    for meta in sorted((lpc / "palette_definitions").glob("*/meta_*.json")):
        entry = json.loads(meta.read_text())
        entry["palettes"] = {
            version.stem.split("_", 1)[1]: json.loads(version.read_text())
            for version in sorted(meta.parent.glob("*.json"))
            if not version.name.startswith("meta_")
        }
        materials[meta.parent.name] = entry
    return materials


def palette_ramp(materials: dict, material: str, version: str, colour: str) -> list[str] | None:
    """The colours a named palette entry is made of.

    Tried in the material's own default version first, then anywhere else that
    has an entry by that name — the generator only ever names a bare colour, and
    a few materials keep their colours in a version other than the default.
    """
    meta = materials.get(material)
    if not meta:
        return None
    found = meta["palettes"].get(version, {}).get(colour)
    if found:
        return found
    for palette in meta["palettes"].values():
        if colour in palette:
            return palette[colour]
    return None


def recolor_slot(definition: dict) -> dict | None:
    """The one colour slot the generator ever chooses.

    A piece with a single colour states its material inline; a piece with more
    than one — a hair with a tie, a head with eyes — nests them under `color_1`,
    `color_2`. Only the first is ever picked, here and in the app; the rest keep
    whatever they were drawn in.
    """
    recolors = definition.get("recolors")
    if not recolors:
        return None
    if "material" in recolors:
        return recolors
    return recolors[sorted(recolors)[0]]


def recolor_source(materials: dict, definition: dict, item_id: str) -> dict | None:
    """The palette swap this piece needs: material, and the ramp it is drawn in.

    The target ramp is not recorded per item — it depends on the colour rolled
    for the traveller, and lives in `materials` under that colour's name.
    """
    slot = recolor_slot(definition)
    if not slot:
        return None
    material = slot["material"]
    base = slot.get("base") or materials[material]["base"]
    version = materials[material]["default"]
    if "." in base:
        version, base = base.split(".", 1)
    source = slot.get("source") or palette_ramp(materials, material, version, base)
    if not source:
        raise SystemExit(f"{item_id}: no {material} ramp named '{base}' to recolour away from")
    return {"material": material, "from": source}


def draw_layers(definition: dict, bodies: list[str], item_id: str) -> tuple[list[dict], dict]:
    """Sprite directories this piece draws, in the order they stack.

    `zPos` is kept rather than resolved into an index because it orders a piece
    against every *other* piece as well: a hat at 130 must land above hair at
    120 whatever else the traveller is wearing.

    Paths keep their `${head}`-style placeholders. Resolving them here would
    mean one entry per head the piece can sit under, when the runtime knows
    which head it picked and can substitute in a line of code. The substitution
    table travels with the piece so it can.
    """
    layers: list[dict] = []
    wanted: set[str] = set()
    for index in range(1, 9):
        layer = definition.get(f"layer_{index}")
        if not layer:
            continue
        paths = {body: layer[body] for body in bodies if body in layer}
        if not paths:
            continue
        if "zPos" not in layer:
            raise SystemExit(f"{item_id}: layer_{index} has no zPos, so it cannot be stacked")
        layers.append({"z": layer["zPos"], "paths": paths})
        for path in paths.values():
            while "${" in path:
                start = path.index("${")
                end = path.index("}", start)
                wanted.add(path[start + 2 : end])
                path = path[end + 1 :]

    replace = {
        key: value
        for key, value in definition.get("replace_in_path", {}).items()
        if key in wanted
    }
    missing = wanted - set(replace)
    if missing:
        raise SystemExit(
            f"{item_id}: layer paths use ${{{', '.join(sorted(missing))}}} with no "
            "replace_in_path table, so the runtime cannot resolve them"
        )
    return layers, replace


def check_stray_colours(item_id: str, definition: dict) -> None:
    """Refuse pieces that carry a colour nobody in this game ever chose.

    A few LPC assets have a second colour slot — a hair tie, an eye colour — and
    ship it with a hard-coded `source` palette instead of a material default.
    The generator only ever picks the first slot, so the second renders in
    whatever the artist drew: `hair_long_tied` arrives with a magenta band and
    `hair_pigtails` with a cyan one. Eyes are the exception; their defaults are
    real eye colours and every human head relies on them.
    """
    recolors = definition.get("recolors")
    if not isinstance(recolors, dict) or "material" in recolors:
        return
    for key, slot in sorted(recolors.items())[1:]:
        if slot.get("source") and slot.get("material") != "eye":
            raise SystemExit(
                f"{item_id}: {key} ({slot.get('type_name')}) has a hard-coded "
                f"{slot.get('material')} palette this generator never picks, so it would "
                "render in a colour nothing else in the wardrobe uses"
            )


def build_entry(
    lpc: Path,
    item_id: str,
    source: str,
    weight: int,
    tags: list[str],
    definition: dict,
    allowed_licences: set[str],
    materials: dict,
) -> dict:
    check_stray_colours(item_id, definition)
    field, variants = colour_field(definition)

    options: list[str] = []
    if source in (CLOTH, FIXED) and field == "variant":
        # Palette-sourced items are limited to the colours they actually ship;
        # fixed items keep their own vocabulary whatever it looks like.
        options = [v for v in variants if v in KNOWN_COLOURS] if source == CLOTH else variants
        if not options:
            raise SystemExit(f"{item_id}: no usable variants left from {variants}")
    elif source == FIXED and field == "recolor":
        raise SystemExit(f"{item_id}: fixed colour asked for, but the item is palette-recoloured")
    elif source == CLOTH and field == "none":
        raise SystemExit(f"{item_id}: cloth colour asked for, but the item has no colour at all")

    bodies = supported_bodies(lpc, definition, item_id, field, options)
    if not bodies:
        raise SystemExit(f"{item_id}: no body type has complete art for this piece")

    generated = [body for body in bodies if body in {name for name, _ in BODY_TYPES}]
    if not generated:
        raise SystemExit(
            f"{item_id}: draws only for {', '.join(bodies)}, and no traveller uses those bases"
        )
    licences, refused = check_licences(
        item_id, definition, layer_paths(definition, generated), allowed_licences
    )

    # Narrowed to the bases the game rolls, for the same reason `layer_paths`
    # is: those are the only paths whose licensing was checked, so those are the
    # only ones whose art may be copied into the shipped runtime tree.
    layers, replace = draw_layers(definition, generated, item_id)
    if not layers:
        raise SystemExit(f"{item_id}: no layer draws for any body type the game generates")

    entry = {
        "id": item_id,
        "name": definition["name"],
        "type": definition["type_name"],
        "bodies": bodies,
        "field": field,
        "source": source,
        "weight": weight,
        # How this piece's art may be used, in family form. `CC-BY-SA` here
        # means the art carries share-alike and there was no plainer offer.
        "licenses": sorted(licences),
        "layers": layers,
    }
    if tags:
        entry["tags"] = tags
    if options:
        entry["options"] = options
    if replace:
        entry["replace"] = replace
    if field == "recolor":
        entry["recolor"] = recolor_source(materials, definition, item_id)
    return entry, refused


def collect_attribution(definitions: dict, wardrobe: dict, handmade: list[tuple]) -> list[dict]:
    """Everyone who has to be named, and under what terms, for the art in use.

    One record per credit entry the project actually touches, from both sources
    that exist: the generator's wardrobe, and the character sheets drawn by hand
    in the LPC web tool. Keeping them in one file is the point — the way to miss
    somebody is to have two lists and check one.
    """
    generated = {base["id"] for base in wardrobe["body_types"]}
    wanted: dict[str, dict] = {}

    def record(prefix: str, entry: dict, user: str, source: str) -> None:
        found = wanted.setdefault(
            prefix,
            {
                "file": prefix,
                "authors": list(entry.get("authors", [])),
                "licenses": list(entry.get("licenses", [])),
                "urls": list(entry.get("urls", [])),
                "notes": entry.get("notes", "").strip(),
                "sources": [],
                "used_by": [],
            },
        )
        if user not in found["used_by"]:
            found["used_by"].append(user)
        if source not in found["sources"]:
            found["sources"].append(source)

    for slot in wardrobe["slots"].values():
        for item in slot["items"]:
            definition, _, _ = definitions[item["id"]]
            bodies = [body for body in item["bodies"] if body in generated]
            entries = credits_for(definition)
            for path in layer_paths(definition, bodies):
                for prefix, entry in entries:
                    if path == prefix or path.startswith(prefix + "/"):
                        record(prefix, entry, item["id"], "generated")

    for sheet, item_id, definition, paths in handmade:
        entries = credits_for(definition)
        for path in paths:
            for prefix, entry in entries:
                if path == prefix or path.startswith(prefix + "/"):
                    record(prefix, entry, item_id, sheet)

    for found in wanted.values():
        found["used_by"].sort()
        found["sources"].sort()
    return [wanted[key] for key in sorted(wanted)]


def lpc_revision(lpc: Path) -> str:
    try:
        out = subprocess.run(
            ["git", "-C", str(lpc), "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def reachable_colours(source: str) -> list[str]:
    """Every colour a piece drawing from this source can ever be asked for.

    Used to narrow the palette ramps shipped to the runtime to the ones the
    generator can actually roll, so the game is not carrying wizard-blue.
    """
    if source == SKIN_SRC:
        return [name for name, _ in SKIN]
    if source == HAIR_SRC:
        return [name for name, _ in HAIR] + [name for name, _ in HAIR_OLD]
    if source == CLOTH:
        return [name for name, _ in CLOTH_BRIGHT]
    return []


def build(lpc: Path, policy: str) -> tuple[dict, list[dict]]:
    definitions = load_definitions(lpc)
    materials = load_materials(lpc)
    if not materials:
        raise SystemExit(f"no palette_definitions under {lpc}")
    allowed = LICENCE_POLICIES[policy]
    handmade, unlicensed = load_handmade(definitions, allowed)
    # Which colours each material has to ship a ramp for, gathered as the items
    # are built: a material is only ever asked for the colours the slots that
    # use it can roll.
    ramps_wanted: dict[str, set[str]] = {}
    slots = {}
    for slot_name, spec in SLOTS.items():
        entries = []
        for item_id, source, weight, tags in spec["items"]:
            if item_id not in definitions:
                raise SystemExit(f"{slot_name}: no sheet definition named {item_id}")
            definition, sprite_root, origin = definitions[item_id]
            entry, refused = build_entry(
                sprite_root, item_id, source, weight, tags, definition, allowed, materials
            )
            if "recolor" in entry:
                material = entry["recolor"]["material"]
                ramps_wanted.setdefault(material, set())
                ramps_wanted[material] |= set(reachable_colours(source))
            if origin == "overlay":
                entry["origin"] = origin
            unlicensed += refused
            if entry["type"] != slot_name:
                raise SystemExit(
                    f"{item_id} is type_name '{entry['type']}', listed under slot '{slot_name}'"
                )
            entries.append(entry)
        slots[slot_name] = {"chance": spec["chance"], "items": entries}

    if unlicensed:
        listing = "\n  ".join(unlicensed)
        raise SystemExit(
            f"{len(unlicensed)} problems under the '{policy}' licence policy:\n"
            f"  {listing}\n"
            "For generated pieces, remove them from SLOTS or choose a policy that accepts\n"
            "their licences. For a hand-made sheet, rebuild the character in the LPC tool\n"
            "without the offending piece — it is already shipping."
        )

    ramps: dict[str, dict[str, list[str]]] = {}
    for material, colours in sorted(ramps_wanted.items()):
        version = materials[material]["default"]
        found = {}
        for colour in sorted(colours):
            ramp = palette_ramp(materials, material, version, colour)
            if ramp:
                found[colour] = ramp
        if found:
            ramps[material] = found

    wardrobe = {
        "note": "Generated by scripts/build-npc-wardrobe.py — edit the script, not this file.",
        "lpc_revision": lpc_revision(lpc),
        "license_policy": policy,
        "body_types": [{"id": name, "weight": w} for name, w in BODY_TYPES],
        # Target ramps for every recolour the generator can roll. A piece names
        # the material and the ramp it was drawn in; the colour rolled for the
        # traveller names the ramp it becomes.
        "materials": ramps,
        "palettes": {
            "skin": [{"color": c, "weight": w} for c, w in SKIN],
            "hair": [{"color": c, "weight": w} for c, w in HAIR],
            "hair_old": [{"color": c, "weight": w} for c, w in HAIR_OLD],
            "cloth_muted": [{"color": c, "weight": w} for c, w in CLOTH_MUTED],
            "cloth_bright": [{"color": c, "weight": w} for c, w in CLOTH_BRIGHT],
        },
        "slots": slots,
    }
    return wardrobe, collect_attribution(definitions, wardrobe, handmade)


def attribution_markdown(attribution: list[dict], revision: str, policy: str) -> str:
    """The same attribution as prose, so the obligation is reviewable in git.

    Every author who has to be named, every licence the art is used under, and
    the source page for each. This is the text the game's About screen owes.
    """
    authors: dict[str, set[str]] = {}
    relied_on: set[str] = set()
    declined: set[str] = set()
    for entry in attribution:
        for name in (name.strip() for name in entry["licenses"]):
            # LPC offers most art under several licences at once. Listing the
            # GPL offers as "in use" would say the opposite of what is true.
            target = declined if licence_family(name) not in LICENCE_POLICIES[policy] else relied_on
            target.add(name)
        for author in entry["authors"]:
            authors.setdefault(author, set()).add(entry["file"])

    lines = [
        "# NPC art credits",
        "",
        "Generated by `scripts/build-npc-wardrobe.py` from the Universal LPC",
        f"Spritesheet Generator at revision `{revision}`. Do not edit by hand.",
        "",
        f"Licence policy: **{policy}**. No art here is GPL-only; every file below is",
        "used under one of the non-GPL licences it is also offered under.",
        "",
        f"{len(authors)} people to name across {len(attribution)} source files.",
        "",
        "## Authors",
        "",
    ]
    lines += [f"- {author}" for author in sorted(authors, key=str.lower)]
    lines += ["", "## Licences relied on", ""]
    lines += [f"- {licence}" for licence in sorted(relied_on)]
    lines += [
        "",
        "## Also offered upstream, and declined",
        "",
        "LPC offers most of this art under several licences at once. These are the",
        "offers Waystation does not take; they are listed so the choice is on the",
        "record, not because anything here is used under them.",
        "",
    ]
    lines += [f"- {licence}" for licence in sorted(declined)] or ["- none"]
    lines += [
        "",
        "## Per-file",
        "",
        "`Used under` is the offer Waystation takes; `also offered` is the rest.",
        "",
        "| Art | Authors | Used under | Also offered | Source |",
        "| --- | --- | --- | --- | --- |",
    ]
    for entry in attribution:
        urls = " ".join(f"[link]({url})" for url in entry["urls"][:2]) or "—"
        offers = [name.strip() for name in entry["licenses"]]
        taken = [n for n in offers if licence_family(n) in LICENCE_POLICIES[policy]]
        rest = [n for n in offers if n not in taken]
        lines.append(
            f"| `{entry['file']}` | {', '.join(entry['authors'])} | "
            f"{', '.join(taken)} | {', '.join(rest) or '—'} | {urls} |"
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lpc", type=Path, default=DEFAULT_LPC, help="LPC generator checkout")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--credits", type=Path, default=CREDITS)
    parser.add_argument("--credits-markdown", type=Path, default=CREDITS_MD)
    parser.add_argument(
        "--licenses",
        choices=sorted(LICENCE_POLICIES),
        default="permissive",
        help="permissive excludes GPL-only art; attribution also excludes share-alike",
    )
    args = parser.parse_args()

    wardrobe, attribution = build(args.lpc, args.licenses)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(wardrobe, indent=2) + "\n")
    args.credits.parent.mkdir(parents=True, exist_ok=True)
    args.credits.write_text(
        json.dumps(
            {
                "note": "Every LPC credit entry the wardrobe draws from. Source for the About screen.",
                "lpc_revision": wardrobe["lpc_revision"],
                "license_policy": args.licenses,
                "entries": attribution,
            },
            indent=2,
        )
        + "\n"
    )

    args.credits_markdown.write_text(
        attribution_markdown(attribution, wardrobe["lpc_revision"], args.licenses)
    )

    items = sum(len(slot["items"]) for slot in wardrobe["slots"].values())
    share_alike = sorted(
        item["id"]
        for slot in wardrobe["slots"].values()
        for item in slot["items"]
        if not set(item["licenses"]) & {"CC0", "CC-BY", "OGA-BY"}
    )
    print(f"{args.output.relative_to(ROOT)}: {len(wardrobe['slots'])} slots, {items} items")
    print(f"{args.credits.relative_to(ROOT)}: {len(attribution)} credit entries to name")
    print(f"{args.credits_markdown.relative_to(ROOT)}: written")
    print(f"licence policy: {args.licenses} — no GPL-only art is included")

    if share_alike:
        print(
            f"share-alike (CC-BY-SA) with no plainer offer: {len(share_alike)} generated "
            "pieces — rerun with --licenses attribution to exclude them"
        )

    # Which shipped sheets carry share-alike art. These are the ones that need
    # the bundle and the EULA carve-out; the rest are attribution-only.
    attribution_only = {"CC0", "CC-BY", "OGA-BY"}
    for sheet in sorted({s for e in attribution for s in e["sources"] if s != "generated"}):
        encumbered = [
            entry["file"]
            for entry in attribution
            if sheet in entry["sources"]
            and not {licence_family(n) for n in entry["licenses"]} & attribution_only
        ]
        state = "share-alike via " + ", ".join(encumbered) if encumbered else "attribution-only"
        print(f"  {sheet:20} {state}")


if __name__ == "__main__":
    main()
