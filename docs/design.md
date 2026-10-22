# Jello design

This is the living design for Jello. Decisions that are settled also have an
ADR in [adr/](adr/README.md). When this file and an ADR disagree, the ADR wins
and this file should be fixed.

## Vision

The dream: release a track that sounds incredible, and when someone asks
which DAW and synths made it, answer "I made the software. The synths, the
studio, the whole thing. It's called Jello."

Jello is a desktop synthesizer and music studio with four long-term goals:

1. **Make real music.** Production-grade songs, ready to release, at the
   level of the user's references: YOASOBI, Ado, Pink Floyd, Massive Attack,
   Radiohead.
2. **Sound like no one else.** Jello grows around the user's artistic
   instincts: instruments and workflows that exist nowhere else, used to
   develop a sound that is distinctly theirs.
3. **Learn to play.** A follow-along mode that trains both hands to play in
   sync. Sync is the goal; speed is secondary.
4. **Make music with AI.** An AI can drive Jello from outside (MCP server) or
   from inside (an in-app harness), and write songs as code.

## Roadmap

### v1: the instrument and studio

1. **Skeleton.** Tauri app, Rust calls C++, C++ plays a test tone through the
   speakers and renders it to a WAV file. Builds on macOS and Windows.
2. **Laptop synth.** Play notes and chords with the current computer keyboard
   layout, with all 24 major and minor chords on keys of their own
   ([ADR 0020](adr/0020-every-chord-on-its-own-key.md)). On-screen
   piano. Custom and guided mapping setup remain optional.
3. **MIDI keyboard input.** Dropped: Jello is played on computer keyboards
   only ([ADR 0027](adr/0027-computer-keyboards-only.md)).
4. **Sound.** A deep synth and a sampler (for piano and drums), all our own
   code, with no guitar
   ([ADR 0019](adr/0019-instruments-built-from-scratch-no-guitar.md)).
5. **Arrange.** Tracks, sequencer, piano roll, recording from the keyboard,
   tempo and key changes, the text song format. Playing a composed song
   highlights its next chords on the same keyboard. Guided composing came
   early: the Compose tab already builds a progression with guidance
   ([ADR 0017](adr/0017-guided-compose.md)). See
   [chord guidance](#chord-guidance).
6. **Mix.** Mixer, effects, sidechain, automation, master limiter, WAV export.

**Finish line:** a full YOASOBI-style instrumental made entirely in Jello and
exported as a WAV file.

The system check (see [Performance](#performance-and-system-check)) grows
together with the engine.

#### Step 2 slices

Step 2 is built in slices. Each one is explained and approved before it is
written, then tested and explained again. The engine tests render notes
without speakers and check pitch, envelope timing, exact silence after
release, no jumps when a voice is stolen, and never going above 1.0.

1. **Design written down.** [ADR 0009](adr/0009-how-notes-become-sound.md).
   Done.
2. **One note.** The mailbox, the envelope and one voice playing triangle,
   with the velocity curve and the voice level. The C API gains note on,
   note off and all notes off; Rust gains `play_note`, `release_note` and
   `release_all_notes`. The test tone goes away: Render WAV saves a held
   middle C to `Music/Jello/test-note.wav`, and the dev panel gets a button
   that plays middle C while it is held. Done.
3. **32 voices.** Finding a free voice, stealing, reusing a ringing note's
   voice, and the soft clip. The C API and the dev panel stay the same, so
   only the tests play chords until slice 5. Done.
4. **Four waves.** Sine, square and saw join triangle, with PolyBLEP, the
   wave trims and the wave switch. The triangle's corners are rounded off
   too, and a voice taken over on a new wave glides across instead of
   clicking. The C API gains set wave and Rust gains `switch_wave`; the dev
   panel stays the same until slice 5. Done.
5. **Play from the laptop.** The webview forwards keys, the Rust key map
   plays the default preset (the accordion grid) and the backtick key (`` ` ``) switches waves.
   Rust gains `press_key`, `release_key`, `current_wave` and `key_layout`,
   and `release_all_notes` also forgets which keys are held. The dev panel
   is replaced by the Stage start page ([ADR 0010](adr/0010-stage-look-for-the-start-page.md)),
   which draws the laptop keyboard with each key's note and lights held
   keys. Then tune by ear: the voice level and release stay as they were,
   square and saw go 2 dB quieter, and held notes stay at full volume
   instead of dipping. Done.

After the slices: the frontend framework is Svelte 5
([ADR 0011](adr/0011-svelte-5-for-the-ui.md)), and the Stage page has moved
to it. The default layout now plays chords with the left hand and the
melody with the right ([ADR 0012](adr/0012-chords-left-melody-right.md)). Keep
that layout for now instead of spending more time redesigning it. Later,
chord guidance highlights what to try or play next on these familiar typing
keys ([ADR 0013](adr/0013-highlight-next-chords-on-the-existing-layout.md)).
The on-screen piano is still planned; custom mapping is optional.

Then, ahead of step 5: Space and right Shift give the chord keys minor and
sharp chords ([ADR 0015](adr/0015-minor-on-space-sharp-on-right-shift.md)),
the top bar gets Play, Compose and Produce tabs
([ADR 0016](adr/0016-play-compose-and-produce-tabs.md)), and Compose gets
guided mode ([ADR 0017](adr/0017-guided-compose.md)). Compose and Produce
are dev panels until they are designed, and the guidance outlines on the
chord keys are a stand-in too.

Asked for after playing it (2026-10-20), each to be designed and approved
in turn:
1. Every chord on its own key, instead of holding Space or right Shift
   ([ADR 0020](adr/0020-every-chord-on-its-own-key.md)). Done.
2. Guidance that changes after each chord is added, to what fits best next.
3. Guidance on the melody keys too. Done
   ([ADR 0023](adr/0023-guidance-on-the-melody-keys.md)); see
   [chord guidance](#chord-guidance).
4. Shift as a looper: the first press records, the next loops it. Designed
   in [ADR 0021](adr/0021-a-looper-on-a-beat.md): loops follow a beat, and
   recorded notes snap to it. See [Looper](#looper). All four slices are
   done, with layers ([ADR 0026](adr/0026-layers.md)) last. Its look
   ([ADR 0024](adr/0024-the-looper-s-look.md)) is built, with the lane
   redesigned in [ADR 0025](adr/0025-the-looper-s-lane-as-wide-as-the-keyboard.md):
   as wide as the keyboard, with the snap switch and a box per layer.

### v1.5: vocals

Audio tracks, importing and recording vocals, a vocal effects chain, live
pitch correction and hum-to-notes. See [Vocals](#vocals-v15).

### v2: learning

Follow-along mode, levels, AI-generated exercises, note-by-note vocal editing.
See [Learning mode](#learning-mode-v2).

### v3: AI composer

MCP server, in-app AI harness on a fork of pi
([ADR 0018](adr/0018-ai-harness-on-a-fork-of-pi.md)), AI voice conversion.
See [AI](#ai-v3).

### After v1, in any order

- **Artist sound studies.** Pick one reference artist, build the modules that
  sound needs, make a track in that style.
- **No guitars for now.** Guitarless music is the trademark
  ([ADR 0019](adr/0019-instruments-built-from-scratch-no-guitar.md)).
- **Third-party plugins (AU/VST3).** Maybe, but never as the way Jello gets
  its sound (ADR 0019). A big feature on its own.

## Architecture

```text
   UI (webview)        scripts / AI (later)
          \                  /
           v                v
   COMMANDS (Rust): every action has a name
           |
   PROJECT (Rust): the song as text, compiled to timed events
           |  C API
   ENGINE (C++): sequencer, instruments, effects, mixer, speakers or WAV
```

| Layer | Does | Never does |
|---|---|---|
| Webview (TypeScript) | Draws, forwards laptop keys | Music logic, timing |
| Rust core | Commands, input router, song files, follow-along clock and scoring, system check, file I/O | Per-sample audio work |
| C++ engine | Sequencer (sample-accurate), instruments, effects, mixer, audio out, offline render | Parsing files, scoring |

### Rules

1. The live path (key or MIDI to sound) never waits on the webview.
   MIDI enters in Rust and goes straight to C++.
2. The audio callback never locks, allocates memory, logs or waits. Events
   reach it through a lock-free queue.
3. Every input (laptop keys, MIDI, AI) becomes the same note event, so nothing
   downstream knows where a note came from.
4. Every action is a named command, defined once, used by the UI, the MCP
   server and the in-app AI.
5. Songs are readable text files that compile into music.
6. Offline rendering is deterministic: the same song file always gives the
   same audio.
7. The Rust to C++ boundary is a small, plain C API.

## Engine

How the C++ engine is built today. It grows with each step. Step 2 (notes,
voices, waves) is designed in [ADR 0009](adr/0009-how-notes-become-sound.md)
and being built.

- **C API:** `engine/include/jello_engine.h` is the only thing Rust sees:
  create, destroy, start and stop audio, note on, note off, all notes off,
  set wave, start click, stop click, set tempo, press looper, clear loop,
  set snap, looper status (with its place), next loop change, plan loop,
  play song, stop song, song position, sample rate, render.
- **Files:**
  - `engine.cpp`: the C API and all device code.
  - `mailbox.h`: carries messages from one thread to another without
    locks. One carries note, wave, click, tempo, looper, snap and song
    messages to the audio thread; another carries the loop's changes back
    to Rust.
    Sending says whether the message got in, and the sender can ask how
    much room is left.
  - `conductor.*`: runs everything the audio thread does, like an
    orchestra's conductor who keeps the beat and cues the players. It goes
    through each chunk of sound sample by sample. On the chunk's first
    sample the looper acts first (a beat, a loop note), and then every
    waiting message, passing notes on to the synth and the looper. So a
    note played on the very sample recording starts is recorded. Then the
    song player moves on to its next chord if one is due. When the looper
    or the song player starts or ends a note mid-chunk, the synth first
    renders the stretch up to that sample. Last, it adds the click on top
    and puts the sum through the soft clip, the last step before the
    speakers.
  - `synth.*`: the instrument. It adds up its voices, picks the voice for
    each new note, and remembers the wave new notes play. It adds into the
    chunk instead of overwriting it, so the conductor can render it in
    pieces.
  - `looper.*`: records live notes and plays the loop back on the beat
    (see [Looper](#looper)).
  - `snap.*`: works out where each time around plays a loop's notes, as
    played or moved to the nearest half beat (see
    [Looper](#looper)). The looper and the C API both use it.
  - `song_player.*`: plays Compose's song
    ([ADR 0022](adr/0022-compose-as-a-tape.md)): a queue of up to 512
    chords of up to 4 notes, each played for its own length in samples,
    back to back, ignoring the beat. A chord lets go of its notes before the
    next one starts, so a note the two share plays again. Rust sends a
    stop, every chord and a play together, or nothing if the mailbox lacks
    room for all of them, so a song never plays in part. It tells Rust how
    far into the song it is; a play still on its way counts as its very
    start, and a stop on its way as stopped.
  - `clock.*`: keeps the beat for the [looper](#looper). It counts every
    sample the engine makes, and knows which sample each beat starts on.
  - `click.*`: the click, a short tick on every beat while it is on, or
    while the looper counts in or records.
  - `voice.*`: plays one note on one of the four waves. The wave trim, the
    velocity curve and the voice level set how loud it is. It knows who
    started its note: the player (live), the looper or the song player,
    and for the looper, which layer. A loop note also plays at its layer's
    volume (see [Looper](#looper)).
  - `wave.*`: the four wave shapes, with their jumps and corners rounded
    off, and the trims that make them equally loud.
  - `envelope.*`: a note's loudness over time: a quick rise (attack), a dip
    to the held level (decay, sustain) and a fade out after letting go
    (release).
  - `miniaudio.cpp` and `miniaudio_settings.h`: the audio library
    ([ADR 0008](adr/0008-miniaudio-for-audio-devices.md)).
- **Build:** `src-tauri/build.rs` compiles the engine as C++17 with the `cc`
  crate, so one Rust build builds everything.
- **Sound format:** the engine always runs at 48,000 samples per second,
  stereo, 32-bit float. miniaudio converts if the speakers run at another
  rate.
- **Threads:** the OS calls the engine on its audio thread every few
  milliseconds. Other threads reach it only through the mailbox, a lock-free
  queue with 1024 slots that it empties at the start of each chunk. When the
  mailbox is full, new messages are dropped. Only one thread may send at a
  time (Rust keeps the engine behind a lock), and the audio thread never
  locks or waits.
- **Voices:** 32, made once when the engine is created. A new note takes the
  voice still ringing that same note from the same source (live, loop or
  song), or else a free voice. When all 32 are
  busy, it takes over the quietest voice fading out, or else the note held
  the longest. Every key press gets the next number from a counter, which is
  how the engine knows which note is oldest. A voice stops on the sample it
  falls silent, so the point in its wave where its next note starts never
  depends on how the sound was split into chunks.
- **Waves:** sine, triangle, square and saw. All four go from -1 to 1 and
  start each cycle rising from zero, so they line up. A new engine plays
  triangle. A switch only changes notes started after it: each voice keeps
  the wave its note started with.
  - The square and saw jump, and the triangle has corners. Played as they
    are, their harmonics too high for the sample rate would fold back down
    as stray, off-key tones (aliasing), worst on high notes. So the samples
    on each side of a jump are bent (PolyBLEP), and so are those on each
    side of a corner (PolyBLAMP). At A7, every stray tone below 5,000 Hz is
    at least 1,800 times quieter than the note.
  - Trims make every wave as loud as the triangle, measured by RMS (the
    square root of the average squared height): sine 0.8165, triangle 1,
    square 0.5774, saw 1. Starting values, to be tuned by ear.
- **No clicks:** a note fades in over 5 ms and out over 300 ms. A note that
  takes over a busy voice picks up from wherever that voice's wave and
  loudness were, so the sound never jumps. If the new note plays another
  wave, that wave starts at a different height. The gap between them is
  added back and shrinks to nothing over 5 ms, so the sound glides across.
  A silent voice has nothing to glide from, so it gets no gap.
- **The beat:** 120 beats per minute to start, from 40 to 240. A beat lasts
  a minute divided by the tempo, so 24,000 samples at 120. Each beat's start
  is worked out from the sample its tempo started on, rounded down to a
  whole sample, instead of adding up beat lengths, so the beat never drifts.
  A new tempo starts with the next beat, so the beat playing keeps its
  length. Every bar has 4 beats.
- **The click:** while it is on, a tick starts on the exact sample of each
  beat, wherever that falls in a chunk. A tick is a sine wave that starts
  from zero at 0.25 and halves in loudness every 4 ms, then stops after
  40 ms. Beat 1 of each bar ticks at A6 (1,760 Hz) and the others at A5
  (880 Hz). It is not one of the 32 voices, so it never takes a note's
  voice. Starting the click begins a new bar, starting it while it is on
  does nothing, and stopping it lets a sounding tick ring out. Kept by ear
  as they first were (2026-10-20).
- **Soft clip:** after the voices and the click are added up, samples
  between -0.8 and 0.8 pass unchanged. Louder ones are bent toward 1.0 without ever reaching it,
  so the sound never clips. The master limiter takes over this job in step 6.
- **Offline render:** Rust makes a second engine with no speakers and calls
  render, the same code the speakers hear. Rust writes 32-bit float WAV files
  (`hound` crate) to `Music/Jello/`.
- **Rust side:** `src-tauri/src/engine.rs` is the safe wrapper,
  `commands.rs` holds the commands, `key_map.rs` turns laptop keys into
  notes, `wav.rs` writes files.
- **Tests:** `cargo test` in `src-tauri/` renders notes silently and checks
  pitch, the envelope's timing and levels, the velocity curve, that a chord
  is its notes added up, exact silence after letting go, which voice a new
  note takes and that taking it never clicks, the soft clip's exact bend and
  never going above 1.0, that values MIDI never sends are ignored, the
  mailbox size, and that the same notes always give the same samples. For
  the waves, they check each one's harmonics, that on an A7 stray tones
  below 5,000 Hz stay at least 1,000 times quieter than the note, that all
  four line up and are equally loud, that a switch only changes new notes,
  and that a voice taken over on a new wave glides across without a click.
  For the click, they check that ticks start on exactly the right samples
  however the sound is asked for, that beat 1 ticks higher, how a tick
  fades and stops, that after 1,000 beats at 115 the beat is still on the
  exact sample, that a new tempo starts on the next beat, the slowest and
  fastest tempos, starting and stopping, that the click takes no voice, and
  that notes and click together stay below 1.0.
  For the looper, most tests play the same music twice: once through the
  looper, and once by hand, with each loop note played live on the exact
  sample the loop should play it. The two must match sample for sample.
  They cover the count-in, closing early, late and right on a bar line, the
  1-bar minimum, notes held across the close, quick and repeated notes, a
  note on the sample recording starts, 60 times around at 115 BPM with no
  drift, 7 ms chunks, Esc, the tempo lock, a full loop of 4,096 notes, loop
  and live notes never cutting each other off, and losing focus.
  The song player tests do the same: a song must match its chords played
  by hand on the exact samples, in one chunk and in odd-sized ones. They
  cover a 2,000 ms chord letting go on sample 96,000, shared notes playing
  again, 0 ms chords, stopping, playing again, live and song notes never
  cutting each other off, the looper never recording the song, Esc leaving
  it playing, where the song is, 512 chords and no more, a song refused
  whole when the mailbox lacks room (and the playhead staying on the song
  playing), and odd chords refused.
  The key map tests check the melody keys finger by finger and each chord,
  that other keys do nothing, and that letting go always stops the notes a
  key started, even after the layout changes, except a note another held
  key still plays. Tests never open speakers.

## Input

### Computer keyboard

Keep the existing layout as the default. The player already knows where the
typing keys are; [chord guidance](#chord-guidance) will show which ones to
try or play next without requiring a new layout to memorize.

Two optional mapping setup modes are still planned. Both produce the same
thing: a table from a physical key to an action. Guided mapping setup is
custom setup where the app picks the next note to assign. This is separate
from guided composing, which suggests chords while playing.

**Fully custom.** Press a key, it shows on screen ("Q"), then click the piano
key on screen it should play. A key can also be a control: octave up or down,
sustain, change sound, start or stop the song. Music software calls this
"MIDI learn"; the same feature later maps knobs and pads on MIDI controllers.

**Guided mapping setup.** The user picks:

- which keys: white, black, or all keys in order
- direction: low to high, or high to low
- the starting key, by clicking it on the on-screen piano (default: a C, so
  white keys give the C major scale)

Then each key press gets the next note. Details:

- Black keys default to line up with the white key mapping: the first black
  key above the lowest white key (or below the highest, going down).
- Each note plays as it is assigned, so mistakes are easy to hear.
- Done and Undo are on-screen buttons, so every keyboard key stays assignable.
- Pressing an already used key is ignored, with a hint ("Q already plays D4").

**Keyboard rules:**

- Store keys by physical position (`event.code`). Show the label the user saw.
- Ignore auto-repeat (`event.repeat`).
- Release every note when the window loses focus, and forget which keys were
  held.
- Block Ctrl, Option/Alt, Cmd/Win, Fn and Caps Lock. Space, Tab and left
  Shift play chords like any other key. While Cmd, Ctrl or
  Option/Alt is held, no key plays, so shortcuts like Cmd+Q still work.
  Pressing Cmd, Ctrl or Option/Alt also releases every note, because a Mac
  may never report a key let go while Cmd is held. Mac brightness and volume
  keys never reach the app.
- Keys pressed without Cmd, Ctrl or Option/Alt belong to Jello. The webview's
  own reactions to them are turned off, so the Mac never beeps, Space and
  Enter never press a button, and Tab never moves the focus.
- Mappings are saved as named presets. A default preset ships, with the
  hands split like on an accordion
  ([ADR 0012](adr/0012-chords-left-melody-right.md)):
  - **Left hand, chords** ([ADR 0020](adr/0020-every-chord-on-its-own-key.md)).
    All 24 major and minor chords, each on a key of its own, with roots from
    C3 to B3. A chord is one key, so ghosting never drops part of it, and no
    key changes what another plays.
    - Each letter key from A to G plays the major chord with its name (C
      plays C3 E3 G3). In touch typing, all seven are left-hand keys.
    - Each minor chord sits next to its major: one key to the left (Cm on
      X, Dm on S, Em on W, Bm on V), or one key up where that key already
      plays a chord (Am on Q, Fm on R, Gm on T).
    - The major chords on the piano's five black keys run low to high on
      the number row, the way the black keys sit above the white keys: 1
      to 5 play C#, Eb, F#, Ab and Bb.
    - Their minor chords go around the left edge, low to high: `` ` `` (top
      left) C#m, Tab Ebm, left Shift F#m, Z G#m and Space (thumb) Bbm.
    - With no song key, chords take the names chord charts use most: C#,
      Eb, F#, Ab and Bb, and C#m, Ebm, F#m, G#m and Bbm. Compose spells them
      for the song's key.
    - Before ADR 0020, holding Space played minor chords and holding right
      Shift raised the root a half step
      ([ADR 0015](adr/0015-minor-on-space-sharp-on-right-shift.md)).
  - **Right hand, melody.** Each finger plays a column of four keys, from
    the bottom row up to the number row, each a half step above the one
    below it, and the next finger's column carries on from there. The keys
    play G#3 to D#5 (N is G#3, M is middle C), one fixed note each. An
    octave is always the same row, three fingers over (M to /). The keys
    past the pinky play nothing yet.
  - A note that a chord and a melody key both play keeps sounding until
    every key playing it is let go. The backslash key (`\`) steps to the
    next wave. Right Shift runs the [looper](#looper) and Esc removes a
    layer from it.
    `-` slows the beat down by 5 and `=` speeds it up by 5. Cmd and Option play nothing, saved for layer keys that will
    switch whole mappings.
  - Before ADR 0012, the default was the accordion grid: three keys per
    finger on both hands, C3 to B4, laid out like a chromatic button
    accordion's note buttons.
- Laptop keyboards drop some 3+ key chords (ghosting). Mechanical keyboards
  with NKRO do not.

### Chord guidance

[ADR 0013](adr/0013-highlight-next-chords-on-the-existing-layout.md) set the
goal; [ADR 0017](adr/0017-guided-compose.md) is how Compose does it now.

- **Compose, guided.** Pick the song's key first, from all 24 (the 12 major
  keys, then the 12 minor keys, each spelled the usual way: Db major, not
  C# major). Trying a chord only plays it. Return adds the chord played
  last to the progression and Delete takes the last one off. Picking
  another key keeps the chords and renames them for the new key.
- **Two levels.** The chord keys are outlined by how the chord they would
  play right now fits the key: in the key (its own chords), or outside it
  but able to work, for a named reason. The rest are outside and dimmed,
  but still play. Every chord key is outlined, since each plays one chord.
  Guidance follows the key only, not the chord before, so it never ranks
  what should come next.
- **Major keys** (C major in brackets). In the key: home (C), the soft two
  (Dm), three (Em), four (F), five (G) and the sad six (Am). Can work:
  lead-ins that pull toward a chord of the key the way the five pulls home
  (D to G, E to Am, A to Dm, B to Em), and chords borrowed from the twin
  minor key (Fm, Eb, Ab, Bb). The key's own diminished chord (Bdim) is left
  out, since the chord keys play none. 6 in the key, 8 can work.
- **Minor keys** (A minor). In the key: home (Am), its bright side (C), Dm,
  the soft five (Em), the major five (E, which minor-key songs use so often
  that it counts as their own), F and G. Can work: home turned major (A), a
  major four (D), a lead-in to the five (B), and the dramatic flat two (Bb).
  7 in the key, 4 can work.
- **Names follow the key.** A chord's letter is counted from the key's home
  note, so G major's borrowed chord is Eb, not D#, and B major's lead-in is
  A#. White keys always keep their own letter (E, never Fb). Chords
  outside the key are spelled the way the key spells its black keys.
- **Reasons.** Each chord gets one plain sentence, naming the chords it
  means: "Tension that pulls back home to C." The tape's Trying shows it
  for the chord tried last.
- **Melody keys** ([ADR 0023](adr/0023-guidance-on-the-melody-keys.md)).
  Guided quietly, with no outline, so the chord keys' outlines still stand
  out. A melody key whose note is outside the key dims, and still plays. A
  major key has 7 notes (C D E F G A B in C major). A minor key has 8: its
  own 7 and the raised note below home (G# in A minor), which its major
  five (E) is built on, so the melody keys agree with the chord keys.
  While chord keys are held, the melody keys that play those chords' notes
  fill with their color, even outside the key, since a chord's own notes
  always sound right over it: G# fills while E is held in C major. The fill
  comes at once and fades out slowly when the chord is let go, like a held
  key. Holding a melody key fills nothing. Play shows none of this.
- **While playing a composed song** (step 5): the song already tells us what
  comes next, so highlight the keys for its upcoming chords as it moves
  forward. Chords played together are highlighted together.
- Highlights point at what a key already plays; they never change it. Music
  decisions stay in Rust (`src-tauri/src/harmony.rs`, `compose.rs`); the
  UI only draws them. The highlight look is not designed yet.
- Sources the rules lean on: the borrowed chords most used in major keys
  are bVI, bVII and iv, then bIII
  ([fachords](https://www.fachords.com/modal-interchanges/),
  [Violet Recording](https://violetrecording.com/borrowed-chords-explained/));
  V of V is the most common lead-in
  ([Wikipedia](https://en.wikipedia.org/wiki/Secondary_chord)); minor keys
  often swap in the major five
  ([Hooktheory](https://www.hooktheory.com/blog/minor-chord-progressions/));
  a major four in minor is the Dorian sound, and a major home is a
  Picardy third
  ([Secrets of Songwriting](https://www.secretsofsongwriting.com/?p=2334)).

### MIDI keyboard

Dropped ([ADR 0027](adr/0027-computer-keyboards-only.md)): Jello is played
on the laptop's and the desktop's computer keyboards only. Every note plays
at one loudness, since typing keys have no velocity.

## Looper

Right Shift records what the player plays and repeats it, so they can loop
chords and play a melody over them
([ADR 0021](adr/0021-a-looper-on-a-beat.md)). It works in every tab.

- **The beat:** 120 beats per minute (BPM) to start. `-` slows it by 5 and
  `=` speeds it up by 5. A bar has 4 beats for now; other bar sizes, like a
  waltz's 3, can come later. The tempo is locked from the first press until
  the loop is cleared.
- **Presses:**
  1. The first press counts in: one bar of 4 clicks to feel the tempo.
     Recording then starts on beat 1, with the click still playing.
  2. The second press closes the loop at the nearest bar line, so pressing
     a little early or late both work. A loop is at least 1 bar. It repeats
     without the click, because the loop keeps the beat, and the player can
     play live over it.
  3. The third press records a new layer on top while the loop plays
     ([ADR 0026](adr/0026-layers.md)): the click comes back and the layer
     records round and round. Notes played in one time around play from
     the next. The fourth press stops adding. Each later pair of presses
     adds another layer.
  - A loop holds at most 8 layers. A ninth press says "8 layers is the
    most, Esc removes one", and a layer stopped with nothing played is
    dropped with "Nothing played, no layer added".
  - Esc removes one layer. With two layers or more it waits half a second:
    a number from 1 to 8 on the number row picks the layer that goes,
    instead of playing its chord or note. With no number, or on a second
    Esc, the newest goes: the layer being recorded if there is one. A
    number with no layer says "There is no layer 5" and removes nothing.
    With fewer than two layers Esc acts at once: it cancels the count-in or
    the recording, or clears a loop of one layer and unlocks the tempo.
    The other layers play on, each with its own voices and volume, and
    their numbers close up.
  - Each layer has a volume from 0 to 100, set by its knob on screen. Live
    notes always play as loud as played.
- **The click:** our own code, a short tick that is higher on beat 1. It
  plays during the count-in and while recording the loop or a layer, and
  is off while the loop only plays. Its sound was kept by ear as it first
  was.
- **Snap to beat:** recorded notes move to the nearest half beat (an eighth
  note), so small timing slips disappear. A "snap to beat" toggle turns it
  on and off, and it is on by default. Choosing another grid can come later.
  - A note's start and end both snap, and a note lasts at least half a
    beat.
  - A first note a little after beat 1 moves back to beat 1. A note pressed
    up to a quarter beat before recording starts still counts, as beat 1,
    and keeps how long it was held, even with snap off.
  - A note within a quarter beat of the loop's end moves round to the next
    beat 1, for half a beat.
  - A note ends no later than the next press of the same key, so two
    presses of one key never cut each other off: the synth plays a key on
    one voice. This holds with snap off too, for a note played early and
    pressed again.
  - The loop keeps every note as it was played and snaps it as it plays
    back. So the toggle also changes a loop already recorded: off plays
    the player's own timing, on snaps it again. It changes a loop already
    playing right away: the loop's notes stop, and those that should be
    sounding start again, so a note sounding across the switch starts over
    once. It is on when the app starts and stays as set from loop to loop.
  - Live sound is never moved: a note can't sound before its key is
    pressed, and holding it back would make the keys feel slow. So the
    first time through sounds as played, and every repeat is on the beat.
  - The same toggle will cover every later way of recording, such as
    recording into Produce.
- **Inside the engine:**
  - A loop is a list of notes, not sound: each note's pitch, and the sample
    it started and ended on, counted from the loop's start. Beats follow
    from the tempo, so the AI harness
    ([ADR 0018](adr/0018-ai-harness-on-a-fork-of-pi.md)) can read loops as
    music later.
  - A clock counts samples on the audio thread. The memory for a loop is
    set aside when the engine is created, so recording never asks for more
    mid-sound.
  - Loop notes play at their exact sample, not at the start of the chunk of
    sound they fall in.
  - Loop notes and live notes stay apart: a note the loop plays and the same
    note played live never cut each other off. A note still held when the
    loop closes, or when a layer stops, is let go at the loop's end, so
    nothing keeps ringing.
  - Right Shift, `-`, `=` and Esc are key actions in the Rust key map, which
    call new commands and new C API functions.
  - The engine ignores a new tempo while the looper is busy. Rust doesn't
    even send one then: it counts the looper messages still on their way,
    so a press the engine hasn't acted on yet already locks the tempo.
  - A loop holds up to 4,096 notes. Past that, new notes still sound live
    but aren't recorded.
  - Layers share those 4,096 notes, one stretch after another, with the
    layer being recorded last. Each layer also has a slot from 0 to 7 that
    keeps its voices and volume, so removing a layer closes up the notes
    but never moves another layer's sound. The C API gains
    `jello_engine_remove_layer` and `jello_engine_set_layer_volume`.
  - While a layer records, its plan is worked out again at the start of
    each time around if new notes came in, so they play from the next one.
  - A layer's volume scales its notes, those already sounding too. A
    change eases in over 10 ms in a straight line, so it never clicks; a
    voice that is silent starts right at its volume.
  - Esc's half second is kept in Rust. Every command that shows the looper
    first checks whether it is up, so the newest layer goes at most 50 ms
    late.
  - The plan: the looper keeps the notes as played, and works out where
    each time around plays them (`plan_loop` in `snap.cpp`) when the loop
    closes and whenever the switch flips. Playback goes through the plan's
    starts in order and its ends in order. Notes moved round to beat 1 go
    first, and notes starting together go by their end, pitch and
    velocity, so the plan never depends on the order they were played in.
    The ends are sorted, since a chord held under a quicker note ends
    after it, and snapping can change their order too. Sorting works in
    place, so the audio thread asks for no memory.
  - Half beats are worked out from the beat's two numbers, a minute's
    samples and the tempo, and each starts on the whole sample before. So
    the last one never falls after the loop's end.
  - Recording starts exactly one bar after the count-in's first beat, since
    the tempo is locked, so the looper knows when from the first press and
    can record notes played just before. A note's end is its start plus how
    long its key was held.
  - A note pressed again while held, say by a second chord key, is recorded
    as an end and a fresh start, which sounds the same.
  - Losing focus lets go of live notes only; the loop plays on.
- **Delay:** the player plays along with what they hear, so a loop recorded
  over Bluetooth headphones lands late. Snapping hides small delays, but
  Bluetooth's are often bigger than a quarter beat, so notes would snap to
  the half beat after. Built-in speakers or wired headphones keep loops
  tight. Moving recorded notes earlier by the measured delay can come later.
- **On screen** ([ADR 0024](adr/0024-the-looper-s-look.md), lane
  redesigned in [ADR 0025](adr/0025-the-looper-s-lane-as-wide-as-the-keyboard.md),
  layers in [ADR 0026](adr/0026-layers.md)): built. It shows in every tab,
  in two places: right Shift (`ui/LooperFace.svelte`) and the lane
  (`ui/LooperLane.svelte`).
  - Right Shift says what the looper is doing. Four beat lights: the beat
    now is lit, and during the count-in the beats already counted stay half
    lit. Beside them, a short state: "looper, 120 bpm", "count-in 3", "rec
    bar 2", "loop 2 bars" or "rec layer 2", with a red square while
    recording. A 3 px bar along the key's bottom edge fills as the loop
    goes round: red while recording and lavender while looping. While the
    first loop is recorded its length isn't known yet, so the bar fills
    once per bar. The bar never crosses the key's text.
  - A lane under the keyboard draws the loop. It is as wide as the
    keyboard's panel, like Compose's tape, and as tall as the room under
    the keyboard less 10 px above and below: 52 px in the 1100x720 window.
    - Ready: one dashed box across the lane saying "Right Shift records a
      loop", and the tempo at the right end.
    - Count-in: a big 1 2 3 4 spread across the box, lit one per beat.
    - Recording: the box is dashed and marked out in 4 bars, and fills
      red up to the playhead. The bar being recorded shows at the right
      end.
    - Looping: one box per layer, with its knob on its left. Each note is
      a row of dots in its note color, low notes at the bottom of its box,
      and bar lines split the box. Each box has its own playhead; the
      layers are one length, so the playheads move together. The loop's
      length shows at the right end.
    - Boxes sit side by side: one fills the lane, and up to four share its
      width. From the fifth on, four sit on a level and the levels share
      the lane's height: two levels of about 24 px, for the 8 layers a
      loop can hold.
    - A layer being recorded is a dashed red box after the others, spread
      over the loop's length, and fills red up to its red playhead.
    - Each knob is a ring of 9 dots, 22 px across (20 px once boxes
      stack), with the layer's number in the middle. The dots light up to
      its volume. Dragging up turns it up, 100 px from 0 to 100; two
      fingers on the trackpad turn it too, and a double click sets it back
      to full. While it turns it glows, follows the hand rather than
      Rust's answer, and the right end shows "Vol 2" and its volume, until
      a moment after it stops. Holding it never takes the focus, so keys
      still play. Notes dim with their layer's volume but never quite
      vanish, and at 0 the number dims.
    - While Esc waits, every knob's number lights, the box that goes turns
      dashed red, and the right end says "Remove layer 3".
    - The snap switch sits at the right end, under the loop's length,
      styled like the Wave button: "Snap", then "on" in lavender or "off"
      in grey.
      It is always shown, so snap can be set before recording, and a click
      changes a playing loop at once. A key for it may come later.
  - Messages are typed out in the top bar, after the title, with the
    blinking cursor as before. A long one wraps onto a second line inside
    the top bar, and past that is cut short. They never squeeze the
    buttons.
  - Where the beat is: after each chunk of sound the engine shares how
    many samples into the count-in, the recording or the repeat playing
    now the looper is, next to its state. Rust turns that into beats at
    the tempo (`beats_in`). Rust reads the two one after the other, so
    right as the loop closes it can see the new state with the old place;
    every change is on a bar line, so the place within the bar is the same
    either way, and while looping Rust wraps it into the loop's length.
  - The loop's notes: the looper sends Rust each change to them through a
    second mailbox, the other way from the first: a note started, a note
    ended, the loop closed and kept only its first notes, the loop was
    cleared, or a layer started, stopped, was dropped or was removed. Each
    change says which layer. Rust applies them in order to its own copy
    each time the page asks, and sends the notes in beats. The mailbox
    holds 4,096 changes; Rust empties it every 50 ms while the looper is
    busy, so it never fills.
  - While looping, Rust asks the engine where each time around plays the
    notes (`jello_engine_plan_loop`, the same function the looper plays
    by) and sends those, so the lane draws every note where it is heard.
    While recording, it sends them as played.
  - The page asks Rust again every 50 ms while the looper is busy. In
    between, like the tape's playhead, it counts on from Rust's last answer
    by its own clock every frame, at the tempo. It also starts the loop
    over by itself, and holds the count-in on beat 4 until Rust says
    recording has started.
  - Esc says "clear" while there is something to clear. `-` and `=` say
    "slower" and "faster".
  - The snap switch: Rust has a `switch_snap` command and says whether snap
    is on (`snap`). A click on the switch sends `switch_snap` and draws
    Rust's answer.
- **Built in slices,** each explained and approved before it is written:
  1. The clock and the click. Done, and the tick was kept by ear as it
     first was. Right Shift turned the click on and off until slice 2.
  2. Record and loop, and Esc. Done.
  3. Snap to beat and its switch. Done, with the lane rebuilt to ADR 0025.
  4. Layers, with Esc removing one layer and a volume knob on each
     ([ADR 0026](adr/0026-layers.md)). Done.
  Each slice adds its own placeholders.

## Sound

Jello is a flexible toolbox, not a genre machine: any instrument, any chain
of effects, anything can modulate or automate anything. Each artist's sound
becomes presets and templates built on top.

| Artist | Signature sound | What Jello needs |
|---|---|---|
| YOASOBI | Bright layered synths, piano, pumping mix | Supersaw, sampled piano, sidechain |
| Ado | Explosive vocals, fast rock arrangements, heavy distortion | Distortion, parallel compression, vocal doubling and harmonies, fast tempo and key changes |
| Pink Floyd | Spacey synths, repeating synth patterns, long echoes, organ, sound-effect loops, long songs | Analog-style synth with a step sequencer, tape delay, long reverbs, organ with a rotary speaker effect, loop playback |
| Massive Attack | Slow heavy beats, deep sub bass, dark dusty textures, dub echoes | Sampler that slices loops, swing, lo-fi effects, dub delay |
| Radiohead | Glitchy electronics, odd time signatures, processed vocals, gliding leads | Granular synth, stutter and reverse effects, vocoder, odd meters, glide, flexible modulation |

**v1 sound set:** subtractive synth (stacked detuned oscillators for supersaw,
filters, envelopes, LFOs, presets), sampler, EQ, compressor, reverb, delay,
saturation, sidechain, master limiter. Everything else arrives through the
artist sound studies.

Sampling other people's records is a copyright problem for anything published.
Use our own recordings or licensed sample packs.

## Vocals (v1.5)

Audio tracks hold audio clips instead of notes.

1. **Import first.** A singer may record elsewhere and send files. Drop in a
   WAV, place it anywhere, trim, fade.
2. **Record.** Needs a mic and an audio interface. Compensate for latency so
   takes line up. Keep multiple takes and combine the best parts.
3. **Vocal chain.** EQ, compression, de-essing, reverb, delay.
4. **Live pitch correction.** Detect the pitch, snap it to the song's key,
   shift it in real time. Gentle for smoothing, hard for the robotic effect.
5. **Hum to notes.** The same pitch detector turns humming into notes.

Later: note-by-note editing like Melodyne (v2, one of the hardest problems in
the project) and AI voice conversion on the GPU (v3, only with voice models
we have rights to).

## Learning mode (v2)

Falling notes on screen, colored by hand, with the keys lighting up.

| Level | Two-hand skill | Example material |
|---|---|---|
| Beginner | One hand at a time, then hands taking turns | 5-finger melodies, white keys only |
| Basic | Hands together: same rhythm, or left hand holding long notes | Folk tunes over long bass notes |
| Intermediate | Left hand keeps a steady pattern while the right hand plays a different rhythm | Sonatina style: broken chords, waltz bass |
| Advanced | Two independent lines, syncopation, chords in both hands | Bach inventions, ragtime, pop arrangements |
| Master | Full independence: 3+ voices, 2-against-3 rhythms, hand crossings | Fugues, Chopin-level pieces |

- **Sync is measured.** The song knows which hand each note belongs to, so
  matching played notes gives per-hand timing ("left hand 60ms late").
- **Together window.** The main difficulty knob: how far apart notes that
  should sound at once may land (about 150ms at Basic, 40ms at Master).
  Tempo rises only after accuracy and sync are solid.
- **Practice tools:** wait mode (only moves on when both hands land together),
  hands separate (the app plays the other hand), loop a few bars with gradual
  speed-up.
- **Timing:** one master clock in Rust, input timestamped where it arrives,
  latency calibration like rhythm games.
- **Content:** exercises generated by AI, plus MIDI files of public-domain
  pieces. MIDI files are messy: hands may share a track, tempo maps must be
  read, timing may not be quantized.
- It is played on the computer keyboard
  ([ADR 0027](adr/0027-computer-keyboards-only.md)), so only offer pieces
  whose notes fit the layout's range and whose chords the keyboard can read
  at once.

## AI (v3)

```text
 Jello UI      in-app AI panel      MCP server (Claude, other agents)
     \               |                   /
      +--------------+------------------+
                     v
           COMMANDS (one list, defined once)
```

- The AI uses commands, never clicks the UI.
- The AI can "play keys" as one more input source.
- The AI writes songs as text (the song format), and later as scripts that
  generate it.
- Offline render lets the AI render a song, check it (loudness, clipping) and
  try again.
- The in-app harness is a fork of pi's coding agent
  ([ADR 0018](adr/0018-ai-harness-on-a-fork-of-pi.md)), set up the way
  PDE's Seelie is. It helps compose along the way, and turns the user's
  trial-and-error sketch (key, progression, loops) into the final song as
  code: write, render a draft, listen, fix, render the final.
- How the harness reaches models is decided when it is built: CLIProxyAPI
  on the user's subscriptions (as in the user's other projects), or the
  user's API key in the OS keychain. A local model on the GPU is a later
  option. Whether a model can hear renders needs a test first.

## Performance and system check

**The CPU makes the sound, the GPU does the visuals and AI.** Live audio must
finish every buffer within a few milliseconds; a GPU round trip adds delay and
jitter, and one late buffer is an audible click.

- CPU: process audio in small blocks with plain loops (the compiler turns them
  into SIMD). Tracks are independent, so they can later run on separate cores.
- GPU: visuals (spectrum, waveforms, falling notes) and AI (local models,
  transcription, pitch analysis, voice conversion).

**System check:**

1. Read the specs (CPU, cores, RAM, GPU) to show the user and pick defaults.
2. Benchmark the engine: render a burst of voices and effects silently, time
   it, and set safe limits.
3. Find the lowest safe latency: try smaller audio buffers on the real device
   and keep the smallest one that never glitches.
4. Keep watching: a live DSP load meter. Near the limit, warn or lower quality
   instead of crackling.

## Platforms

| | Laptop | Desktop |
|---|---|---|
| OS | macOS (Apple Silicon) | Windows 11 (assumed) |
| Hardware | | Ryzen 7 9800X3D, RTX 5070, 32 GB RAM |
| Audio | CoreAudio | WASAPI low-latency or exclusive; ASIO later with an audio interface |
| Webview | WebKit (no Web MIDI) | WebView2 |

CI (`.github/workflows/ci.yml`) builds and tests both platforms on every
push to `main` and every pull request, so one machine never silently breaks
the other. It cannot hear anything: sound still gets checked by ear on each
machine.

## UI design process

The user is very particular about UI, so UI is its own long, iterative track,
not a side task of each feature.

Stage uses dark tiles, the 12-pixel dot grid and one permanent color for
each note letter ([ADR 0014](adr/0014-one-color-per-note-family.md)):
C lavender (#BFA5FF), D peach (#FFBC9A), E mint (#8BE6C2), F butter (#F5DD8C),
G pink (#FF91C4), A ice blue (#A9D8F5), and B pearl (#EAE4DB). Sharps share
their note letter's color across octaves. Chord keys and beams use the
root's color; the readout colors each note. The dev toggle, alternative
themes and saved theme selection are removed. Development and packaged
builds use the same colors, with no palette name shown in the app.

Still to design: the Produce screen (a dev panel for now,
[ADR 0016](adr/0016-play-compose-and-produce-tabs.md)), the look of the
guidance outlines on the chord keys, and the tab icons (a pencil for
Compose, faders for Produce). Compose's tape (below) is built.

1. Explore directions with mockups before any UI code.
2. Settle a design system with the user (colors, type, spacing, components).
3. Design one screen or component at a time; build it only after approval.
4. Until a screen is approved, it is a plain "dev panel" placeholder that only
   exposes the feature for testing.

### Compose screen

The tape look, [ADR 0022](adr/0022-compose-as-a-tape.md), picked over
chord cards and a dot-matrix readout. It sits above the keyboard, as wide as
the keyboard's panel. At the default window (1100x720) there are only
157 px between the tabs and the keyboard, so the approved stacked mockup was
refitted side by side (approved 2026-10-20): about 125 px tall, 16 px of
room above and below, and it still fits at 1000x640.

- **Rough timing.** The user composes by feel: not exact, and not fully on
  the beat. Each chord keeps the whole length it was held, in seconds, and
  is never snapped to the beat.
- **Top row.** Key on the left, Song next to it, Trying on the right: each
  a small Doto label over its value. Chord and key names keep their case,
  as on the keys ("Am", "Db major"); only the labels are capitals.
- **Room.** 16 px above the tape and below it. In a short window both
  shrink alike; in a tall one only the room below grows, so the tape stays
  under the tabs. The keyboard never moves.
- **Key.** Before a key is picked, it says "Pick a key" and the picker is
  open: two rows of 12, major above and minor below, each name in its home
  note's color (by pitch, so Db is C's purple and Bb is A's blue, like the
  keys). The picker sits in the song lane's place, exactly its height, and
  the hint line is hidden while it is open. Once picked, it folds into a
  readout ("C major" and a chevron); clicking that opens the picker again,
  and clicking it once more closes it.
- **Song.** The number of chords and the total length ("4 chords, 9.2 s"),
  or "No chords yet". Song and Trying show once a key is picked.
- **Trying.** The chord tried last, in its root's color, with its reason
  ("Em in the key. Gentle and a little sad. It often moves on to Am."), or
  "Play a chord to see how it fits." before any. Too long, it ends in
  "..." on its one line.
- **Song lane.** A play button and one block per chord: as wide
  as it was held, at the same width per second across the whole song,
  outlined and tinted in its root's note color, with its name and length.
  When the song outgrows the lane, it scrolls to keep the newest chord in
  view, with no scrollbar (a trackpad or Shift and the wheel scroll it
  back). The play button stays put; only the blocks scroll. Playing never
  scrolls the lane.
- **Names are never cut off.** Every block, the growing one too, is at least
  as wide as its name and length. On the mockup's scale (36 px a second)
  that is about 1.1 s, so shorter chords look a little longer than to
  scale; the seconds shown always give the true length.
- **Holding.** While a chord is held, a dashed block grows at the end.
  Return adds the chord being held, or the one just let go of; added while
  held, it keeps growing until it is let go. Delete takes the last chord
  off.
- **Hearing it.** Clicking a block lights it and plays that chord for its
  own length, with a playhead sweeping across. The play button plays every
  chord in turn.
- **Empty song.** A dashed lane: "Your song shows up here. Play a chord,
  then press return to add it." The play button comes with the first
  chord.
- **Timing.** Rust times each chord (`src-tauri/src/compose.rs`): from its
  key going down to it coming up, read as each command starts, so waiting
  for a lock never counts. Losing the window's focus ends every held chord.
  The page gets each chord's length in milliseconds and whether it is still
  held, and where the playhead is: which chord and how many milliseconds
  into it. Rust works that out from the song's position in the engine and
  the lengths it sent. Each chord also carries its root (0 to 11) for its
  color, and `holding` is the chord held right now that isn't in the song
  yet: the dashed block. The page asks again after every key press and
  let-go, and every 50 ms while the song plays; in between, it grows held
  blocks and moves the playhead by its own clock, so they glide.
- **Build slices.** 1: Rust keeps each chord's length (done; the dev panel
  shows it). 2: a song player in the engine plays one chord, or the whole
  song, each for its own length, on its exact samples (done). 3: the tape
  screen replaces the dev panel (done, `ui/ComposeTape.svelte`).

## Open questions

Ask the user one at a time, when the step that needs the answer comes up.

- Is a level a version of a song (5 difficulties per song) or a tier of the
  course (different material per level)? Leaning: tiers.
- Design system for screens after the start page. They follow the start
  page's Stage look (ADR 0010); the details are settled screen by screen
  (see [UI design process](#ui-design-process)).
- Song file format details. Decide at step 5.
- Where the sampler's recordings come from: free sample libraries whose
  licenses allow releasing the music, or our own recordings. Decide with the
  sampler's design (ADR 0019).
