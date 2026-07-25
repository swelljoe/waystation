# Submission Runbook

## Three-minute video

- **0:00–0:15 — Hook:** the storm, a lone Scribe, and impossible standing stone.
- **0:15–0:45 — Discovery:** inspect the motel sign, light the hearth, find the book.
- **0:45–1:15 — Hospitality:** smoke rises; Mara arrives and tells her story.
- **1:15–2:05 — Native Scripture loop:** show listening, validated Gloo selection,
  YouVersion passage retrieval, and the three-part illuminated card craft.
- **2:05–2:30 — Payoff:** Mara carries the first remembrance back to the road.
- **2:30–2:50 — Proof:** brief architecture/provenance overlay and both live APIs.
- **2:50–3:00 — Vision:** more rooms, travelers, languages, and paths converging.

Capture the game at 1920×1080, keep browser chrome out of the primary footage, and
record one uninterrupted live API sequence. Do not represent fixture mode as live.

## Writeup draft (under 500 words)

**The Waystation at the Edge of the Ash** asks what a book can rebuild after the
institutions around it have disappeared. In a far-future world where literacy is
rare, the player is The Scribe: a wanderer who discovers a preserved Bible inside
an ancient stone motel. By restoring its hearth and writing desk, the Scribe turns
an abandoned ruin into a place of hospitality.

The first traveler does not arrive looking for religion. They follow smoke because
they need warmth. The player listens to an authored story of grief, weariness, or
exile, then makes an illuminated remembrance they can carry back onto the road.
Scripture is therefore not a notification or reward. It is the material at the
center of the game's listening, craft, and world-building loop.

Gloo AI Studio interprets the need beneath each authored vignette. We use a required
structured tool call constrained to a small, human-reviewed catalog of compassionate
themes and paired passage IDs. Our server independently validates that pair, then
asks YouVersion for the authoritative passage text and attribution. Gloo never
invents or paraphrases the Scripture shown on the card. The player chooses paper,
pixel illumination, and border, making the result personal without turning comfort
into a quiz with right and wrong answers.

The product is a Rust workspace: a Bevy WebAssembly game, a shared validation crate,
and an Axum service that keeps OAuth and app credentials outside the browser. Live,
cached, and reviewed-fixture results carry visible provenance. Authored content is
data-driven RON, while a deterministic Python pipeline generates card art and
license reports. An OCI container built with Podman serves the complete public demo.

This short story is the beginning of a larger cozy game. Every restored motel room
will support another act of care; every traveler can carry words to another valley;
and languages from YouVersion can let the waystation welcome people the Scribe's
world—and ours—too often leaves outside.

## Final checklist

- Public cover image and media gallery
- Public YouTube video no longer than three minutes
- Kaggle writeup no longer than 500 words
- Attached public notebook from `notebooks/waystation_pipeline.ipynb`
- Public repository link and live `http(s)` project link, neither requiring login
- Health check reports live configuration; one final live playthrough succeeds
- Submit the single allowed entry before the deadline; do not leave it as a draft

