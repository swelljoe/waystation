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
default; `assets/custom/` is the sole project-authored exception. Authored room
JSON may record source paths and crop rectangles, but the raw sheets and generated
catalog remain ignored. The runtime build receives only flattened room images and
the specific state crops used by authored repair pairs.

The runtime bundle also includes two open fonts:

- EB Garamond by the EB Garamond Project Authors, used for player-facing text.
- Noto Emoji by Google, used explicitly for UI action and status symbols.

Both fonts are distributed under the SIL Open Font License 1.1. Their license
texts are stored beside the canonical font files in `open-assets/fonts`; the
asset build verifies and copies them into `runtime-assets/fonts`.

Scripture text is fetched at runtime from YouVersion. The default development
fixtures use short passages from the public-domain Berean Standard Bible and are
identified as fixtures in the UI.
