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
- Skills currently have three levels. Tools are durable; supplies are consumed.

The initial playable route is: gather kindling, clean three debris items in the
office/room 5, find the fallen ladder in the valley, clear the office chimney,
and light the hearth. Searching the office desk yields the motel keys, a starter
hammer, and nails. Fallen logs, kindling, and sound planks are scattered across
the expanded valley for later work.

`R` performs authored restoration tasks, including cleaning, clearing, restoring,
and literal repairs. `E` remains the contextual interaction key for inspecting,
searching, gathering, tending the hearth, and welcoming travelers. Keeping those
paths separate prevents a search target beside damaged scenery from silently
choosing the wrong action.

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
tools are `hammer`, `hatchet`, `trowel`, and `ladder`; supplies are `kindling`,
`log`, `plank`, `nails`, `stone`, and `cloth`.

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
