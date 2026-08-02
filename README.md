# The Waystation at the Edge of the Ash

A cozy, far-future game about restoration, literacy, hospitality, and a book that survived.

![Screenshot of the folio display showing 2 Timothy 4:16](/static/folio-display.png)

You play the scribe, a solitary traveler who finds an ancient stone motel in a
protected valley. Restore its hearth and writing desk, welcome strangers who follow
the smoke, listen to what they carry, and feed and shelter them, and give them
a gift of an illustration and verse.

This repository is the working submission for YouVersion and Gloo AI's **Scripture
in New Frontiers** challenge. It contains a playable early version of the game, 
a secure API service, deterministic content/art pipelines, and public development
fixtures.

## Play locally

NOTE: Much of the game are is currently licensed from creators on itch.io and not
distributable. Placeholder art is provided, but it's just boxes and AI-generated
junk. Easier to play online at https://swelljoe.github.io/waystation/

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
- E: inspect, search, gather, or welcome
- R: restore, including cleaning, clearing, and repair work
- Space: advance quiet story moments and dialogue; begin another traveler after the ending
- 1–3: choose paper, illumination, and border
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

Room 3 is unusually well preserved. Its baked nightstand carries a named search
hotspot; searching it reveals the old Gideon Bible and persists that discovery
under the room object's stable ID. The find opens a dismissible item card with
the native-pixel Bible icon and the Scribe's first impression; use E, Space, or
Escape to continue. The Scribe leaves the book safely in room 3 rather than
putting it in the backpack, so the nightstand remains searchable.

The same parchment card now carries one-time story thoughts without pretending
they are inventory. Entering and exploring the office reveals its unnumbered
entrance, hearth and chairs, guest-ledger scraps, and numbered keys in stages;
only after all three clues does the Scribe recognize a place built to welcome
strangers. Entering room 3 for the first time also pauses on its remarkable
preservation. Seen thoughts persist with the room state, while keys and the book
appear under **Known** as access or knowledge rather than carried supplies.

The valley is now a 144×96-cell exploration area with persistent kindling,
fallen-log, and plank pickups. An authored tool shed behind the motel contains
portable tools whose condition and location persist. The upper-right restoration
ledger shows skills, carried tools, and consumable supplies. The objective, ledger, and
contextual controls use compact aged-paper panels so they remain legible over
the world. Clean debris to develop Upkeep; that unlocks Carpentry and Masonry,
while Carpentry eventually unlocks Roofing.
The office desk supplies keys and nails; the shed holds a hammer, axe, shovel,
and broken pickaxe. The office hearth now
requires three kindling plus a cleared chimney reached with the discoverable
ladder. See [docs/RESTORATION_GAMEPLAY.md](docs/RESTORATION_GAMEPLAY.md) for tool
controls and scene/task schemas.

The Scribe uses the complete 13×54 LPC action sheet from
`assets/custom/scribe.png`. Movement currently animates the four nine-frame walk
rows at native pixel scale. Hammer repairs and axe chopping now compose the LPC
six-frame work layers; the remaining action rows stay available for farming,
sitting, climbing, expression, and combat systems. The complete matrix is in
[docs/LPC_ACTIONS.md](docs/LPC_ACTIONS.md).

Travelers who arrive are generated rather than drawn, and composited in the
running game rather than baked: humans only, in scavenged browns and tans, no
armor and no weapon but the occasional cane. A waystation that keeps its fire
lit for two hundred days meets two hundred different strangers. `make npcs`
writes a reviewable cast as loadable `character.json` files, a page of links
into the web generator, and a contact sheet; pruning happens in one allowlist.
The three compositors involved — the LPC web app, the reference renderer, and
the game — are checked against each other byte for byte. See
[docs/NPC_GENERATOR.md](docs/NPC_GENERATOR.md).

What a stranger says first is drawn separately from the story they came to
tell, so three authored vignettes and thirty openings in
`content/openings.ron` meet as ninety different first minutes.

Exterior trees depth-sort at their trunk ground contact. When the Scribe passes
behind the dense leafy canopy, the character is fully hidden rather than leaving
a stray head pixel above the opaque foliage. Full concealment samples the PNG's
actual alpha silhouette, so partial overlaps and the rounded transparent outer
area still composite naturally. Walking in front draws the complete Scribe over
the trunk and lower branches.

## Author interiors and buildings

![Screenshot of the building/room editor showing the motel office](/static/editor.png)

The local scene editor searches the private art library, selects single- or
multi-cell rectangles from sprite sheets, and paints floor, wall, object, and
overlay layers with the mouse. It also edits collision, entry, and exit cells.
Hold the mouse button to paint continuously. Stamp strokes advance by the full
native-art footprint rounded up to the current snap interval, preventing
differently sized stamps from overlapping one another; collision strokes advance
by the independently adjustable **Collision pen** size. One undo reverts the
complete stroke. Existing logical-cell collision remains readable, while new
strokes store their own 1–256 pixel grid so coarse and fine boundaries can coexist.
Using a finer pen as an eraser carves that square out of a compatible coarse
area, automatically subdividing the remainder instead of deleting the whole cell.

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
control never moves existing art. Collision, entry, and exit editing remain on
their own controls: **Collision pen** selects a native-pixel square brush, while
entry and exit markers remain on the room's logical gameplay grid. The pointer
preview shows the exact collision square that will be added or removed.

Choose **Building exterior** to author a transparent building canvas with the
same native-pixel stamps, layers, repair pairs, transforms, collision, and undo
behavior. Building documents save under `content/buildings`; generated caches
and state sprites go under `runtime-assets/buildings`. The browser initially
filters to `assets/components`, but **All packs** remains available.

In the exterior game view, a building's southernmost collision edge is also its
ground-contact depth line. The complete building—including its baked floor,
wall, object, and overlay caches and all repairable components—passes in front
of the Scribe when the Scribe walks north/behind that line, while preserving its
authored internal layer order. A cropped 16-pixel crown from the current Scribe
animation remains visible above the façade so the player is never lost; authored
components marked **Fully hides player** still completely occlude that crown
when they overlap it. The motel office gable uses this setting, while chimney
repair pairs receive the same behavior from their semantic kind. In Select mode,
toggle it from the selected-placement toolbar for other baked or repairable
building components. Keep
collision only where the Scribe's feet should actually be blocked: roof and
chimney overhangs can extend beyond it without changing the building's art.

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
   the repaired crop and choose **Use selection as repaired**. For removed junk,
   choose **Completed state is invisible** instead of capturing a blank tile.
3. Set the task action, skill level, durable tools, consumed supplies, and XP.
   Save the pair; it is now searchable and reusable in every room and building.
4. Select a saved pair, choose **Repairable structure** or **Repairable
   fixture**, then stamp automatically identified instances.

Pair identity is independent of its two crops. Multiple damaged variants may
share one repaired crop, and multiple repair outcomes may share one damaged crop.
Use **Duplicate** to retain both crops under a new identity, then replace only the
side that differs. The library is stored in `content/repair-pairs.json`.

Use **Flip H** and **Flip V** before stamping to mirror baked scenery or a
repairable instance without creating another source asset. Repair-pair instances
apply one stored orientation to both damaged and repaired states.

Baked scenery goes into separate cached images for the floor, wall, object, and
overlay layers. Repair-pair states are extracted as separate native-size sprites
and interleaved with those caches according to the same layer order.
Approach an authored item and press `E`; the game reports missing skills, tools,
or supplies before changing the sprite and persisting `room-id/instance-id`.

Asset tags live in `meta/asset-tags.json`; expand that sidecar as packs are
reviewed. The editor binds only to localhost because it serves licensed source
sheets.

## Run with Podman

```bash
make container
make run-container
```

Open <http://127.0.0.1:7777>. The image defaults to reviewed fixture mode. Live
mode needs the Gloo pair; `YVP_APP_KEY` is optional and adds only the wording:

```bash
podman run --rm -p 7777:7777 \
  -e API_MODE=live \
  -e GLOO_CLIENT_ID \
  -e GLOO_CLIENT_SECRET \
  -e YVP_APP_KEY \
  waystation:latest
```

Locally, `make server-live` reads the same values from a gitignored `.env`.
Never place credentials in the browser build, source tree, or container image.

## How the Scripture loop works

1. The client sends only an authored `vignette_id` to the same-origin server.
2. Gloo Completions V2 receives the reviewed vignette and must call a structured
   `select_remembrance` tool. Auto-routing picks the model per request, and the
   one it picked is reported back in the response provenance.
3. The server validates the selected need and passage against `content/*.ron`.
   A selection outside the authored pairs is refused, not served.
4. YouVersion returns the passage text for the selected ID, in the player's
   language. Without a key the reviewed wording in `content/passages.ron` is
   served instead, marked `reviewed_local`, and the card credits the reviewed
   text rather than YouVersion.
5. The player creates a card from the passage and project-authored pixel motifs.

## Translations

The traveler's passage arrives in the player's own language. The browser build
sends `navigator.language`; the desktop build sends `LANG`. Nothing is asked of
the player, and an unrecognised tag costs a translation rather than a passage.

A tag is matched whole first, then with one trailing subtag dropped at a time.
`zh-Hant-TW` finds the Traditional Chinese edition the catalog lists under that
exact tag, while `pt-BR` — a distinction the catalog does not make — falls
through to Portuguese.

`content/bible-versions.json` decides which translation each language gets. It
is committed and reviewable — the server never discovers versions at runtime —
and `make bible-versions` rebuilds it from the YouVersion catalog with
`YVP_APP_KEY` set. A version is only eligible if it carries every book
`content/passages.ron` draws from, which is what keeps a New Testament edition
from being chosen for a Psalm. Of 356 versions in the catalog, 92 clear that bar,
covering **64 languages**. To change a pick, move an entry out of its
`alternatives` list; a rebuild keeps the choice.

English is pinned to BSB because the reviewed wording in `content/passages.ron`
is BSB, and that wording is what a player sees whenever the live path is
unavailable. It is labelled English even when another language was requested:
visible English beats English under a Spanish name.

Only Scripture is translated. The vignettes, the reflections, and the rest of
the game are still English.

Gloo's content controls answer a declined vignette with `200 OK` and prose in
place of the tool call. The server names that case in its logs so an edited
vignette that trips the filter is not mistaken for a network fault. Either way
the traveler still gets the reviewed fallback.

The first twelve wordless block-print illustrations and their exact card
overlays are cataloged in `content/prints.json`. The verse text is BSB, fetched
from YouVersion by `make verses`; the illustrations carry no words by design, so
changing translation is a recomposite and never a regeneration. Run `make
prints` after changing a verse or its source art. The deterministic compositor renders EB Garamond at low
resolution with intentionally large type, giving exact Roman serif text an
appropriately crude apprentice-print scale without enlarging the card. See
[`docs/PRINT_CARDS.md`](docs/PRINT_CARDS.md) for the collection and prompt set.
New catalog entries can be generated sequentially with `make print-art`. The
resumable Codex batch skips existing illustrations and composes the finished
cards after verifying each new portrait PNG.
Use `make add-print` to append a reviewed verse and its wordless illustration
brief without editing JSON by hand.

`P` opens the folio at any hour and anywhere: every block ever cut, one leaf at
a time, arrow keys to turn them. Prints already carried off by a traveler stay
in it and are marked as gone. Cards are drawn at their composed 2:3 shape both
here and on the visit screen, so a whole illustration is visible rather than a
card squeezed into a letterbox slot.

If a live dependency is unavailable, the server uses a disclosed cache or reviewed
fixture. The UI always reports provenance; it never presents fixture text as a live
API response.

## Project layout

```text
crates/game/       Bevy native/WebAssembly client
crates/npcgen/     Procedural LPC travelers, as generator selections
crates/server/     Axum static host and secret-bearing API proxy
crates/shared/     DTOs, validation, and reviewed content loader
content/           Authored traveler, opening and passage catalogs (RON)
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
make web       # requires Trunk, wasm32-unknown-unknown, and Python Pillow
```

`make web` writes a self-contained browser build to `dist/`. CI uploads that
directory as the downloadable `waystation-web` artifact on every run and deploys
default-branch builds to the repository's configured GitHub Pages site at
<https://swelljoe.github.io/waystation/>. Trunk uses relative URLs, so the same
build works at a Pages repository path and from a local static server. The web
shell waits for the player to choose **Enter the Waystation** before starting
Bevy, satisfying browser audio-autoplay policy while keeping music and rain
enabled from the first game frame.

Because `assets/` is intentionally gitignored, pull-request runners build the
distributable procedural fallback art. Default-branch builds overlay the
flattened, runtime-only bundle held by the private `demo-runtime-assets` Release;
they never receive the purchased source sheets or raw licensed audio. The same
boundary selects only currently used files from ignored `music/` and places them
under `runtime-assets/audio`. After changing licensed art or audio, run
`make publish-demo-assets` from the authoring machine to rebuild and replace that
bundle. Delete the private Release before ever making the repository public.

The code is public for competition review but remains all rights reserved unless
the project wins. If selected, the submitted source will be relicensed under MIT
OR Apache-2.0 as required by the competition rules. Third-party assets remain
under their original licenses.
