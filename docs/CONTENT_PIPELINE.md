# Content and Art Pipeline

Traveler stories and allowed passages live in RON under `content/`. Each traveler
must list at least two human needs, and every need must have a passage candidate.
Shared-crate tests enforce that invariant.

`scripts/build-assets.py` provides the public art build:

- verifies known hashes when purchased sources are present;
- never modifies or republishes purchased source sheets;
- extracts a compact 35-cell terrain atlas from `THE GROUND`, containing only
  grass variants, the 14 supported dual-grid dirt masks, and cell-centered water
  transitions understood by the engine;
- generates nine deterministic 96×64 pixel motifs in the shared card palette;
- emits a contact sheet and machine-readable provenance report;
- flattens authored interior layers from private source-sheet rectangles into
  runtime room images at their native pixel dimensions, with procedural
  stand-ins when private art is absent, applying authored horizontal/vertical
  flips and per-placement pixel snap offsets without resampling;
- extracts reusable repair-pair states as separate native-size runtime
  sprites so repaired structures and fixtures are not baked into room caches;
- verifies and copies bundled open fonts and their license files;
- works without private art for CI, judging, and code review.

When `THE GROUND` is unavailable, the same terrain-atlas slots are filled with
project-authored procedural fallback art. Atlas slot meanings are stable; world
generation and mask selection therefore behave identically in both builds.

Future public-domain imports should add creator, title, date, source URL, public-
domain basis, crop, palette, and output hash to `assets-manifest.json`. Raw imports
belong outside the public repository until their status has been reviewed.

The private asset catalog is deliberately metadata-only. Automatic tags come
from paths; human-reviewed sheet-level tags such as `bed`, `front desk`, and
`wall` live in `meta/asset-tags.json`. Run `make catalog` to inspect the generated
catalog or `make editor` to browse it visually.

Reusable damaged/repaired transitions live in `content/repair-pairs.json`.
Rooms refer to stable pair IDs; source-crop equality has no semantic meaning, so
many pairs may intentionally share either side. The local editor manages this
library, while the build extracts only pairs actually referenced by each room.

The byte-identical Modern Farm terrain sheet can reuse the wang-set metadata and
autotile generator proven in the sibling Ducks project when the placeholder valley
is replaced with final licensed art.
