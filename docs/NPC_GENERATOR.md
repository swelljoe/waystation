# NPC generator

The waystation gets strangers for as long as the fire keeps burning. Four
hand-drawn sheets run out long before the game does, so `crates/npcgen` builds
new travellers at runtime: a body, a face that suits it, and clothes that look
like they came out of a valley rather than off a fantasy rack.

`crates/npcgen` produces **Universal LPC Spritesheet Generator selections**, not
pixels — a generated traveller can be opened in the LPC web tool from a link,
looked at, adjusted, and exported as a finished action sheet, so when something
comes out wrong the fix is visible rather than guessed at. The game composites
those selections itself, at runtime; see **In the game** below.

## Pieces

| Path | What it is |
| --- | --- |
| `crates/npcgen/src/lib.rs` | The generator. Seeded, deterministic, no dependencies beyond serde. |
| `crates/npcgen/data/wardrobe.json` | The curated catalogue, compiled into the binary. Generated; do not hand-edit. |
| `crates/npcgen/data/credits.json` | Every author, licence and source URL for the art in use. Generated. |
| `docs/NPC_ART_CREDITS.md` | The same attribution as prose, for review. Generated. |
| `scripts/build-npc-wardrobe.py` | Writes the wardrobe from a local LPC checkout. **This is where you prune.** |
| `crates/npcgen/src/bin/npc-preview.rs` | Writes a cast of `character.json` files plus a page of links. |
| `scripts/preview-npcs.py` | Composites those files into a contact sheet, and verifies itself against hand-made characters. |
| `scripts/build-npc-art.py` | Copies the walk sheets the wardrobe names into `runtime-assets/npc/`, and writes the reference sheets the game is tested against. |
| `crates/game/src/npc_art.rs` | Stacks those sheets into one texture, in the running game. |
| `crates/game/src/visitors.rs` | Who arrives, in what shape, and what they say first. |
| `content/openings.ron` | The first thing a stranger says. **This is where you add lines.** |
| `assets/custom/lpc/` | Project-drawn LPC pieces, laid out like an upstream checkout. See its README. |
| `scripts/lpc-art-split.py` | Splits a sprite into editable hair/shadow layers and rejoins them exactly. |
| `scripts/check-lpc-art.py` | Refuses hand-edited art that would not recolour. |

## Looking at what it makes

```
make npcs                    # 24 travellers, early era
make npcs ERA=dyed COUNT=48  # the later, brighter palette
```

`npc-preview` also takes `--cast grown|elder|youth|child`, which is what the
game asks for rather than "anybody" — an arrival is an authored shape with
people of particular ages standing in it.

That writes to `target/npc-preview/`:

- `npc-NNN.json` — load any of these into the LPC web generator with **Import**.
- `index.html` — one clickable link per traveller, opening them in the web tool
  fully dressed. Fastest way to inspect one closely.
- `contact-sheet.png` — the whole cast, standing and facing you.

Every traveller comes from a seed, and the same seed always makes the same
person, so a rogue result can be reported by number and reproduced.

Both scripts expect the LPC generator checked out at
`~/src/Universal-LPC-Spritesheet-Character-Generator`; pass `LPC=/some/path`
otherwise.

## Pruning

Everything the runtime cannot work out for itself lives in the tables at the top
of `scripts/build-npc-wardrobe.py`: body types, palettes, and a slot-by-slot
allowlist of LPC item ids with weights. Edit those, run `make wardrobe`, and the
game picks up the change with no Rust edits.

- **A piece looks wrong** — delete its line from `SLOTS`.
- **A piece is licensed wrong** — it never reaches the wardrobe; see Licensing.
- **A colour looks wrong** — drop it from `CLOTH_MUTED`, `HAIR`, or `SKIN`.
- **Something appears too often** — lower its weight, or the slot's `chance`.
- **You want more variety** — add an item id from `sheet_definitions/`. The
  builder will refuse it if its `type_name` does not match the slot, if its
  sprites are not on disk, or if it carries a stray colour (see below).

The builder copies three facts out of LPC for each piece: which body types it
*actually draws for*, whether its colour goes in the `variant` or `recolor`
field, and which colours it offers.

## What was cut, and why

**No armour, helmets, visors, shields, capes, or weapons.** The one carried item
is the cane from the polearm category, weighted towards old travellers.

**No non-human anything** — no beast, lizard, orc, goblin, skeleton or zombie
heads; no tails, wings, horns, fins, or furry ears. No prostheses, wheelchair, or
wounds; those are characterisation the story should choose, not dice.

**No fancy dress** — no dresses, corsets, bodices, tricornes, bicornes, wizard
hats, crowns, tiaras, jabots, cravats, bowties, or formal shirts. Hats are three
plain hoods, three headbands, and two bandanas.

**No jewellery** — necklaces, amulets, gems, earrings and rings are all out.

**Hair is practical cuts only.** Liberty spikes, beehives, mohawks and idol cuts
are gone; the 78 that remain are things somebody could do to their own hair.

Three cuts were made after looking at rendered output rather than at names:

- **`ash` and `ginger` hair.** ULPC's `ash` is not an ash blonde — it runs dark
  plum through mauve to cream, and reads as purple hair. `ginger` tops out at a
  neon yellow.
- **`hair_long_tied` and `hair_pigtails`.** Both have a second colour slot for
  the hair tie, hard-coded to a magenta and a cyan the generator never picks and
  no palette here contains. The builder now refuses any piece like this, so it
  cannot happen again quietly. Making them usable means teaching the generator
  to emit LPC sub-selections (`subId`), which nothing needs yet.
- **`backpack_basket`.** A back-carried pannier, so from the front it is a bright
  wicker frame sticking out around the head.

## Licensing

**Nothing in the wardrobe is GPL-only.** Nothing in the entire LPC catalogue is,
in fact: GPL always appears alongside OGA-BY, CC-BY-SA, CC-BY or CC0, as an
additional offer rather than the only one. Waystation picks a non-GPL offer for
every file, and `make wardrobe` fails if it ever cannot.

Do not rely on the web app's GPL checkbox for this. `isItemLicenseCompatible`
keeps an item when **any one** of its credit entries has an enabled licence, so
a piece whose male art is triple-licensed and whose female art is GPL-only
passes its filter. The builder checks each sprite path against the credit
entries that actually cover it, restricted to the body types the game generates.

Two policies exist:

| `--licenses` | Accepts | Cost |
| --- | --- | --- |
| `permissive` (default) | CC0, CC-BY, CC-BY-SA, OGA-BY | none |
| `attribution` | CC0, CC-BY, OGA-BY | 35 items, listed below |

The difference matters. **CC-BY-SA is share-alike, not just attribution.** It
does not reach the game's own source the way GPL would, but it attaches to the
sprite sheets and to anything derived from them — and a composited traveller is
a derivative. 35 of 179 items have no plainer offer:

- **24 hairstyles** — `parted`, `parted2`, `messy1/2/3`, `bedhead`, `unkempt`,
  `mop`, `bangs`, `bangsshort`, `bangslong`, `bangslong2`, `pixie`, `swoop`,
  `cowlick_tall`, `high_and_tight`, `spiked2`, `long`, `long_messy2`, `loose`,
  `braid2`, `ponytail`, `pigtails_bangs`, `bunches`
- **3 noses** — straight, button, big (leaving `large` and `elderly`)
- **2 beards and 2 moustaches** — the basic beard, 5 o'clock shadow, basic
  moustache, bigstache
- **both backpacks**, **the scarf**, and **the cane**

Running `--licenses attribution` prints all 43 offending paths at once rather
than stopping at the first, so the bill is visible before you decide. It would
empty the `neck`, `backpack` and `weapon` slots outright — the cane goes.

`heads_human_male_plump` was cut on these grounds already: the catalogue credits
its plump variant to `??` and offers it only as CC-BY-SA, so it cannot be
attributed the way its licence requires. Art credited to an unnamed author is
refused unless its id is listed in `KEEP_UNATTRIBUTED`, so accepting one is a
decision written into the script rather than an oversight. CC0 art is exempt,
since CC0 waives attribution.

### Screenshots and flat images

Share-alike is fine inside a running game — the game is not a derivative of the
art, and the two licence worlds never meet in one file. A **screenshot is a
different matter**: it flattens travellers and purchased tilesets into a single
image, and purchased packs cannot be relicensed CC-BY-SA to match. Whether a
screenshot counts as an adaptation is genuinely contested, so the cheap move is
not to have the argument.

Generate anyone headed for a screenshot, trailer, or store art with the licence
bar raised:

```
make npcs ART=attribution-only
cargo run -p waystation-npcgen --bin npc-preview -- --art attribution-only
```

In code, `generate_for(seed, era, ArtLicense::AttributionOnly)` refuses
share-alike art outright. `Npc::art_license()` reports what a traveller actually
came out as, and `Npc::encumbered_pieces()` names the pieces responsible when it
is share-alike — so you can swap one hairstyle rather than reroll the person.

The bar costs the cane, both backpacks, the scarf, most noses and about a third
of the hairstyles. Roughly a third of default travellers are already
attribution-only without asking.

### Attribution

`make wardrobe` writes two credit files covering the art actually in use, from
**both** sources: the generator's wardrobe and the character sheets drawn by
hand in the LPC web tool.

- `crates/npcgen/data/credits.json` — machine-readable, for building the About
  screen: authors, licences, source URLs, which items use each file, and whether
  it arrived via the generator or a named sheet.
- `docs/NPC_ART_CREDITS.md` — the same thing as prose, so the obligation is
  reviewable and diffable in git.

That is **39 people across 215 source files**. Neither file is compiled into the
game yet, because the About screen does not exist; when it does, `credits.json`
is what it should read.

Covering the hand-made sheets is not cosmetic: `torso/clothes/longsleeve/
longsleeves/male` is used by the Scribe and by nothing the generator produces,
so a wardrobe-only credits pass would have missed it entirely.

Any 832×3456 PNG in `assets/custom/` is an LPC action sheet, and the build
**fails** if one has no `.txt` export beside it — there would be no way to say
who drew it. Where each shipped sheet stands today:

| Sheet | Licence | Encumbered by |
| --- | --- | --- |
| `black-teen.png` | attribution-only | — |
| `little-sister.png` | attribution-only | — |
| `scribe.png` | share-alike | `hair/long` |
| `old-guy.png` | share-alike | basic beard, straight nose, scarf |
| `redhead-lady.png` | share-alike | backpack, pixie hair, cane |

The Scribe is share-alike on the strength of one hairstyle, and he is in every
gameplay screenshot there will ever be.

## Drawing your own pieces

`assets/custom/lpc/` is an overlay in the same shape as the upstream checkout —
`sheet_definitions/` beside `spritesheets/` — and `make wardrobe` reads it too.
A piece there is checked exactly as strictly as a downloaded one: sprites must
exist for every body type it claims, it must carry a credit block, and it must
name a licence this project can use. An id that already exists upstream is
replaced, and the build says so. Overlay items are marked `"origin": "overlay"`
in the wardrobe.

Two things make hand-editing LPC art survivable, both learned the hard way from
`long_messy`:

**The art is two layers, and they separate on alpha alone.** Recolourable art is
fully opaque in exactly the six colours of its material's base ramp; cast shadows
are flat black at alpha 64, deliberately off-ramp so a shadow stays a shadow
rather than turning orange on a redhead. `lpc-art-split.py` divides them and
proves the rejoin is byte-exact before writing anything. With the shadow out of
the way, the hair layer can be indexed to six colours, which locks a pencil to
the palette. There is no layered master for this art anywhere upstream — the
split you keep is the only one there is.

**Off-ramp pixels fail silently.** A pixel painted in any other colour is not
recoloured; it keeps whatever it was, so one anti-aliased edge becomes a speck
that survives onto every hair colour, and you would not see it until you rendered
a colour you were not looking at — possibly after editing 352 frames.
`check-lpc-art.py` catches off-ramp opaque pixels, unexpected alpha values,
frames that are not whole, and sheets that went missing or changed size against
the art you derived from. Run it after every save.

## What LPC cannot currently do

These are gaps in the art, found by checking every piece against the files on
disk rather than trusting the sheet definitions. All of them would resolve
themselves if the upstream art were drawn or converted.

- **The `muscular` base has no shirt.** Not one torso garment in the whole
  catalogue draws for it, only a pair of suspenders.
- **The `pregnant` base has no usable shirt either.** Three exist on disk, but as
  per-colour files where their definitions promise a single recoloured sheet, so
  the LPC web app cannot draw them either. Same for `legs/pants/pregnant`.
- **Children have no shoes, faces, or noses.** Every generated child is barefoot
  and blank-faced; this is the art, not the generator. The hand-made
  `little-sister` character has `feet_shoes_basic` and `face_neutral` selected
  and neither one draws.
- **`hair_messy` and `hair_relm_ponytail` have no child sprites** despite
  claiming to, and **`hair_parted_side_bangs`** has a missing trailing slash in
  its `pregnant` path.

Both unusable bases are one line each in `BODY_TYPES`; add them back when the art
lands and the builder will report whether it worked.

## Worth adding later

Deliberately not enabled, but plausible for this game if you want them:

- **Spectacles.** `facial_glasses_round`, `_nerd`, `_halfmoon` — very fitting for
  an elder or a scribe, left out only because they were not asked for.
- **Working tools in the carried slot.** `tool_hoe`, `tool_shovel`, `tool_axe`,
  `tool_rod` all exist and read as a livelihood rather than a weapon. The game
  already uses the LPC tool overlays for the Scribe.
- **Socks, gloves, cloth wrist cuffs, and the plain `belt_robe`.**
- **`torso_clothes_robe`** — ten baked colours with non-palette names
  (`forest green`, `light gray`), so it needs its own colour mapping.
- **A worn/dirty pass.** Nothing in LPC ages a garment; a shader or a baked
  overlay would do more for "these people walked here" than any extra item.

## How the colour actually works

Two generations of LPC asset are mixed in one catalogue, and the difference
matters at every layer of this:

- **Older pieces** ship one baked PNG per colour, named in `variants`, stored at
  `<path>/<animation>/<colour>.png`. The chosen colour goes in the selection's
  `variant` field.
- **Newer pieces** ship a single sheet at `<path>/<animation>.png` drawn in a
  material's base colour, recoloured at draw time by mapping six hex values onto
  six others. The chosen colour goes in `recolor`.

Heads, faces, noses and wrinkles carry `match_body_color`, so they take the
body's skin tone; eyebrows and beards take the hair colour. That is why the
export sets `matchBodyColorEnabled`.

One rule the catalogue cannot express is enforced in the generator: **a shirt
has to look like a shirt.** LPC shades a torso gently, so a garment within a few
tones of the wearer's skin does not read as brown cloth on a brown body, it
reads as somebody with no shirt on — walnut on brown skin is seven units apart
across the whole ramp. `clothes` and `legs` therefore reject cloth colours too
close to the skin (`TELLS_APART` in `lib.rs`), falling back to the full palette
rather than leaving anyone undressed. It costs the darkest skin tones about a
third of the muted palette, and what it costs them is the browns they were
disappearing into.

`scripts/preview-npcs.py` implements all of this — including `${head}`-templated
paths, where a face lives in a directory named for the head above it. It is
checked against ground truth rather than trusted:

```
python3 scripts/preview-npcs.py --verify assets/custom/old-guy.txt assets/custom/old-guy.png
```

This renders the hand-made character and searches for the result inside the
sheet the web app exported for them. All four of `old-guy`, `redhead-lady`,
`black-teen` and `little-sister` match exactly, pixel for pixel.

## In the game

Travellers are composited at runtime, in the game, from the walk sheets in
`runtime-assets/npc/`. Nothing is baked: a waystation that keeps its fire lit
for two hundred days meets two hundred different strangers.

The chain runs one way and each link is checked against the one before it:

```
scripts/build-npc-wardrobe.py   reads the LPC catalogue, writes wardrobe.json
scripts/build-npc-art.py        copies the walk sheets the wardrobe names
crates/npcgen  Npc::draws()     names sheets and palette swaps, in order
crates/game/src/npc_art.rs      stacks them into one texture
```

`Npc::draws` is the whole contract: an ordered list of sheets with the colours
to replace in each. `npc_art.rs` is the only part that knows what a pixel is,
and `crates/npcgen` still has no idea what an image is.

Only the **walk** rows are copied — nine frames by four facings, 576×256. A
visitor approaches, stands on frame 0 of the south row, and leaves; the other
fifty rows of an LPC action sheet never reach a screen, and copying them would
multiply 4.5 MB by an order of magnitude. `runtime-assets/people/` still holds
the four hand-made sheets, now cropped to the same shape, and a visitor falls
back to one if their art fails to load or was never built.

### Arrivals

`visitors.rs` authors the *shape* of an arrival, not the people in it: someone
alone, two siblings, an old hand. Each body in a shape names a `Cast` —
`Grown`, `Elder`, `Youth`, `Child` — and the traveller standing in it is
generated fresh. Two lone walkers a month apart share nothing but the fact that
they came alone.

What the Scribe reads is drawn from three pools that vary independently:

- the **sighting**, from the profile — what you can make out from the court;
- a **detail** from the generated traveller themself, when there is one worth
  naming: a stick, a hood, a pack;
- the **opening**, from `content/openings.ron` — the first thing they say.

Openings are separate from vignettes on purpose. Three authored stories and
thirty openings meet as ninety different first minutes, and the opening carries
the thing the story cannot: that meeting anyone at all is the unusual event, and
nobody walks up to a stranger's fire pleased about it.

### Checking that it draws the right person

Three implementations composite LPC layers, and they are checked against each
other rather than trusted:

| Check | Where | What it proves |
| --- | --- | --- |
| `preview-npcs.py --verify` | manual | Pillow compositing equals a sheet the LPC **web app** exported. |
| `check_against_catalog` | every `make assets` | The committed wardrobe draws what the **LPC catalogue** draws. |
| `every_reference_traveller_composes_to_the_same_pixels` | `cargo test` | The **game** draws what the reference renderer draws. |

Each is a byte-for-byte comparison, not a tolerance. That is affordable because
`npc_art.rs` reproduces Pillow's `alpha_composite` integer for integer — see the
comment on `over_under`, which explains why matching a specific implementation
is worth the fuss.

The reference cast lives in `runtime-assets/npc/reference/`, written by
`build-npc-art.py`: the first and the last fitting piece in every slot, for
every body type. Not plausible travellers — they all end up in hoods with canes
— but they exercise templated face paths, palette recolouring, baked colour
variants, and two pieces claiming the same `zPos`.

### Licences, in the game

Compositing happens in memory and only in memory. A traveller is a texture for
as long as somebody is standing in the court; they are never written to disk and
never flattened into an image with the purchased tilesets around them, which is
the distinction share-alike actually cares about. See **Screenshots and flat
images** above for the case where that stops being true.

`runtime-assets/npc/` carries `CREDITS.md` beside the art, so attribution
travels with the sheets wherever the runtime tree goes — including into the
demo-assets archive, which packs the whole tree.

## Not done yet

**More stories.** Three vignettes is what the game has, and the opening pool
hides that rather than fixing it. Adding one costs a block in
`content/vignettes.ron` reusing the existing needs, plus its id in a profile's
`vignettes` list — the server routes on the authored text and needs no change.

**One era.** `Era::Dyed` exists, is tested, and nothing turns it on;
`visitors::CURRENT_ERA` is the single line that has to learn about the story
beat where real dye comes back.

**The exported `character.json` carries no `credits` array.** The generator does
not write one because nothing reads it — the attribution is computed in
`crates/npcgen/data/credits.json` instead.
