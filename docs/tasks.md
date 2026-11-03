# Active tasks

## Order (2026-11-03)

1. **Mix UI.** Built 2026-11-03 (`ui/MixPanel.svelte` and the `Mix*`
   pages, `ui/mix.ts`, `src-tauri/src/mix_commands.rs`, engine meters in
   `engine/src/meter.*`). Checked in the real app with tools/jello-drive
   (add, reorder, on dot, remove, arrow keys, fader, + Bus, remove bus,
   every page, meters while playing). Left: hear it, and look at it full
   screen (checked only in a 1352x849 window, where the chain and the add
   list scroll). Not checked by driving: EQ and shaper squares, bus
   rename, delay and reverb tiles.
2. **Learning and composing, rethought.** Tutoring, composing through the
   agent harness, guided mode and everything about learning and using the
   instrument well. Nothing that exists now has to stay, its UI included.
   Designed one topic at a time.
3. **Later:** Produce (after everything else is final), Windows checks, and the
   finish line song.

Heard and approved by ear: guitar presets, grand piano, song key mappings
while playing, and Mix sound.

## Guitar: known limits

Built ([ADR 0049](adr/0049-the-keyboard-guitar.md), [design](design.md#guitar)).

- Loops replay a string's move as a fresh pluck, and Compose doesn't time moves.
- Presets saved earlier stay as saved: remove one from the Sounds folder to get
  the new values.

## Grand piano: later

Built ([ADR 0048](adr/0048-the-grand-piano.md), [design](design.md#sampler)).

- A room reverb belongs to Mix; `jello_mix` can add one now.

## Drums: later

- Either find a better drum kit, or design this kit's key maps ourselves.

## Song key mappings: known limits

Built ([ADR 0047](adr/0047-song-key-mappings.md), [design](design.md#song-key-mappings)).

- The next-key marks cover the looper's picked track, not arrangement playback
  or further progression chords. Compose's now and next chord lights remain.
- A practice piece's note with no key on the mapping gets no mark.
- The mapping page edits the song's mappings, so Play's scale default has no
  page.
- Melody labels spell with sharps (D#4) beside chords in flats (Eb).

## Mix backend: left over

Built and committed (151f94c).

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

### Next

- Tests miss two deliberate mistakes: a send ignoring its amount
  (`engine/src/mixer.cpp`) and a replaced effect never freed
  (`engine/src/conductor.cpp`). Add a test for each.
- Once (2026-10-30), "Untitled" and "Untitled 4" ignored clicks and presses
  for about a minute after opening, with no message; not reproduced in 26
  more tries. If it returns, log `open_song` in Rust and `open` in
  `SongsPanel`.
