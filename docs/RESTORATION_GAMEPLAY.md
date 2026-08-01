# Restoration gameplay

The first restoration loop is intentionally small enough to understand at a
glance while leaving room for more specialized work later.

```text
Upkeep 1 ─┬─ Carpentry 1 ── Roofing
          └─ Masonry
```

- **Upkeep** begins unlocked. Cleaning debris needs no experience, tools, or
  supplies. Three completed jobs raise a skill by one level.
- **Carpentry** and **Masonry** unlock at Upkeep 1. A level-0 job in an unlocked
  specialty is how the Scribe begins learning it.
- **Roofing** unlocks at Carpentry 1. Its jobs normally need both a hammer and a
  ladder.
- Skills currently have three levels. Tools have persistent condition and
  location; supplies are consumed.

There is no quest list and nothing announces a next step. What the valley can
support is: kindling under the old growth, three debris items in the office and
room 5, a fallen ladder somewhere in the grass, a chimney that has to be reached
from the roof, and then a fire. Searching the office desk yields the motel keys
and nails. The authored tool shed behind the motel holds the working hammer, wood
axe, shovel, and a broken pickaxe, and the sawbuck stands against its back wall —
the one place in the valley a log becomes planks, and the only roof old enough to
explain why a wooden bench is still standing. Fallen logs, kindling, and sound
planks are scattered across the expanded valley.

None of that is stated to the player. Requirements appear on the thing they
belong to, when the Scribe walks up to it — the cold hearth says what is stopping
it, and does not say where to go about it. Whether the player ever lights the
fire is up to them; everything the waystation becomes afterwards simply does not
happen if they do not.

`R` performs authored restoration tasks, including cleaning, clearing, restoring,
and literal repairs. `E` remains the contextual interaction key for inspecting,
searching, gathering, sleeping, tending the hearth, and speaking to whoever has
come down the road. Keeping those paths separate prevents a search target beside
damaged scenery from silently choosing the wrong action.

## The folio

`P` opens the block-prints at any hour and anywhere, `←`/`→` turn a leaf, and
`P` or `ESC` puts it away. While it is open the world does not answer the
keyboard, so leafing through prints cannot walk the Scribe into a wall or start
a repair. Prints already given away remain in it, marked as gone.

## Portable tools

The Scribe carries at most three tools. `E` picks up a nearby tool, `Tab` changes
which carried tool is selected, and `Q` puts it down. A dropped tool keeps its
scene and native-pixel position in save data. Using `Q` inside the tool shed
returns a shed tool to its authored home position instead. Drop positions are
checked against water, trees, buildings, room edges, and authored collision.

Broken tools may still be carried but do not satisfy a task requirement. Taking
one says so in the Scribe's own voice — *I'll need to repair this pickaxe before
I can use it* — and stops there, without naming the bench, the skill, or the
material.

`R` mends a broken tool, either where it lies or out of the pack. A tool goes
where the Scribe goes, so the job goes with it: when nothing underfoot wants the
key, `R` works on the carried tool, preferring whichever one is selected. Only
when the player is standing over a job does that job take the key instead. With
a broken tool in hand and nothing nearby, the prompt line becomes
`R — repair the broken pickaxe     [Upkeep 2 · hammer · 1 sound plank or 1
fallen log]`, which is the same shape every standing station uses: it names what
the work wants and not where to find it.

Mending costs **Upkeep 2**, a serviceable **hammer**, and **one piece of wood** —
a sound plank or a fallen log, whichever the Scribe has. A broken hammer is the
one tool that needs no tool, since it cannot mend itself; it still wants the
skill and the wood. This is the first requirement in the game where alternatives
are allowed, expressed as `any_of` on a `TaskSpec`: any single option satisfies
it and exactly one is spent, in authoring order, so the plank goes before the
log — a whole log is worth more milled than whittled down for one handle.

Tool state is keyed by its stable item ID and can represent `home`, `carried`,
`dropped`, or `held_by` locations; `held_by` is reserved for later NPC use.

To author one, select a source crop in the scene editor, choose **Portable tool**,
set its type, label, initial condition, and layer, then stamp it like scenery.
Select mode can promote an already placed baked stamp or demote a portable tool
without changing its crop, position, or orientation. Schema-v5 scenes store the
result outside `placements`:

```json
{
  "items": [
    {
      "id": "claw-hammer-01",
      "label": "claw hammer",
      "tool": "hammer",
      "condition": "serviceable",
      "layer": "object",
      "position": { "grid": 8, "x": 28, "y": 25 },
      "source": { "path": "sheet.png", "grid": 48, "x": 13, "y": 12,
                  "width": 1, "height": 1 }
    }
  ]
}
```

The build extracts each item to its own native-size runtime image, so picking it
up never erases or rebuilds the flattened room cache. Initial condition currently
uses the same authored crop in both states; add state-specific portable art when
the repaired tool variants are available.

## Authoring a task

Task data belongs to a reusable repair pair in `content/repair-pairs.json`:

```json
{
  "label": "Broken Floorboards 1",
  "kind": "floor",
  "layer": "floor",
  "task": {
    "action": "repair",
    "skill": "carpentry",
    "level": 0,
    "tools": ["hammer"],
    "supplies": [{ "item": "plank", "amount": 1 }],
    "xp": 1
  },
  "states": {
    "damaged": { "source": {} },
    "repaired": { "source": {} }
  }
}
```

Supported actions are `clean`, `repair`, `clear`, `restore`, and `light`.
Supported skills are `upkeep`, `carpentry`, `masonry`, and `roofing`. Durable
tools are `hammer`, `hatchet`, `trowel`, `ladder`, `pickaxe`, `shovel`, `hoe`,
and `watering_can`; supplies are `kindling`, `log`, `plank`, `nails`, `stone`,
and `cloth`.

The scene editor exposes these fields in the Repair-pair library. A completed
visual may be marked invisible, which is the normal representation for debris:
the same persistent state transition occurs, but no fake blank source tile is
needed. Older repair pairs without task metadata still receive conservative
kind-based defaults at runtime.

## Searchable baked scenery

Furniture that does not need mutable repair art may remain in a baked layer and
still expose a persistent interaction. Room 3 uses this for the preserved
nightstand containing the Gideon Bible:

```json
{
  "id": "bible-nightstand",
  "label": "dusty nightstand",
  "kind": "search",
  "discovery": "gideon_bible",
  "position": { "grid": 8, "x": 22, "y": 8 },
  "width": 32,
  "height": 48
}
```

Interaction positions use the same native-pixel top-left coordinate convention
as placements. Discovery state persists under `room-id/interaction-id` without
requiring the furniture to become a repair pair.

The same mechanism carries three other discoveries, so a scene can author what a
piece of furniture is for without any new engine code:

| `kind` | `discovery` | What it does |
| --- | --- | --- |
| `search` | `gideon_bible` | Opens the book, reads a passage, puts it back |
| `search` | `seed_store` | The one sack of seed grain, once |
| `search` | `salvage` | One draw from `content/salvage.json`, then empty |
| `rest` | `bed` | Sleeps until morning, if the light has started to go |
| `work` | `sawbuck` | Mills one log into planks, as often as there are logs |

Two of those bring their own art rather than sitting over furniture the room has
already drawn: the seed sack, which vanishes when the shelf is emptied, and the
sawbuck, which fills the authored rectangle exactly. Moving the bench, resizing
it, or putting a second one in another scene is a content edit; the game reads
`world/sawbuck.png` at whatever `width` and `height` the interaction gives it.

Where a station stands matters as much as whether it is reachable. Proximity
picks the *nearest* interactable, so a bench in a crowded corner spawns, draws,
and is silently unselectable because a tool on the floor is always half a pace
closer. A test asserts every `work` station has ground in front of it — outside
its own rectangle, walkable, and nearer to the station than to anything else in
the room. The shed's first back-left placement failed exactly that way.

An unknown pairing panics at load with the scene and interaction named, rather
than silently spawning something that answers to no key. Salvage spots record
themselves as found under the same `scene-id/interaction-id` key, so a drawer
already turned out does not offer itself again after a reload.
