# Third-Party Asset Boundary

The source code is public for competition review. Purchased pixel-art packs are
not part of the repository and may not be redistributed as source material.

The hosted game is built locally from licensed copies recorded in
`assets-manifest.json`. `scripts/build-assets.py` verifies known source hashes and
produces the runtime directory consumed by the container build. Without those
packs it generates a complete, openly distributable fallback set, so code review,
tests, and reproducible builds do not require a purchase.

Current licensed packs include Modern Farm by LimeZu, The Natural, The Ground,
and Time Fantasy-compatible animals. Full purchase receipts and original archives
remain in the owner's private itch.io library.

The expanded private library is browsed only through the localhost level editor.
Everything under `assets/` is treated as licensed and non-redistributable by
default; `assets/custom/` is the project-curated exception. The current Scribe
sheet there is derived from the OpenGameArt LPC Character Base ecosystem, so its
complete exporter/source attribution and license list must accompany any release
that redistributes the sheet. Authored room JSON may record source paths and crop
rectangles, but the raw sheets and generated catalog remain ignored. The runtime
build receives only flattened room images and the specific state crops used by
authored repair pairs.

Procedurally generated travellers draw on the same LPC ecosystem, through
`crates/npcgen`. That wardrobe is built by `scripts/build-npc-wardrobe.py`, which
refuses any sprite offered only under GPL and any sprite whose author the LPC
catalogue cannot name — Waystation is not open source, so GPL is not a licence it
can pick. The art in use is currently a mix of OGA-BY 3.0, CC-BY-SA 3.0, CC-BY
and CC0; 35 of 179 generated pieces are share-alike with no attribution-only
alternative, which is recorded rather than hidden. Rebuilding with
`--licenses attribution` excludes those.

The same script credits the hand-drawn character sheets in `assets/custom/`, so
generated and authored people land in one attribution list rather than two. Every
832x3456 LPC action sheet there must have its generator export saved beside it as
`.txt`; the build fails otherwise, because art with no provenance record cannot
be attributed. Of the five today, `scribe.png`, `old-guy.png` and
`redhead-lady.png` carry share-alike art and `black-teen.png` and
`little-sister.png` do not.

The runtime build now ships the LPC sprites themselves. `scripts/build-npc-art.py`
copies the walk sheets the wardrobe names — and only those — into
`runtime-assets/npc/`, prunes any that the wardrobe stops naming, and puts a copy
of `docs/NPC_ART_CREDITS.md` beside them as `CREDITS.md`, so attribution travels
with the art wherever the runtime tree goes. Redistributing these sheets is
exactly what their licences permit, provided that credit goes with them.

Share-alike art is safe as animated art inside the running game, and compositing
a traveller happens only in memory: the sheet exists as a texture for as long as
somebody is standing in the court, and is never written to disk. It must not be
flattened into a single image with the purchased packs, which cannot be
relicensed to match — that includes screenshots, trailer frames and store art.
Generate people for those with `make npcs ART=attribution-only`. Full per-file
attribution lives in `docs/NPC_ART_CREDITS.md` and
`crates/npcgen/data/credits.json`, and must accompany any release that ships
these characters.

The runtime bundle also includes two open fonts:

- EB Garamond by the EB Garamond Project Authors, used for player-facing text.
- Noto Emoji by Google, used explicitly for UI action and status symbols.

Both fonts are distributed under the SIL Open Font License 1.1. Their license
texts are stored beside the canonical font files in `open-assets/fonts`; the
asset build verifies and copies them into `runtime-assets/fonts`.

Licensed music and sound sources under `music/` are also excluded from git. The
manifest selects only the two AndriiG background tracks and the Dragon Studio
rain and floorboard effects currently used by the demo. The build copies those
selected files and attribution notes into `runtime-assets/audio`; unused source
audio is never packaged. AndriiG participates in YouTube Content ID, so retain
the provider's license/certificate records when publishing captured gameplay.

For the hosted demo, `make publish-demo-assets` performs a strict private build
and uploads only `runtime-assets/` to the `demo-runtime-assets` Release. The
repository is private, and default-branch CI overlays this archive after its open
fallback build. This keeps purchased source sheets and catalogs out of git while
allowing the licensed game build to be served. Delete that Release before making
the source repository public; recreate an equivalent private delivery boundary
if the project is later open-sourced.

The web shell links to the complete, revision-pinned Universal LPC Generator
credits and license catalog for the Scribe sheet. The deliberately comprehensive
catalog is used because the original per-export selection file was not retained.
Regenerate the Scribe with its selection-specific credits before a final release.
The same applies to the generator's hand-tool overlay layers under
`assets/custom/lpc-tools` — hammer, axe, hoe, shovel, and watering can — which
the build composites onto the Scribe's own slash and thrust rows.

The four visitor sheets are from the same generator and carry the same
obligation: `redhead-lady.png`, `black-teen.png`, `little-sister.png`, and
`old-guy.png`, copied by the build into `runtime-assets/people/` as `walker`,
`elder-sibling`, `younger-sibling`, and `old-hand`. Each has a retained
`.txt` selection file beside it, so unlike the Scribe these can be regenerated
with selection-specific credits rather than the comprehensive catalog. A build
without the private sheets substitutes tinted fallback bodies, so the open
fallback build still tells one visitor from another.

Scripture text is fetched from YouVersion, at runtime for a traveler's passage
and at build time for the print cards and the Gideon Bible readings. All of it
is the Berean Standard Bible, which its translation committee dedicated to the
public domain (CC0) on 30 April 2023; attribution is appreciated and not
required:

> The Holy Bible, Berean Standard Bible, BSB is produced in cooperation with
> Bible Hub, Discovery Bible, OpenBible.com, and the Berean Bible Translation
> Committee.

Development fixtures use the same text and are identified as fixtures in the UI.
Changing translation with `make verses VERSION=…` puts a different version's
wording in the repository, and whether that version permits it is a question the
YouVersion licence for that version answers, not this file.
