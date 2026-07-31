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

The asset build currently copies only the ignored hammer and axe layer pairs to
`assets/custom/lpc-tools`, combines them around rows 12–15 of the Scribe, and
emits `runtime-assets/world/scribe-hammer.png` and `scribe-axe.png` as 6×4 atlases
with 128×128 cells. Open builds generate atlas-compatible procedural tools. The
runtime uses the hammer cycle for hammer-required repairs and the axe cycle for
tree chopping, locks movement for one cycle, and then restores the ordinary
64×64 walk atlas.

Recommended next actions:

- pickaxe: reuse the same 128px six-frame work composer;
- hoe and shovel: compose their thrust layers with rows 4–7, retaining their
  eight-frame layout;
- watering can: confirm whether the generator's thrust layer reads as pouring;
- fishing: use the dedicated fishing-rod layers, then author line/float effects;
- sitting, lying/resting, climbing, farming, and emotes: first use body-only
  rows, adding props only where the action needs them.

Exact generator/exporter credits for the chosen Scribe and tool layers must be
retained before redistributing those LPC-derived runtime sheets; see the known
issue in `docs/ai/known-issues.md`.
