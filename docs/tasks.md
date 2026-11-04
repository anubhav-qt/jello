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
   Designed one topic at a time; every decision is in
   [design](design.md#ai-and-learning-rethought-2026-11-03). Settled: takes
   (one per answer), the slim right rail, right Command keys (Windows key
   open), the jam card, the jam partner, practice with two hand lanes,
   skills from songs, taste drills, one agent, the learn switch, the path
   by song stages, lyrics now with vocals next, and tools do the work.
   References: full depth (stems, notes, sound matching). Shared library.
   The path shows as a stage strip on the tape (constant tape height).
   [ADR 0050](adr/0050-ai-and-learning-rethought.md). Building, piece by
   piece:
   - Built: the path (`path.rs`, `path_tool.rs` with `jello_path`; song
     file `learn` and `stages_done`; topics covered in
     Memory/topics.json), the stage strip and topic map on the tape
     (`ComposeTape.svelte`, `TopicMap.svelte`), the Learn switch (in the
     rail's header) and the Learn line the agent gets.
   - Built: the rail (one agent, no jobs, "Open now" lines), takes
     (`takes.rs`: flip, keep, drop, drop a part; right Cmd, Return, Esc),
     lesson and task cards, the jam card (`jam.rs`).
   - Built and tested (2026-11-04: tests, deliberate mistakes, driven in
     the real app; the AI's jam picks by tests only), not yet heard: the
     AI picking and naming jam moments (`jam.rs`, host `pickMoments`),
     practice from a section with two hand lanes (`practice_song.rs`,
     `PracticeLanes.svelte`, Practice buttons in section details,
     `RailPractice.svelte`, `jello_practice` action `section`), skills
     from songs (`song_skills.rs`, Memory page, look part `song_skills`),
     taste drills (`drills.rs`, `RailDrill.svelte`, while Learn is on:
     chord, interval, sound), the jam partner's call and answer
     (`jam_partner.rs`, `RailPartner.svelte`).
   - Next: the jam partner looping everything so far while the AI works,
     and the groove that follows; the EQ and reference drills; the
     keyboard marking the next key in song practice. Lyrics and the
     reference card get mockups first.
   - Known gaps: right Cmd blocks nothing while talking (Cmd Q still
     quits); a take that changes another song than the one open reopens
     its song; a section's practice plays without the section's key
     shift (to decide).
3. **Later:** Produce (after everything else is final), Windows checks, and the
   finish line song.

Heard and approved by ear: guitar presets, grand piano, song key mappings
while playing, and Mix sound.

## Learning: later

- A graded set of taste drills, easy to hard, beside the drills made from
  the player's songs ([design](design.md#ai-and-learning-rethought-2026-11-03)).
- A fixed curriculum (scales, chords, functions, cadences, modes and on),
  beside the path that follows each song's stages.

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
