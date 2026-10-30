# Active tasks

## Build song key mappings

Design approved ([ADR 0047](adr/0047-song-key-mappings.md)), not built. Each
scale gets a default mapping, the user or the agent remaps while working, and
the agent optimizes from the finished song.

- Nothing stays fixed between songs: any note can go on any key. The player
  reads the lit keys and types them, so nothing needs memorizing.
- A key plays any set of notes: one note or any chord voicing from the song.
- A scale's default holds only notes that fit it, plus outside notes that
  still work. Keys left over get the best notes in other octaves. Chord keys
  follow the same rule: 14 chords fit a major key and 11 a minor key today
  (`harmony.rs`), and the rest get other voicings of the best ones. A note
  left out has no key until the user or the agent maps it.
- Each scale's notes follow the approved table in
  [design](design.md#song-key-mappings): major adds b3, b7 and b5, minor adds
  6 and b5.
- Scale defaults keep chords left and melody right (0012); a song's optimizer
  may move a key across hands.
- Chord keys go by role, most-played under resting fingers. C major: home row
  A S D F G plays Am G F C Dm; upper Q W E R T plays Eb Ab Fm Bb Em; bottom
  Z X C V plays B A D E; the 10 spare keys get other voicings of the best
  chords. The melody shape in design is the starting point, tuned by ear.
- One mapping per section: each starts on the song's main mapping, and a
  change counts against the optimizer's score. The switch happens at the
  section boundary, the new keys light, and held notes keep sounding.
- An optimized mapping arrives as an agent card with old and new keys and the
  comfort gain. Try plays it unsaved, Use saves it. Locked keys never move,
  and nothing changes while the song plays.
- Manual mapping has its own separate page, not a panel in the tape's slot.
- On that page, press the laptop key to edit, then click notes on a drawn
  piano to add or remove them, each sounding as it is clicked.
- Pick a scale first: keys play nothing until a scale is chosen, so today's
  layout goes away as a mapping.
- Older songs need no support: the six placeholders went to the Trash on
  2026-10-30, so no key detection or old-file handling for mappings.
- Play asks for a scale once, then opens on the last one used. A song keeps
  its own scale.
- The optimizer's score starts from layout-analyzer rules and learns from the
  player's own timing and misses, kept on their machine.
- To build: the next key lights only for chords in Compose today; melody notes
  need it too. Play has no song to look ahead in and stays unguided.

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
