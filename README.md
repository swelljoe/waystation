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

To start directly in the authored motel room while iterating on interiors, run:

```bash
WAYSTATION_START_INTERIOR=1 cargo run -p waystation-game
```

## Author interiors

The local level editor searches the private art library, selects single- or
multi-cell rectangles from sprite sheets, and paints floor, wall, object, and
overlay layers with the mouse. It also edits collision, entry, and exit cells.

```bash
make editor
```

Open <http://127.0.0.1:7790>, save the room, then run `make assets` to flatten its
private source stamps into gitignored runtime art. For repairable content:

1. Select the damaged crop and choose **Use selection as damaged**.
2. Select its repaired crop and choose **Use selection as repaired**.
3. Choose **Repairable structure** or **Repairable fixture**, enter a reusable
   kind such as `wood-floor` or `mirror`, then stamp instances.
4. Reuse the same kind to stamp more automatically identified instances without
   recapturing its artwork.

Baked scenery goes into the cached room background. Mutable template states are
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
