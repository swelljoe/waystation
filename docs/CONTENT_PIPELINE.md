# Content and Art Pipeline

Traveler stories and allowed passages live in RON under `content/`. Each traveler
must list at least two human needs, and every need must have a passage candidate.
Shared-crate tests enforce that invariant.

`scripts/build-assets.py` provides the public art build:

- verifies known hashes when purchased sources are present;
- never modifies or republishes purchased source sheets;
- generates nine deterministic 96×64 pixel motifs in the shared card palette;
- emits a contact sheet and machine-readable provenance report;
- works without private art for CI, judging, and code review.

Future public-domain imports should add creator, title, date, source URL, public-
domain basis, crop, palette, and output hash to `assets-manifest.json`. Raw imports
belong outside the public repository until their status has been reviewed.

The byte-identical Modern Farm terrain sheet can reuse the wang-set metadata and
autotile generator proven in the sibling Ducks project when the placeholder valley
is replaced with final licensed art.

