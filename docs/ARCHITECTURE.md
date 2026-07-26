# Architecture

## Trust boundary

The Bevy client is untrusted and contains no API credentials. It sends a known
traveler ID to the Axum service over the same origin. The server owns authored
content, OAuth state, validation, retries, caching, and YouVersion attribution.

```text
Bevy WASM ── POST /api/interpret { vignette_id }
                         │
                         ├─ Gloo OAuth2 token cache
                         ├─ Gloo Completions V2 + required tool call
                         ├─ reviewed need/passage allowlist validation
                         └─ YouVersion passage lookup
                                      │
Bevy card UI ◀── InterpretResponse + provenance
```

The client cannot inject dialogue or arbitrary passage IDs. Gloo may choose only
from candidates paired with that vignette, and invalid structured output falls
back to a reviewed result rather than reaching YouVersion.

## API contract

`POST /api/interpret`

```json
{"vignette_id":"mara_grief"}
```

The response contains `need_id`, a player-facing need label, the Scribe's short
reflection, exact passage content/reference/version/deep-link, and provenance for
the Gloo route and Scripture source. `GET /api/health` reports configuration booleans
but never credential values.

## Failure behavior

- Live configuration is fail-fast at server startup if a credential is missing.
- HTTP calls have a twelve-second overall timeout.
- Gloo output is schema constrained and then independently allowlist validated.
- A successful live result is retained in an in-memory per-vignette cache.
- On dependency failure the service uses that cache, then a reviewed fixture.
- Every fallback remains explicit in `provenance.scripture_source` and the UI.

## Builds

The multi-stage `Containerfile` creates procedural art, compiles the Bevy client to
WebAssembly with Trunk, compiles the native Axum service, and copies only runtime
artifacts into an unprivileged Debian image. Port 7777 is the project default.

## World terrain

`crates/game/src/terrain.rs` owns the seeded logical `WorldGrid`. Every cell is a
semantic `Grass`, `Dirt`, or `Water` value; image coordinates never enter world
generation. Rendering derives compact-atlas selections from those semantic values.

Grass and dirt use a dual grid: each rendered sprite is centered on the shared
intersection of four logical cells and selected from their four-bit dirt mask.
The rendered layer is offset half a tile from logical cell centers. The asset set
provides all masks except the two ambiguous diagonal checkerboards, which world
generation removes deterministically. Water remains a cell-centered,
eight-neighbor overlay.

The logical grid remains a Bevy resource after startup so navigation, collision,
spawning, and future chunk streaming can query the same terrain source of truth.
Water generation enforces a grass shore because the current art does not define a
direct water-to-dirt transition.

The F3 terrain-debug overlay labels both logical cells and nearby rendered
dual-grid intersections. Its world, render-mask, runtime-atlas, and private-source
coordinate notation is documented in `docs/TERRAIN_DEBUG.md`.

During terrain development the client uses a fixed 2× presentation scale: the 2D
camera projection renders half the former world extent, and Bevy's `UiScale`
doubles fixed UI measurements. Camera edge clamping uses the scaled viewport.
This constant is intentionally centralized pending dynamic display scaling.

## Authored interiors

Interior source data lives in `content/interiors`. Schema v2 separates reusable
mutable templates, automatically identified structural/fixture instances, and
legacy baked placements. Templates own state-specific private source crops;
instances own room-cell anchors and initial state. Source pixels are always
composited at native size, and the source selection grid never changes scale.
Collisions, entry cells, and exits remain independent of pixels.

`scripts/build-assets.py` turns baked placements into a cached room background
and extracts each mutable template state as a separate runtime sprite. The Bevy
client spawns mutable instances over the cache, records changes under stable
`room-id/instance-id` keys, and includes those values in browser save data. The
runtime never needs access to the private library.

`scripts/level_editor.py` serves a localhost-only browser editor. Its asset
catalog is generated from filenames, image dimensions, and the distributable
sidecar tags in `meta/asset-tags.json`. The editor supports multi-cell stamps for
48-pixel RPG sheets, ordered layers, collision painting, entry/exit placement,
undo, direct JSON save/load, reusable damaged/repaired templates, and repairable
structure or fixture stamping.

The Bevy client places interior maps in a separate world-space island. Door
interaction moves the player between exterior and interior spawn cells; camera
scale changes to frame the room against a near-black brown backdrop. Interior
movement checks the authored collision grid.
