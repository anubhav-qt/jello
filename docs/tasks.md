# Active tasks

## Finish Mix backend integration

Changes are uncommitted. Runtime checks pass; sound by ear and Windows are
still unchecked. Do this before starting Mix UI work.

### Settled decisions

- Effect connections belong to the song Mix and survive sound preset loads.
  Tracks share their existing 8 LFOs and 64 connections with sound routing;
  buses and master have their own. Presets replace shared track LFO settings.
- Reject combined connection counts above 64 before any mutation. Preserve
  the current sound, Mix, engine state and playback on rejected changes.
- Connections with per-note or press-based LFOs remain saved but inactive;
  they reactivate when their source and scaling LFOs become compatible.
- Stop arrangement playback before changing occupied ordinary connection
  slots. Ordinary values, LFO edits and unchanged occupancy remain live.
- Bus removal stops arrangement playback first, deletes automation targeting
  that bus, and adjusts later `bus N` and track-scoped `send N` references.
  Invalid removal must not stop playback or mutate state.
- Fresh compressor, gate and filter effects initialize their selected
  listener immediately; later source changes retain the 10 ms fade.
- Temporary internal track IDs identify preview targets across selection
  and reindexing. These IDs are not persisted or sent to the engine.

### Implemented and reviewed

Mix modulation model, persistence and tools; checked preset and sound edits;
card Play/Use/restore routing; drum LFO/routing save and undo; sparse ordinary
connection indices with legacy file support; percentage-point tool amounts;
bus automation cleanup; listener initialization. Independent Sol reviews
have no remaining actionable static findings after corrections.

### Fixed after review (2026-10-30)

`cargo test --lib`: 935 passed, 0 failed.

- Song open and WAV render lost settings to the full 1,024-slot mailbox. They
  now pace their batches (`Engine::make_room`), every lost message is counted
  (`Conductor::send_counted`), and both report an error instead of going on.
- Stalled audio is waited for once, up to 500 ms, not once per batch.
- Stopping an arrangement now puts back what its automation moved.

### Next

- Real app, audio on (2026-10-30): six songs opened about 40 times with no
  error and no file changed. Once, "Untitled" and "Untitled 4" ignored clicks
  and presses for about a minute, with no message; not reproduced in 26 more
  tries. If it returns, log `open_song` in Rust and `open` in `SongsPanel`.
- Tests miss two deliberate mistakes: a send ignoring its amount
  (`engine/src/mixer.cpp`) and a replaced effect never freed
  (`engine/src/conductor.cpp`). Add a test for each.

### Other verification and follow-up

- Listener and bus regressions pass, including the previously failing
  master-listens-to-bus case.
- Under Node 24.21.0, `npm run build` passed with 0 errors and warnings;
  `npm run test:agent` passed 8 tests. No UI/JS changes in this task.
- `git diff --check` passed; no added source em dashes were found.
- Real-app/browser QA was not run. Jello is not running; native driver
  Accessibility is trusted, but no browser control tool is available.
  Audible testing requires the established sound-demo approval. The app
  has no supported isolated data root, so plan disposable song handling.
- Preexisting: README and CI invoke nonexistent `test:tutor`; package.json
  defines `test:agent`. This mismatch is not fixed.
- No Mix UI yet. Mockups follow backend verification and separate approval.
