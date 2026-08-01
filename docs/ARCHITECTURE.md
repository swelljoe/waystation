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

Licensed audio uses the same private runtime-only delivery boundary as purchased
art. The manifest selects individual ignored source files and the asset build
copies only those sounds into `runtime-assets/audio`; open CI builds omit them,
while strict demo builds require them.

## Looking at the game

The native binary opens a real window, and on a Wayland session no screenshot
tool can reach it — `import -window root` and `ffmpeg -f x11grab` both come back
black. The WebAssembly build has no such problem, because its frames come back
through the debugging protocol rather than the compositor, so the web bundle is
how the game gets *looked at* as well as exercised.

`make web-smoke` serves `dist`, runs it in headless Chromium with ANGLE pointed
at SwiftShader (Bevy needs a real WebGL2 context and there is no GPU), clicks the
page's own start button so the wasm entry point and the audio gesture take a
player's path, optionally walks the Scribe with dispatched key events, and writes
PNGs. It also collects `Log.entryAdded` and `Runtime.exceptionThrown` and exits
non-zero on anything left after filtering the two complaints that are normal for
this build — Bevy probing for `.meta` files it does not ship, and a browser
refusing to start an `AudioContext` before the title screen is clicked. That
filter is the whole reason the runner can fail on console output, so it has its
own tests in `scripts/test_web_smoke.py`.

`WALK=--walk d:2.2 s:0.35` passes `key:seconds` steps through. The browser window
size decides how much world is in frame rather than how large it is drawn: the
camera scale is fixed, so a wider window shows more valley.

This catches placement mistakes that unit tests cannot be written for in advance.
The parking bays passed every geometric assertion while sitting squarely on top
of the motel sign's placeholder art, and only a screenshot showed it.

## Audio

`crates/game/src/game_audio.rs` owns three independent concerns: alternating
background music, deterministic wet/dry weather ambience, and surface-triggered
effects. Rain is guaranteed during the opening six minutes and then fades between
wet and dry phases. Its sink follows location with a smooth exterior-to-interior
crossfade. Bevy's built-in sink does not expose per-source EQ, so the private
build derives a compact 900 Hz low-pass variant with FFmpeg rather than adding a
custom runtime backend.

Broken floorboards reuse authored mutable scene state. A spawn marker identifies
the relevant repair-pair placements, their sprite rectangle defines the trigger,
and `state == "damaged"` enables a rotating creak set. Repairing the existing
entity therefore disables its surface sound without a second persistence path.

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

## Authored interiors and buildings

Interior source data lives in `content/interiors`. Schema v5 combines globally
reusable repair pairs, automatically identified structural/fixture instances,
legacy baked placements, per-placement pixel snap positions, and optional
persistent interaction rectangles over baked scenery. Its optional `items`
collection gives portable tools stable identity, type, initial condition, layer,
position, transform, and a private native-pixel crop. Item crops are extracted
separately under `runtime-assets/items`; they never enter a baked layer cache.
`content/repair-pairs.json` owns every pair's stable
identity, semantics, render layer, and damaged/repaired private source crops;
room instances reference the pair ID and own room-cell anchors plus initial
state. An instance may override its pair's default render layer without changing
other uses of the pair. Either crop may be shared by any number of independently identified
pairs. Source pixels are always composited at native size, and the source
selection grid never changes scale. Collisions, entry cells, and exits remain
independent of pixels. Schema-v2 room-local templates remain readable for
backward compatibility.

Room 3 uses an interaction rectangle over its preserved nightstand rather than
making that furniture a repair pair. Finding the Gideon Bible records
`motel-room-03/bible-nightstand = found` in the same stable scene-state map used
by mutable instances. The Bible remains in the room and can be revisited; the
state records knowledge, not possession.

One-time narrative observations use the same saved map under stable `story/*`
keys. A queued narrative-card resource presents either an illustrated discovery
or an unillustrated thought and blocks movement until dismissed. The office's
final hospitality realization is gated on the entrance, hearth, and ledger
observations rather than story-stage order.

Building source data lives in `content/buildings` and uses the same schema-v5
placement and mutable-instance model with `scene_type: "building"`. Unlike a
room, a building's four baked layer caches have transparent canvases and it has
no entry/exit requirement. The asset pipeline writes those caches and referenced
repair-state sprites to `runtime-assets/buildings`. This makes the static and
mutable split identical on both sides of a motel door.

The tool shed uses that same model for both its exterior building and interior.
Portable tool state lives in `Progression`, keyed by the authored item ID, and
stores condition plus `home`, `carried`, `dropped`, or future `held_by` location.
An immutable runtime catalog retains home scene/position/art metadata. The sync
system shows an item only when its saved location belongs to the active scene.
Save version 6 therefore moves tools between scenes without mutating authored
JSON or flattened art.

## The restoration economy

A `TaskSpec` states what a job asks for (skill, level, tools, supplies) and what
it gives back (`yields`). Yields are what keep the valley circulating rather than
draining: cleared debris returns nails, a chopped tree returns logs and kindling,
and two standing stations convert raw material into the two currencies every
repair is priced in. The sawbuck in the motel court takes a hatchet and one log
and returns two planks; the outcrops along the valley rim take a pickaxe and
return stone. Both are worked in place instead of collected, so their entities
carry a task rather than a pickup reward, and the outcrop's `collected_pickups`
entry is what keeps it quarried across a save.

Conversion teaches nothing — milling and quarrying award no experience — so a
skill level still measures restoration done rather than material gathered. The
gates are deliberate and each one uses a mechanism that already existed: milling
is Carpentry work, so it waits on the Upkeep 1 unlock; quarrying needs the shed's
pickaxe, which starts broken, so masonry begins with a tool repair. Masonry lays
stone with a shovel because no trowel art exists in any licensed pack; `Trowel`
remains in `ToolId` for whenever one does.

Because a supply may now arrive from more than one source, opening-stage progress
reads the supply itself rather than any single act that produces it: kindling
gathered from the valley floor and kindling split off a felled tree advance the
hearth identically, and a save written before the second source existed is
reconciled on load. The hearth is likewise gated on world state — a cleared flue
and a full pile — instead of on story stage, and it reports whichever half is
missing both in the nearby prompt and on a refused interaction. Any interaction
that can decline needs its own reason; the generic fallback line is for scenery
with no use yet, never for a real action.

## The garden

Every other restoration is repair: something existed, it broke, the Scribe puts
it back. The garden is the only loop that makes something the valley did not
have, so it is modelled apart from the repair pairs, in `crates/game/src/garden.rs`.

The beds are the motel's own parking bays — nine of them, six for the rooms and
three for the office, the way these places were always built. Under the asphalt
is soil from before the ash, the only ground in the valley that will carry more
than straggly weeds, and getting at it means levering the slabs up with the pick.

The lot is authored, not compiled: `content/buildings/motel-parking.json` is an
ordinary building scene, so the editor lists it and lays it out with no editor
changes at all. Each bay is a mutable instance of a `parking-bay` repair pair,
which is already the engine's word for "this thing has two states and a job that
turns one into the other" — cracked asphalt, and the pack's own torn-out bay with
the kerb still framing bare ground. Three pair variants cycle across the run so
it does not visibly tile. The pair carries the task too, so what levering a slab
costs is editable; `TaskSpec::for_breaking_ground` still exists because the
economy-coverage tests reason about it, and a test holds the two in step.

Content therefore owns the two states the editor can preview, and the garden owns
the four it cannot. To keep them from reading as different objects, the asset
build lifts the kerb off the torn-out crop and composites it onto every worked
state, so a bed keeps the outline of the space it used to be right through
harvest.

Bays are the only interactable drawn as flat ground — a fixed depth between the
terrain and every prop, and no collision — so the Scribe walks over them rather
than around them. They skip `spawn_building` entirely for that reason: no layer
caches, no depth sorting, no collision. Because the layout is now editable, a
test reads the authored scene and asserts nine bays on one line with no gaps, on
land, out of the building, and clear of the motel sign and the sawbuck; an editor
can put a bay in a wall, and that is what catches it.

A bed is a small state machine — cracked bay, broken ground, tilled rows, sown
seed, standing grain, harvest — and each state owns exactly one `TaskSpec`
describing what it is waiting for. Nothing about the sequence lives in the
engine: the entity carries only a stable plot id, and its sprite, its prompt,
and the work it will accept are all read back out of the saved `Garden`. Because
harvested ground returns to broken rather than to paved, a slab comes up once and
every later season starts from open soil.

Growing is the one step the Scribe cannot do. Its clock runs in `grow_garden`,
which ticks through `bypass_change_detection` so a two-minute season does not
write a save every frame; only ripening marks the resource changed, and that is
also the moment the world interrupts the player to say so. `Cultivation` sits
behind the same Upkeep 1 unlock as Carpentry and Masonry.

## What the valley does not give

Nothing manufactured lies about waiting to be useful. There is exactly one sack
of seed grain in the game, on a tool-shed shelf, authored as a second
`SceneDiscovery` beside the Gideon Bible; every seed after that has to be grown,
traded for, or given. A sowing costs one seed and a harvest returns two, so the
one sack is enough to open the garden and the garden is then the only thing that
widens it.

The water is caught, not fetched, and not found either. The seeded river runs a
thousand paces down the western rim, and a per-watering round trip that long
would make the garden an errand rather than a slow reward — but a barrel simply
standing full in the court would be the valley handing something over. So the
cistern is the motel's own staved-in rain butt at the corner where the roofline
drains: it holds nothing until the Scribe rebuilds it, and only then does it
answer for water. Its two states live in the same saved scene-state map the
repair pairs use, and like a bed it writes its own prompt, because what it wants
depends on which state it is in.

What the Scribe eats before the first harvest is forage: twelve wild plants
across the whole valley, one ration each, and nothing else. They are food and
never seed, so gathering can never short-circuit the garden's own loop. They are
deliberately meagre — a bridge to the first harvest, not a living.

Six tests hold the economy together, because its failure mode is silent — a task
authored with a supply or tool the valley never produces simply leaves its skill
at zero with nothing in the UI to explain why, and a bed that cannot be sown
looks exactly like one that has not been sown yet. One walks the full chain from
the first cleaned debris to every skill at its ceiling; one checks all authored
tasks in every scene against what the world can actually produce; one checks that
every skill has enough level-0 work authored to reach its ceiling, counting the
standing stations and one full season on every bed alongside the scene files,
since Cultivation lives entirely outside the room JSON; one checks that the single
sack of seed can open a garden that then keeps itself; one checks that nothing
manufactured is obtainable off the ground; and one walks a bed through every
stage of a season asserting the prompt names what that stage wants, including the
stage that wants nothing.

`Progression::shortfalls` names everything standing between the Scribe and one
job, where `attempt` reports only the first thing it hits. Prompts use the former
because they have room for the whole reason; the work itself uses the latter.

New placements store `position: {grid, x, y}`. Multiplying the signed integer
coordinates by that placement's grid yields its exact native-pixel offset from
the room's top-left corner. Because the grid travels with each placement, one
room may safely mix 16-, 32-, and 48-pixel snapping and later editor changes do
not reinterpret existing positions. Legacy schema-v1–v3 `x`/`y` cell anchors
remain supported. Collision, entry, and exit coordinates intentionally stay on
the independent logical gameplay grid.

Baked placements and mutable instances may carry optional `flip_x`/`flip_y`
booleans. The editor previews those transforms without interpolation. The asset
build applies them once while flattening baked scenery; Bevy applies instance
flips through the sprite renderer. One instance transform covers every repair-
pair state, preserving alignment across state changes without duplicated art.
The fixed layer order is floor, wall, object, then overlay; mutable instances use
their optional layer override before falling back to the repair pair's layer.
The build emits one full-scene baked cache per layer so baked art interleaves
with mutable sprites correctly. Within one layer, mutable art renders just above
baked art.

`scripts/build-assets.py` turns baked placements into four cached room layers and
extracts each repair-pair state referenced by that room as a separate runtime
sprite. The Bevy client spawns mutable instances among those layers, records
changes under stable `room-id/instance-id` keys, and includes those values in
browser save data. The runtime never needs access to the private library.

`scripts/level_editor.py` serves a localhost-only browser editor. Its asset
catalog is generated from filenames, image dimensions, and the distributable
sidecar tags in `meta/asset-tags.json`. The editor supports multi-cell stamps for
48-pixel RPG sheets, ordered layers, collision painting, entry/exit placement,
undo, direct JSON save/load, a searchable repair-pair library with create/edit/
duplicate/delete controls and side-by-side previews, and repairable structure or
fixture stamping. Duplication preserves both source crops but requires a new ID,
making convergent or divergent repair transitions quick to author. Pair deletion
is rejected while a saved room or building references it.

The catalog deliberately exposes direct images under `assets/components` and
pack `Texture/` folders while ignoring Unity/Godot import caches and previews.
Smart slicing finds connected foreground regions separated by transparency. For
opaque sheets dominated by a flat background, the selected source rectangle
also stores a color key, tolerance, and edge softness; preview and build apply
that metadata without altering or resampling the licensed source.

Source images are normally cached in both the browser and editor. Per-sheet
refresh increments an in-memory URL revision, evicts the raw and background-keyed
copies, and redraws every dependent preview while preserving valid crop geometry.

Room pointer drags are authored as single undoable paint strokes. Baked stamps
stride by their native crop's room-cell footprint; repairable stamps stride by
the maximum width and height across all visible pair states. Collision painting
uses a one-cell stride. Bresenham interpolation fills pointer positions skipped
by fast movement without allowing stamps within one stroke to overlap.

The room/building canvas derives its pointer ghost from the same source metadata,
snap coordinate, layer, transform, and initial repair state used to construct the
eventual placement. It is render-only editor state and never enters authored JSON.

The Bevy client places interior maps in a separate world-space island. An upward
head probe transitions only after the player is within a door's art bounds and
reaches the authored collision above it; walking onto an interior exit cell
returns to the saved doorstep. Locked doors block at the same collision and
apply one latched recoil per held approach while explaining where to seek a key. Camera
scale changes to frame the room against a near-black brown backdrop. Interior
movement checks the room collision grid; exterior movement checks the authored
motel grid while treating terrain beyond its canvas as walkable.

Interiors are drawn as fixed layer bands rather than a Y-sorted field, so the
Scribe holds one depth above every band. Scenery authored with
`occludes_player: true` opts out of that: the asset build extracts each flagged
interior placement into `runtime-assets/interiors/<scene>/occluder--NN.png`
instead of baking it into its layer cache, indexed by authored order so the
engine can name the crop without recovering it from flattened pixels. Mutable
instances carrying the flag need no extraction — they are already their own
entities. Either way the runtime compares the Scribe's ground contact against
the art's southern edge each frame and lifts the object above them only while
they stand behind it, so art transparency does the rest and walk-behind depth no
longer has to be faked with collision. The flag keeps its exterior meaning on
buildings, where it still governs the crown reveal.

## Time

A day is ten minutes (`daylight::DAY_SECONDS`). Nothing schedules against the
clock except arrivals; its real job is to make time feel like it is passing, to
give sleeping a reason, and to give strangers somewhere to arrive *from*.

The light is flat through the working middle of the day and only falls at the
ends, because a tint the player has to squint through is a fault rather than
atmosphere. `Clock::tint` returns a wash that leans amber while the light is
still going and cold once it has gone — the difference between evening and night
without either word — capped so full dark is still a readable screen. It is laid
down by a single full-screen UI node at `GlobalZIndex(-1)`, which puts it over
the world and under every parchment panel; that ordering is the whole reason the
status and prompt boxes stay legible at midnight.

Sleeping is refused before dusk, so a bed cannot be used to skip a day the player
does not feel like living through, and sleeping at dusk still costs the whole
night rather than waking at dusk on the same date. Whatever else the night does
happens in `advance_clock`, keyed on the date changing rather than on the bed, so
a player who stands outside until dawn lives the same night as one who slept
through it.

Beds are authored, not hard-coded: an interaction with `kind: "rest"` and
`discovery: "bed"`, currently in rooms one and six — the two with a sound roof
and a door that latches. A test asserts that list, because a bed in a room open
to the weather is a lie the content can tell silently.

## Strangers

Nobody arrives because the story says so. A fire in a dead valley is the only
advertisement the waystation has and a frightening one: smoke means strangers,
and strangers are what everyone left alive has learned to avoid. So the first
three nights of a lit hearth bring nobody at all, and after that the odds climb
with each night the fire keeps up and then level off — a waystation becomes
known, but the wastes never become busy. `Visitors::roll_for_today` throws once
per day and schedules an hour in the working middle of it; tests cover the cold
hearth, the early nights, the ceiling, and the once-a-day guarantee.

An arriving party walks in from the western road so it is seen coming, and waits
about seventy seconds in the open before deciding this was a bad idea. Nothing
warns the player it is counting. A stranger standing beside a building they do
not know is taking a risk, and if the keeper of the fire does not come out, the
sensible thing is to keep walking.

Profiles carry art, a name pool, and which authored vignettes suit them, so the
same LPC sheet arriving twice across a long game is not the same person twice.
A profile may have two bodies — the sibling pair walk in together, are addressed
together, and are offered hospitality as one party.

Greeting opens their story, and the last line sends the vignette to the same
Gloo/YouVersion listening call the earlier build used. What changed is what the
answer does: the need it returns now only marks which of the Scribe's own prints
their hand goes to first. It never removes a choice, and every hospitality screen
keeps a way out — sharing food, offering a room, and giving a card are each
either an offer the Scribe can make or a plain statement of why they cannot.
Turning somebody away is a valid play and the farewell says so without scolding.

A guest given a room goes into it for the night and comes back out at dawn to say
goodbye; only then does the visit end. Offering a room needs the brass keys,
which needs having searched the office desk, which is a requirement the player
meets by exploring rather than by being told.

## What the Scribe does after dark

Prints are cut at night, unasked, from whatever was read that day. Nothing is
unlocked by making them; it is what the Scribe does with their hands when the
day's work is finished and the alternative is lying in a strange room thinking
about people who are not there. `Collection::cut_a_block` prefers a theme
matching the last passage read, which is the only mechanical link between the
book and the blocks and is deliberately a preference rather than a rule.

Colour gates the later cards: the catalogue's `stage` field maps to a tier, and
running out of blocks the Scribe can cut in the colours they have is a quiet,
findable reason to want dyes. Dyes come from people, which is the point.

`content/prints.json` stays the authority on which cards exist. The asset build
composes every catalogue entry into `runtime-assets/prints/`, falling back to a
readable placeholder card when an illustration has not been generated yet — so
authoring a reviewed verse is never blocked on running the image pipeline, and a
test asserts every catalogue entry has a card in the runtime tree.

## Nothing tells the player what to do

There is no objective line. The status panel carries the date and the last thing
that happened, and that is all it has ever been allowed to carry since the
scripted arc was removed. Working out what a ruin needs is the game.

Requirements live on the thing they are about and appear when the player walks up
to it. The hearth is the pattern: `hearth_blockers` produces one list, the nearby
prompt renders it as a terse requirement line, and `hearth_complaint` renders the
same list as something the Scribe says out loud — "I can't light this fire, no
telling what's clogging up that chimney." A test asserts the complaint names the
flue and the fuel and mentions neither the roof, the ladder, nor where kindling
is found: saying what is wrong is the game's job, and finding out where to fix it
is the player's.

Searching is worth doing because most of what it turns up is worthless.
`content/salvage.json` is mostly bent wire, dead batteries, and a photograph of
two strangers, read by somebody literate enough to describe a television remote
and not to recognise it. A test caps the share of finds that pay out, because the
moment searching becomes reliably profitable it stops being curiosity. Search
spots are authored per scene, give up one find, and are then empty for good.

The Bible is read, not collected. It never leaves room three: the Scribe opens
it, reads what it falls open at, and puts it back. The first reading is fixed and
is about taking a stranger in — the idea has to be in the Scribe's head before a
stranger exists, or the welcome is only a quest step. Everything after is drawn
without repeating until the book is finished.
