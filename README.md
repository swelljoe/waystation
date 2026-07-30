# The Waystation at the Edge of the Ash

A cozy, far-future game about literacy, hospitality, and a book that survived.

You play The Scribe, a solitary traveler who finds an ancient stone motel in a
protected valley. Restore its hearth and writing desk, welcome someone who follows
the smoke, listen to what they carry, and make an illuminated remembrance from a
passage of Scripture.

This repository is the working submission for YouVersion and Gloo AI's **Scripture
in New Frontiers** challenge. It contains a complete 10–15 minute vertical slice,
a secure API service, deterministic content/art pipelines, and public development
fixtures.

## Play locally

Prerequisites: Rust 1.92+, Python 3 with Pillow, and the usual Bevy Linux graphics
libraries. Two terminals are used for the native build:

```bash
make assets
make server
```

```bash
make game
```

The fixture server listens on `http://127.0.0.1:7777`. Controls are:

- WASD or arrow keys: move
- E: inspect, gather, repair, or welcome
- Space: advance quiet story moments and dialogue
- 1–3: choose paper, illumination, and border
- R: replay with the next authored traveler after the ending
- F3: toggle logical-cell, dual-grid mask, and atlas identifiers

The development client currently uses a fixed 2× presentation scale for both
world rendering and UI. This is temporary until dynamic display scaling is added.

To start directly in the authored motel office while iterating on interiors, run:

```bash
WAYSTATION_START_INTERIOR=1 cargo run -p waystation-game
```

The valley motel now uses `content/buildings/motel-exterior.json` and the office
plus rooms 1–6 use their matching documents under `content/interiors`. Exterior
doors route left-to-right to the office and numbered rooms. The office and room
5 begin unlocked; searching the authored office desk finds the keys for the
remaining rooms. Every room returns the Scribe to the same exterior doorstep it
was entered from.

The Scribe uses the complete 13×54 LPC action sheet from
`assets/custom/scribe.png`. Movement currently animates the four nine-frame walk
rows at native pixel scale; the remaining action rows stay in the runtime atlas
for future tool, farming, sitting, climbing, expression, and combat systems.

## Author interiors and buildings

The local scene editor searches the private art library, selects single- or
multi-cell rectangles from sprite sheets, and paints floor, wall, object, and
overlay layers with the mouse. It also edits collision, entry, and exit cells.
Hold the mouse button to paint continuously. Stamp strokes advance by the full
native-art footprint rounded up to the current snap interval, preventing
differently sized stamps from overlapping one another; collision strokes advance
one gameplay cell at a time. One undo reverts the complete stroke.

Use **Grid: shown/hidden** to hide the destination grid for a clean scene
preview. This display-only toggle does not change snap behavior or saved scene
data; collision visibility remains independently controlled.

While the stamp tool is active, the exact native-pixel crop follows the pointer
at its snapped destination before placement. The ghost uses the chosen layer,
horizontal/vertical flips, smart-slice transparency, and the damaged state of a
repair pair; its dashed bounds and top-left marker make sparse shapes easier to
align.

Use **Select** to target the topmost placed item. The editor outlines the
selection; drag it to reposition it on the grid it was originally stamped with,
use the arrow keys for one-grid nudges, or use **Flip H** and **Flip V** to edit
its stored orientation. While an item is selected, the existing **Layer** menu
shows its current layer and moves it to a different layer. These placement
changes are undoable and save normally. Outside Select mode, the menu continues
to set the layer for new stamps.

### Layer order

Layers render in this fixed bottom-to-top order:

| Order | Layer | Intended use |
|---:|---|---|
| 1 (bottom) | **Floor** | Flooring, ground, and marks beneath the room |
| 2 | **Wall** | Walls, roofs, windows, and structural surfaces |
| 3 | **Object** | Furniture, fixtures, doors, and most interactable items |
| 4 (top) | **Overlay** | Foreground trim, shadows, cracks, vines, and effects that must cover other art |

Changing one repairable instance's layer creates an override on that placement;
it does not alter the repair pair or move every other instance that uses it.

**Repair view** can render the complete scene in its authored state, with every
repairable item damaged, or with every item repaired. A selected repairable item
can instead follow the scene view or preview only its damaged/repaired state from
the **Selected placement** inspector in the toolbar. Repair views are
inspection-only and never rewrite the scene's saved `initial_state` values.

The right-hand source palette is intentionally wide so large sprite sheets have
more screen space for precise crop selection. Its preview expands to the palette
width while preserving the sheet's aspect ratio and native source coordinates;
only the display size changes.

**Snap grid** controls destination placement independently of **Source grid**.
Set it to `16` for half-cell art offsets, or any 1–256 pixel interval needed by a
pack. Each placement remembers the grid used when it was stamped, so changing the
control never moves existing art. Collision, entry, and exit editing remains on
the room's logical gameplay grid.

Choose **Building exterior** to author a transparent building canvas with the
same native-pixel stamps, layers, repair pairs, transforms, collision, and undo
behavior. Building documents save under `content/buildings`; generated caches
and state sprites go under `runtime-assets/buildings`. The browser initially
filters to `assets/components`, but **All packs** remains available.

For sheets whose pieces are separated by empty space, choose **Smart slice** and
click a detected outline. Transparent sheets use their alpha channel. Opaque
sheets with a dominant solid background, including the damaged village-house
sheet, store a nondestructive background key so the gaps remain transparent in
editor previews and generated art. Smart selections are exact pixel rectangles;
manual source-grid selection remains available.

If a source PNG is edited while the editor is running, select it and choose
**Refresh sheet**. The editor cache-busts only that file and refreshes the source
canvas, asset thumbnail, room/building canvas, and repair-pair previews. Existing
crop coordinates are retained if they still fit; Smart slice must be run again
because the foreground regions may have changed.

After adding, renaming, or removing image files under `assets`, choose **Refresh
assets** in the Asset browser. The running editor rescans the private library and
updates search results, pack filters, dimensions, and cached image previews
without a server or browser restart. The current search, valid pack filter, and
selected sheet are preserved when possible.

```bash
make editor
```

Open <http://127.0.0.1:7790>, save the room, then run `make assets` to flatten its
private source stamps into gitignored runtime art. For repairable content:

1. In the **Repair-pair library**, choose **New pair** and give the transition a
   stable ID, label, kind, and render layer.
2. Select the damaged crop and choose **Use selection as damaged**, then select
   the repaired crop and choose **Use selection as repaired**.
3. Save the pair. It is now searchable and reusable in every room and building.
4. Select a saved pair, choose **Repairable structure** or **Repairable
   fixture**, then stamp automatically identified instances.

Pair identity is independent of its two crops. Multiple damaged variants may
share one repaired crop, and multiple repair outcomes may share one damaged crop.
Use **Duplicate** to retain both crops under a new identity, then replace only the
side that differs. The library is stored in `content/repair-pairs.json`.

Use **Flip H** and **Flip V** before stamping to mirror baked scenery or a
repairable instance without creating another source asset. Repair-pair instances
apply one stored orientation to both damaged and repaired states.

Baked scenery goes into the cached room background. Repair-pair states are
extracted as separate native-size sprites and are never baked into that image.
In the current slice, approach the cracked motel-room mirror and press `E` to
repair it. Browser saves persist the state under `room-id/instance-id`.

Asset tags live in `meta/asset-tags.json`; expand that sidecar as packs are
reviewed. The editor binds only to localhost because it serves licensed source
sheets.

## Run with Podman

```bash
make container
make run-container
```

Open <http://127.0.0.1:7777>. The image defaults to reviewed fixture mode. For a
live deployment, provide all three secrets at runtime:

```bash
podman run --rm -p 7777:7777 \
  -e API_MODE=live \
  -e GLOO_CLIENT_ID \
  -e GLOO_CLIENT_SECRET \
  -e YVP_APP_KEY \
  waystation:latest
```

Never place credentials in the browser build, source tree, or container image.

## How the Scripture loop works

1. The client sends only an authored `vignette_id` to the same-origin server.
2. Gloo Completions V2 receives the reviewed vignette and must call a structured
   `select_remembrance` tool.
3. The server validates the selected need and passage against `content/*.ron`.
4. YouVersion returns the authoritative passage text for the selected ID.
5. The player creates a card from the passage and project-authored pixel motifs.

If a live dependency is unavailable, the server uses a disclosed cache or reviewed
fixture. The UI always reports provenance; it never presents fixture text as a live
API response.

## Project layout

```text
crates/game/       Bevy native/WebAssembly client
crates/server/     Axum static host and secret-bearing API proxy
crates/shared/     DTOs, validation, and reviewed content loader
content/           Authored traveler and passage catalogs (RON)
scripts/           Deterministic asset tooling
web/               Trunk HTML shell
notebooks/         Kaggle technical notebook
docs/              Architecture, content, and submission runbooks
```

Purchased itch.io art is intentionally excluded. See
[`THIRD_PARTY_ASSETS.md`](THIRD_PARTY_ASSETS.md) and `assets-manifest.json` for the
reproducible boundary. The checked-in pipeline generates a complete open fallback.

## Quality gates

```bash
make test
make analyze
make web       # requires Trunk and wasm32-unknown-unknown
```

The code is public for competition review but remains all rights reserved unless
the project wins. If selected, the submitted source will be relicensed under MIT
OR Apache-2.0 as required by the competition rules. Third-party assets remain
under their original licenses.
