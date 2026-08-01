# Scribe print cards

The first authored set separates generated illustration from exact Scripture
text. Image generation supplies only the wordless block-print scene. The verse,
reference, translation provenance, theme, and progression stage live in
`content/prints.json`; `make prints` overlays them deterministically.

This prevents invented or misspelled wording, allows later translation and
accessibility work, and lets the game reveal a passage independently from the
art the Scribe has learned to carve.

## Where the two prompts come from

`generate-print-art.py` is only the runner. Its inputs live in
`content/prints.json`:

1. `image_generation.common_prompt` is shared by the whole collection. It
   defines monochrome ink, birch bark, the rough block-print style, border, blank
   text area, and exclusions.
2. Each entry's `art_prompt` describes only that card's wordless illustration.
   This is combined with the common prompt and sent to `$imagegen`.
3. Each entry's exact `verse` and `reference` are never sent to image generation.
   `build-print-cards.py` adds them afterward with the local font.

That separation is intentional: Codex creates pictures, but a human-reviewed
catalog controls which passage was found and its exact wording.

## Where the verse text comes from

`make verses` refetches every reference in `content/prints.json` and
`content/readings.json` from YouVersion and writes back exactly what the API
returned. `make verses VERSION=ASV` changes translation; anything in
`content/bible-versions.json` is accepted.

Two things it will not do quietly:

- **Partial references.** `Matthew 12:20a` names half a verse and the API only
  serves whole ones. Where the verse divides into sentences the matching
  sentence is taken. Where it does not — Matthew 12:20 is a single sentence —
  the whole verse is written and reported for a human to cut. That cut is then
  preserved across reruns, because a stored excerpt that is a literal span of
  the fetched verse is treated as reviewed. Text from a different translation is
  not a span of it, so switching versions correctly discards the old excerpt and
  asks again.
- **Cards that no longer fit.** The blank panel holds four wrapped lines and a
  reference, and nothing clips — an overlong verse draws over the illustration.
  `build-print-cards.py` now refuses to write such a card, and a test checks the
  whole catalog. This matters because a different translation of the same
  reference can be longer: moving from KJV to BSB pushed three cards past the
  border and one to five lines, which became `Mark 4:39b`.

## Add another card

For a guided prompt, run:

```bash
make add-print
```

It asks for an ID, title, theme, Bible reference, exact reviewed verse, and a
plain-language illustration description. It derives both PNG paths and appends
the complete entry to `content/prints.json` atomically.

The same operation can be scripted:

```bash
python3 scripts/add-print.py \
  --id early-welcome \
  --title "A Place at the Table" \
  --theme hospitality \
  --reference "Romans 12:13" \
  --verse "Distributing to the necessity of saints; given to hospitality." \
  --art-prompt "An ordinary host makes room at a rough table for a tired traveler; the host divides one modest loaf between them. Keep the scene humble and avoid wealth, ceremony, halos, weapons, and text."
```

This creates a catalog job equivalent to:

```json
{
  "id": "early-welcome",
  "title": "A Place at the Table",
  "theme": "hospitality",
  "reference": "Romans 12:13",
  "verse": "Distributing to the necessity of saints; given to hospitality.",
  "art": "assets/prints/early-welcome-art.png",
  "card": "assets/prints/early-welcome-card.png",
  "stage": "early_monochrome",
  "art_prompt": "An ordinary host makes room at a rough table for a tired traveler; the host divides one modest loaf between them. Keep the scene humble and avoid wealth, ceremony, halos, weapons, and text."
}
```

Review the exact verse against the named translation before adding it. Then
generate only the new card:

```bash
python3 scripts/generate-print-art.py early-welcome
```

Or add several entries first and use `make print-art`; existing art is skipped,
so only the newly cataloged jobs invoke image generation.

## First monochrome set

| ID | Need | Passage | Image |
| --- | --- | --- | --- |
| `early-hospitality` | hospitality | Hebrews 13:2 | A lamp raised for a stranger at an inn door |
| `early-burdens` | mutual aid | Galatians 6:2 | Two travelers sharing one heavy load |
| `early-bruised-reed` | gentleness | Matthew 12:20a | Hands tending a bent reed and weak wick |
| `early-rest` | rest | Matthew 11:28 | A weary traveler resting safely by the road |
| `early-light` | hope | John 1:5 | One inn light answered across a dark valley |

The text is BSB, fetched from the YouVersion Platform API by `make verses` and
not hand-entered. The Berean Standard Bible was dedicated to the public domain
(CC0) in April 2023, so the wording can live in a public repository and ship in
the game. KJV was the original choice and is no longer possible here: the whole
YouVersion catalog reachable by this key contains no English KJV, only a Thai
one. The `a` and `b` suffixes name half a verse, which the API does not do —
see below.

## Practical-help expansion

The second set draws directly from the Gideon edition's front-matter index,
especially its entries for being afraid or anxious, friends failing, leaving
home, loneliness, pain, and sorrow. The situations stay concrete enough for a
traveler who does not share the book's beliefs to recognize their own trouble
in the image and decide what, if anything, to make of its words.

| ID | Need | Passage | Image |
| --- | --- | --- | --- |
| `early-great-calm` | fear | Mark 4:39 | A storm settling around a crowded open boat |
| `early-this-day` | anxiety | Matthew 6:34b | One worker attending to one repair today |
| `early-forsaken` | friends failing | 2 Timothy 4:16 | Departing tracks, an opened hand, and a stone set down |
| `early-going-out` | leaving home | Psalm 121:8 | A traveler at a door with tracks going and returning |
| `early-comfortless` | loneliness | John 14:18 | A real human figure approaching with lamp and blanket |
| `early-perfect-weakness` | pain or frailty | 2 Corinthians 12:9a | Aging hands braiding weak fibers into strong cord |
| `early-out-of-the-mire` | sorrow or trouble | Psalm 40:2 | One traveler helping another onto firm rock |

Matthew 6:34b and 2 Corinthians 12:9a are deliberately short excerpts. Keeping
them short preserves the established 30-pixel type instead of shrinking the
words beyond what an apprentice could plausibly cut and a player could read.

## Reaching the game

`content/prints.json` is the authority on which cards exist. `make assets`
composes every catalog entry into `runtime-assets/prints/<id>-card.png`; an entry
whose `card` PNG has not been composed yet still gets a readable placeholder
carrying its title and reference, so authoring a reviewed verse is never blocked
on running the image pipeline. A Rust test asserts every catalog entry has a card
in the runtime tree.

In game these are not collected or awarded. The Scribe cuts one a night, unasked,
preferring a theme matching whatever the book last fell open at, and the
catalog's `stage` field gates which are reachable: running out of blocks that can
be cut in the colors already learned is the intended, findable reason to want
dyes. See "What the Scribe does after dark" in `docs/ARCHITECTURE.md`.

## Build

```bash
make prints
```

Cards are composed and saved at 512×768. The 30-pixel EB Garamond verse face is
intentionally large: an apprentice cutting a block by hand cannot produce tiny,
regular type. Source illustrations are reduced to the card canvas once; the
finished low-resolution image is not enlarged again.

Every catalog entry retains both paths:

- `*-art.png` is the wordless image-generation result.
- `*-card.png` is the deterministic, text-bearing game asset.

## Original image-generation prompt set

The original five assets used the built-in image generator with
`assets/prints/sower-pixel.png` as a style-and-format reference only. The shared
prompt requested a vertical game card, primitive hand-cut woodblock/linocut
translated into chunky pixel art, one-color black ink on warm fibrous birch-bark
paper, imperfect border and ink coverage, a scene confined to the upper 67–68%,
and a blank lower 26–27% text field. It explicitly prohibited words, letters,
numbers, watermarks, modern logos, photorealism, polished vector lines, and
ornate religious iconography.

The five final subject prompts were:

1. An exhausted cloaked traveler in cold rain welcomed through a weathered inn
   door by an ordinary keeper raising an oil lamp.
2. Two ordinary travelers on a ruined road carrying one bundled load together,
   with the stronger traveler steadying the tired one.
3. Two work-worn hands binding a bent reed beside a tiny clay lamp whose weak
   smoking wick remains alight.
4. A weary traveler who has set down a heavy pack and sleeps safely beneath an
   old tree, with a cup, loaf, and continuing road nearby.
5. An oil lamp in the repaired window of a battered roadside inn, answered by a
   few distant window lights across a scarred valley as dawn begins.

The seven practical-help illustrations use the same built-in generator, Sower
style reference, shared visual specification, empty lower panel, and wordless
constraint. Their reviewed subject prompts live with the exact passages in
`content/prints.json`; this keeps the resumable generator's inputs and the
finished catalog from drifting apart.

Future stages should reuse the same catalog rather than baking progression into
filenames: decorated initial and text only, monochrome motif, one spot color,
then additional registered colors as carving, ink, paper, and press skills grow.

## Resumable image-generation batch

Add catalog entries with `make add-print` or edit `content/prints.json` directly,
then run:

```bash
make print-art
```

`scripts/generate-print-art.py` invokes one ephemeral, workspace-scoped
`codex exec` job per missing source illustration. Each prompt explicitly calls
`$imagegen`, attaches the Sower card with `--image`, permits writing only the
declared `art` path, verifies that the result is a readable portrait PNG, and
then composes exact-text cards. Existing art is skipped, so rerunning the same
command resumes an interrupted collection.

Useful controls:

```bash
# Generate no more than five missing illustrations this run.
python3 scripts/generate-print-art.py --limit 5

# Generate only selected catalog entries.
python3 scripts/generate-print-art.py early-hospitality early-light

# Inspect missing jobs without invoking Codex.
python3 scripts/generate-print-art.py --dry-run

# Deliberately regenerate one existing illustration.
python3 scripts/generate-print-art.py early-light --force
```

Generation is sequential by design. A few dozen built-in image generations can
consume Codex plan limits quickly, so `--limit` supports small resumable runs.
If that becomes constraining, the later high-volume path should use the Image
Generation API with separately budgeted API usage rather than spawning parallel
Codex sessions against the same working tree.
