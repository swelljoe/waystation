# Audio

## Runtime mix

- The web build presents an **Enter the Waystation** button after WebAssembly
  loads. Bevy and its audio output are constructed synchronously by that user
  gesture so browser autoplay policy does not leave every sink permanently
  silent. Native builds still start immediately.
- The two AndriiG tracks alternate at 10% linear volume with a twelve-second
  quiet interval between songs.
- Rain begins immediately and remains continuous for the first six minutes.
  Afterward, deterministic wet and dry intervals alternate so save/reload and
  testing do not depend on an opaque random source.
- Both rain derivatives are normalized to −22 LUFS during the private build.
  Exterior rain plays at 22% linear volume. Entering an interior crossfades it
  to a 900 Hz low-pass derivative at 5%; leaving restores it smoothly. Dry
  weather fades both synchronized loops to zero.
- Walking within the authored bounds of a damaged `Broken Floorboards` mutable
  placement rotates through three creaks, with a 720 ms minimum interval.
  Repaired placements no longer trigger sound.
- Hammer-required restoration and tool repair plays the Dragon Studio hammering
  effect with the matching six-frame LPC work animation.

Bevy 0.17's built-in audio integration exposes per-sink volume, playback,
speed, and stereo spatialization, but not a real-time filter or EQ graph. The
private asset build therefore uses FFmpeg to derive and loudness-normalize
compact two-minute outdoor and low-pass indoor loops from the untouched source
recording, and the runtime crossfades them without adopting a custom audio
backend.

## Licensed source boundary

Raw files under `music/` are ignored, like purchased art under `assets/`.
`assets-manifest.json` explicitly selects the two music tracks, one rain loop,
three creaks, and one hammering effect. `scripts/build-assets.py` copies or
derives only those files and their available attribution notes into
`runtime-assets/audio` and records
their hashes in `runtime-assets/provenance.json`.

Open CI builds tolerate an absent private music directory. Preparing private
rain audio requires FFmpeg. A strict demo build requires every selected source
and attribution file. After adding or replacing audio, update the manifest and
run `make publish-demo-assets` before deploying the hosted demo.
