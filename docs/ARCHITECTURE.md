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

