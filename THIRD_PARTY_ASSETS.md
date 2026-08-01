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

Scripture text is fetched at runtime from YouVersion. The default development
fixtures use short passages from the public-domain Berean Standard Bible and are
identified as fixtures in the UI.
