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

The text is KJV, which is public domain in the United States. The `a` suffix on
Matthew 12:20 identifies the deliberately short opening excerpt.

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

## Final image-generation prompt set

All five assets used the built-in image generator with
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
