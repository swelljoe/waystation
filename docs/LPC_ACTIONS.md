# LPC Scribe action map

The Scribe runtime source is a complete 13-column × 54-row LPC sheet with
64×64 cells. Rows are zero-based:

| Rows | Action | Directions / frames |
| --- | --- | --- |
| 0–3 | spellcast | up, left, down, right; 7 frames |
| 4–7 | thrust | up, left, down, right; 8 frames |
| 8–11 | walk | up, left, down, right; 9 frames |
| 12–15 | slash | up, left, down, right; 6 frames |
| 16–19 | shoot | up, left, down, right; 13 frames |
| 20 | hurt | 6 frames |
| 21 | climb | 6 frames |
| 22–25 | idle | up, left, down, right; 2 frames |
| 26–29 | jump | up, left, down, right; 5 frames |
| 30–33 | sit | up, left, down, right; 3 frames |
| 34–37 | emote | up, left, down, right; 3 frames |
| 38–41 | run | up, left, down, right; 8 frames |
| 42–45 | combat idle | up, left, down, right; 2 frames |
| 46–49 | backslash | up, left, down, right; 13 frames |
| 50–53 | halfslash | up, left, down, right; 7 frames |

The LPC generator also supplies tool-specific foreground/background layers.
Hammer, axe, and pickaxe are 128×128, four-direction, six-frame work cycles that
fit the Scribe's slash body frames. Hoe and shovel provide thrust and walk layers;
the watering can currently provides thrust and walk layers. These are the safest
next action integrations because the character body and tool ordering already
exist without drawing new animation.

The asset build copies the ignored tool layer pairs to `assets/custom/lpc-tools`
and composites them onto the Scribe in two shapes, because the generator draws
the two families at different sizes:

| Family | Body rows | Overlay | Runtime atlas |
| --- | --- | --- | --- |
| hammer, axe | slash, 12–15 | 128×128, 6 frames | `scribe-hammer.png`, `scribe-axe.png` — 6×4 of 128px |
| hoe, shovel, watering can | thrust, 4–7 | 64×64, 8 frames | `scribe-hoe.png`, `scribe-shovel.png`, `scribe-watering-can.png` — 8×4 of 64px |

The long-handled overlays share the body's own frame size, so they stack without
the centring offset the swung tools need. Open builds generate atlas-compatible
procedural stand-ins for both shapes. `ToolWorkAnimation` carries each cycle's
frame size and column count so the runtime does not assume one atlas geometry;
it locks movement for one cycle and then restores the ordinary 64×64 walk atlas.
Which cycle plays is derived from a task's own `tools`, so a new tool needs a new
arm in one place rather than at every site where work happens.

The thrust cycle reads clearly from the side and is foreshortened from the front,
which is how LPC draws it; the watering can's pour lands on frames 5–6.

Recommended next actions:

- pickaxe: reuse the 128px six-frame swing composer; quarrying currently borrows
  the hammer body because no pick layer is drawn;
- sowing and harvesting: body-only thrust rows, no overlay, so hand work in the
  garden stops being silent;
- fishing: use the dedicated fishing-rod layers, then author line/float effects;
- sitting, lying/resting, climbing, and emotes: first use body-only rows, adding
  props only where the action needs them.

Exact generator/exporter credits for the chosen Scribe and tool layers must be
retained before redistributing those LPC-derived runtime sheets; see the known
issue in `docs/ai/known-issues.md`.
