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

Scripture text is fetched at runtime from YouVersion. The default development
fixtures use short passages from the public-domain Berean Standard Bible and are
identified as fixtures in the UI.
