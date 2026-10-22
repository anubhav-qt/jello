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
   Started with a low-pass filter on every voice, with its own envelope
   ([ADR 0028](adr/0028-a-low-pass-filter.md)), then unison: up to 7
   detuned copies of each note's wave, which makes the saw a supersaw
   ([ADR 0029](adr/0029-unison.md)). Both belong to each song, in its own
   sound profile ([ADR 0030](adr/0030-a-sound-profile-per-song.md)), along
   with its loudness envelope, key tracking and tuning, and the song keeps
   its own tempo. Then 8 LFOs and a routing list of 64 connections that
   can move every sound setting
   ([ADR 0031](adr/0031-lfos-and-a-routing-list.md)), with left Shift as
   the vibrato key ([ADR 0032](adr/0032-the-vibrato-key.md)); see
   [Engine](#engine). The sound panel in Compose sets the profile
   ([ADR 0033](adr/0033-the-sound-panel.md)); it replaced the dev panel,
   and every other placeholder screen goes the same way. Then 3
   oscillators per note, each a sine, triangle, square, saw or noise with
   its own level and tuning
   ([ADR 0035](adr/0035-oscillators-with-kinds.md)). Then four filter
   types, a width for the square (a pulse) and glide on a held left
   Option ([ADR 0036](adr/0036-glide.md)).
5. **Arrange.** Tracks, sequencer, piano roll, recording from the keyboard,
   tempo and key changes, the text song format. Guided composing came
   early: the Compose tab already builds a progression with guidance
   ([ADR 0017](adr/0017-guided-compose.md)), which ranks the best next
   chords ([ADR 0034](adr/0034-best-next-chords.md)), and lights the
   song's chords on the same keyboard as it plays back. See
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
   Done: rules in [ADR 0034](adr/0034-best-next-chords.md), look in
   [chord guidance](#chord-guidance).
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
  set oscillator (with its width), start click, stop click, set tempo, press looper, clear
  loop, set snap, looper status (with its place), next loop change, plan
  loop, play song, stop song, song position, set filter (with its type),
  set filter envelope, set unison, set loudness envelope, set key tracking,
  set tuning, set LFO, set connection, set vibrato key times, set vibrato
  key, set glide key, set glide time, sample rate, render.
- **Files:**
  - `engine.cpp`: the C API and all device code.
  - `mailbox.h`: carries messages from one thread to another without
    locks. One carries note, oscillator, click, tempo, looper, snap, song,
    sound, LFO, connection, vibrato key and glide key messages to the
    audio thread;
    another
    carries the loop's changes back to Rust.
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
    each new note, and remembers the oscillators, filter envelope and
    unison new notes play. It holds the routing list and works it out every 32
    samples (see LFOs and the routing list below), and each player's
    melody for the glide key (see [Glide](#glide)). It adds into the
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
  - `voice.*`: plays one note on its 3 oscillators, by up to 7 copies of
    them, in stereo (see Oscillators and Unison below). The trims, the
    oscillators' levels, the velocity curve and the voice level set how loud
    it is. It knows who started its note: the player (live), the looper or
    the song player, and for the looper, which layer. A loop note also plays
    at its layer's volume (see [Looper](#looper)). Everything a note starts
    with and keeps until it ends comes in one `NoteSound`: its oscillators,
    loudness envelope, filter envelope, key tracking, copies and tuning. The
    synth keeps one for new notes, and each setter changes its part.
  - `oscillator.*`: the oscillators' kinds, settings and ranges, and where
    a new song's start.
  - `wave.*`: the four wave shapes, with their jumps and corners rounded
    off, the square's width, and the trims that make them equally loud.
  - `noise.*`: white noise, a new random height every sample, and its trim.
  - `unison.h`: unison's controls and their ranges.
  - `stereo.h`: one moment of sound for both speakers.
  - `filter.*`: each voice's filter, one per speaker, its four types, its
    filter envelope, and the glides that keep a moved control from
    clicking (see Filter below).
  - `envelope.*`: a note's loudness over time: a rise (attack), a dip to
    the held level (decay, sustain) and a fade out after letting go
    (release). Each song sets its own stages (see Sound profile below). The
    filter envelope is one too, with stages of its own.
  - `lfo.*`: the LFOs' settings and shapes, and the random heights.
  - `routing.*`: the routing list, and everything notes share: the shared
    LFOs, each player's vibrato key and key presses.
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
- **Oscillators** ([ADR 0035](adr/0035-oscillators-with-kinds.md)): every
  note is played by 3 oscillators, added up before the filter. Each has a
  kind, a level from 0 to 100%, a width that only the square hears (see
  [Pulse width](#pulse-width)), and a tuning from the note: up to 3
  octaves, 12 half steps and 100 cents either way, added up in cents.
  - The kinds are sine, triangle, square, saw and noise, in that order, so
    the routing list steps through them round and round. Later kinds, like
    wavetables, come after noise.
  - A copy's oscillators all start their cycles together when the engine
    starts, so like waves at the same pitch add up instead of cancelling:
    two halves of a triangle are exactly one whole.
  - An oscillator at 0% is skipped, and its level glides in straight lines
    like the other live settings. A new engine plays oscillator 1 as a
    triangle at 100% and the other two as silent triangles, all on the
    note, so it sounds exactly as one wave did, sample for sample.
  - A change only reaches notes started after it: each voice keeps the
    oscillators its note started with. A voice taken over on new kinds
    glides across with the gap, like any other takeover.
  - The C API ignores an index or value out of range; Rust brings its own
    within them.
- **Noise:** a new random height every sample, spread evenly from -1 to 1,
  so every pitch at once: a hiss. It has no pitch, so an oscillator's
  tuning does nothing to it. Its heights come from SplitMix64 (the same
  mixing step as the LFOs' random heights), started from the note's press
  number, the copy and the oscillator. So every render sounds the same, and
  no two copies or oscillators hiss alike: 7 copies, turned down by √7,
  stay as loud as one, and spread makes the hiss wide. Its RMS is the
  triangle's already, so its trim is 1, a starting value to tune by ear.
- **Waves:** sine, triangle, square and saw. All four go from -1 to 1 and
  start each cycle rising from zero, so they line up.
  - The square and saw jump, and the triangle has corners. Played as they
    are, their harmonics too high for the sample rate would fold back down
    as stray, off-key tones (aliasing), worst on high notes. So the samples
    on each side of a jump are bent (PolyBLEP), and so are those on each
    side of a corner (PolyBLAMP). At A7, every stray tone below 5,000 Hz is
    at least 1,800 times quieter than the note.
  - Trims make every wave as loud as the triangle, measured by RMS (the
    square root of the average squared height): sine 0.8165, triangle 1,
    square 0.5774, saw 1. Starting values, to be tuned by ear.
- **Unison** ([ADR 0029](adr/0029-unison.md)): every note is played by 1 to
  7 copies of its oscillators at once, each tuned a little apart and placed
  between the speakers. They drift in and out of step, which makes the
  sound thick and shimmering. A saw in unison is the supersaw.
  - The copies sit evenly from -1 to 1: copy i of n at 2i / (n - 1) - 1,
    and a lone copy at 0, right on the note and in the middle.
  - Detune, from 0 to 50 cents (hundredths of a half step), tunes each
    copy its place times the detune away from the note: the lowest copy
    the detune below, the highest as far above.
  - Spread, from 0 to 100%, pans each copy its place times the spread
    toward a speaker: the lowest left, the highest right. Its left side
    gets √(1 - pan) and its right √(1 + pan), so the two squared always
    add up to 2: a copy is as loud wherever it sits, and in the middle
    each side gets exactly 1, as a voice did before unison.
  - Their sum is turned down by 1 / √copies. Copies that drift apart add
    up like unrelated sounds, so a note stays about as loud: 7 copies at
    20 or 50 cents are within 1% of one. At 0 detune they never drift,
    and partly cancel where they started: 7 copies are then about 6 dB
    quieter than one.
  - 7 copies can peak at about 2.6 times one copy's height (√7), so loud
    chords in unison reach the soft clip sooner.
  - When the engine starts, each voice's copies start a step of 0.618 of
    a cycle (the golden ratio's share) apart, wrapped back into one cycle,
    so they never start lined up as one loud wave. Copy 0 starts at 0, so
    a lone copy starts where a voice always did. Each copy then runs on
    from wherever it is, so every note starts a little differently.
  - Each voice is stereo now. Each side goes through a filter of its own,
    moved together, and the gap that keeps a takeover from clicking is
    worked out for each side.
  - A change reaches the notes started after it, like the oscillators: a
    held note keeps its copies. A note that takes over a sounding voice
    gets the new copies and glides across with the gap.
  - A new engine starts at 1 copy, which is off, with detune 20 and spread
    100% waiting: notes sound exactly as before, sample for sample. Values
    outside their ranges are ignored by the C API and brought within them
    by Rust.
- **Filter** ([ADR 0028](adr/0028-a-low-pass-filter.md)): every voice puts
  its wave through a filter of its own, before the loudness envelope, so a
  quiet note is filtered just like a loud one: a low-pass, high-pass,
  band-pass or notch, the song's own (see [Filter types](#filter-types)).
  It is a
  state variable filter, 12 dB per octave, built the zero-delay-feedback
  way from Andrew Simper's published math (Cytomic), so it stays steady
  even when its cutoff moves on every sample.
  - The cutoff control sets middle C's cutoff, from 20 Hz to 20,000 Hz.
    Every other note's follows it by the song's key tracking, from 0 to
    100%: at 100%, where a new song starts, an octave higher for each
    octave up, so every note is equally bright; at 0%, every note has
    middle C's cutoff, so higher notes sound darker. A note keeps the key
    tracking it started with. Cutoffs move in octaves, not Hz, and never
    go past 21,600 Hz (0.45 of the sample rate), where the math stops
    behaving.
  - At its type's open end, 20,000 Hz for a low-pass and 20 Hz for a
    high-pass, the filter is open: the wave skips it and comes out
    exactly as it went in. A band-pass or notch is never open. The filter still runs underneath, so closing it
    mid-note starts it from where the wave is. Closing and opening fade
    between the two over 20 ms.
  - Resonance goes from 0 to 100%, squared on the way, so Q goes from
    0.7071 (no bump) to 10 (a strong peak). Squaring keeps the first half
    gentle.
  - Cutoff and resonance are live: every voice, even one sounding, glides
    to a moved value in a straight line over 20 ms, so a jump never
    clicks.
  - The filter envelope opens the cutoff by its amount, from 0 to 8
    octaves, at its peak, then falls to its sustain, a share of the
    amount, and closes over its release after letting go. Each stage takes
    0 to 10,000 ms. A note keeps the envelope it started with.
  - A note on a silent voice starts right at its own cutoff, with nothing
    left over from the voice's last note. A note that takes over a
    sounding voice keeps its filter's memory and glides from the old
    note's cutoff to its own, so it never clicks.
  - A new engine starts as an open low-pass, with resonance 0 and an
    envelope amount of 0 (attack 5 ms, decay 300 ms, sustain 0, release 300 ms), so it sounds
    exactly as before the filter. Values outside their ranges are ignored
    by the C API and brought within them by Rust.
- **LFOs and the routing list**
  ([ADR 0031](adr/0031-lfos-and-a-routing-list.md)): every song has 8 LFOs
  (low frequency oscillators: slow waves) and a routing list of 64
  connections, which say what moves what.
  - Shapes, each from -1 to 1 (-100% to 100% on the screen): sine,
    triangle, square, saw up, saw down, random steps (a new random height
    each cycle, held), smooth random (gliding from one to the next in half
    a cosine) and steps (up to 32 heights you set, one after another).
  - Per note, every note starts its own copy as it starts, at the LFO's
    start point plus a random share of its spread. Shared, one runs for
    every note: in Hz from the engine's start; in beats from beat 1 when
    the click or the looper starts the beat, gliding there over 20 ms,
    and lined up again on every beat after. "Once" runs a per-note LFO one
    cycle, then holds it where it ended, like an extra envelope.
  - Speed in Hz (0.01 to 100), in beats a cycle (1/16 to 128, at the song's
    tempo, the beat running or not), or in key presses a cycle (1 to 32):
    each press moves on a step, and notes starting on the same sample are
    one press, so a chord is one. Each player counts its own presses: live
    play, the song (from 0 each time it plays) and each loop layer. Shared
    in presses, every note of that player slides to the press's step over
    the smoothing's share of a beat.
  - Delay and fade-in, 0 to 10,000 ms (0 to 128 beats in beats): the LFO
    waits, then grows to its full size in a straight line. Smoothing, 0 to
    100%, is the share of the time between jumps spent sliding instead; it
    only touches the square, both saws, random steps and steps.
  - Random heights come from SplitMix64 on the note's press number, the
    LFO's number and the cycle, so each note and each LFO moves its own way,
    and the same notes always give the same samples.
  - A connection has a source, a target, a signed amount in the target's own
    unit, both ways (from minus the amount to the amount) or one way (from 0
    to the amount), snap (pitches only, the note's and each oscillator's:
    whole half steps, sliding between them) and an optional second source it
    is scaled by, read one way. Sources: the 8 LFOs, the vibrato key, the
    note's height (-1 five octaves below middle C, 1 five above), a random
    number per note, and where the loudness and filter envelopes are. The
    envelopes are read as each 32 samples start, so their moves come 32
    samples late.
  - Targets, live while a note sounds: pitch (cents, up to 4,800 either
    way), loudness (only down; -100% is silence), pan, cutoff (octaves, up
    to 10), resonance, key tracking, detune (cents, up to 50), spread, both
    sustains, the filter envelope's amount (octaves, up to 8), the
    filter's type (whole steps round low-pass, high-pass, band-pass and
    notch, up to 3), each LFO's speed (octaves, up to 14), each
    oscillator's level, each oscillator's pitch (cents, up to 4,800 either
    way) and each oscillator's width (percent, up to 90). Read once as each
    note starts: each oscillator's kind (steps round sine, triangle,
    square, saw and noise, up to 4), the copies (up to 6), the six
    envelope times and the glide time (octaves of time, up to 14). Moves on one
    target add up, and the sum is kept within the setting's range. A
    connection moving the cutoff brings an open filter in.
  - Every 32 samples, counted from the engine's first, each sounding voice
    works out its moves and heads to them in straight lines, so a render
    is the same in any chunk size. A note starts right where its moves put
    it. A shared LFO's speed is moved only by shared LFOs, with no fade-in,
    since every note shares it.
  - A changed LFO or connection reaches notes already sounding from the
    next 32 samples on. A shared LFO glides from where it was over 20 ms,
    so it never clicks; one that only now becomes shared starts where the
    change puts it.
  - A new engine's LFOs (each a per-note sine at 5.5 Hz, spread 100%,
    smoothed 10%) and connections (none) move nothing. Values outside
    their ranges are ignored by the C API and brought within them by Rust,
    in `modulation.rs`, which also names every shape, speed, source and
    target for the screen and later song files.
  - The sound panel's LFOs and Connections pages set them through
    `set_lfo` and `set_connection`, and its Notes page sets the vibrato
    key's times through `set_vibrato_key_times`.
- **Vibrato key** ([ADR 0032](adr/0032-the-vibrato-key.md)): left Shift.
  Held, it rises from 0 to 1 in a straight line over its rise time; let
  go, it falls back over its fall time, each 0 to 10,000 ms, the song's
  own (0 jumps). Each player has its own: live play, the song (always up
  until step 5) and each loop layer, so it only moves that player's notes.
  The looper records its presses as notes on pitch 128, one past the
  highest, and plays them back on the layer's own key. The glide key is
  built the same way, on pitch 129 (see [Glide](#glide)). Letting go of every
  note, as when the window loses focus, or removing a layer lets go of its
  key too.
- **Sound profile** ([ADR 0030](adr/0030-a-sound-profile-per-song.md)):
  how a song's notes sound belongs to the song, so no setting is tuned to
  one value for every song. Its sound profile
  (`src-tauri/src/sound_profile.rs`) holds the oscillators, the loudness
  envelope, the filter and filter envelope, key tracking, unison, tuning,
  the LFOs, the routing list, the vibrato key's times and the glide time,
  and the song in
  Compose
  (`compose.rs`) holds the profile.
  - Every change, from the sound panel or the backslash key, goes to the
    profile, which sends it on to the engine and keeps it as the engine
    sent it, with values outside their ranges brought within them. Rust's
    engine wrapper keeps no sound of its own, and the screen reads the
    sound from the profile.
  - As the app starts, the song sends its tempo and its whole profile,
    before the first note: 84 messages, which arrive in order.
  - A new song starts where a new engine does: one triangle oscillator at
    full level with the other two silent, every width at 50%, a 5 ms attack
    held at the full peak with a 300 ms release, an open low-pass, no
    filter envelope, full key tracking, 1 copy, a 100 ms glide, A4 at
    440 Hz and 120 beats per minute. It also has vibrato ready: its first connection moves the
    pitch 25 cents either way with LFO 1, scaled by the vibrato key, which
    rises and falls over 250 ms. With the key up, it sounds exactly like a
    new engine. It is only a starting point.
  - The loudness envelope's attack, decay and release each take 1 to
    10,000 ms; at 0 ms a stage would jump the loudness in one sample, which
    clicks. Its sustain goes from 0 to 100% of the peak. Tuning sets A4's
    pitch from 415 to 466 Hz, about a half step either way (further is a
    key change), and every other note keeps its distance from A4. Both, and
    key tracking, reach the notes started after a change, like the
    oscillators: a held note keeps its own.
  - The song keeps its tempo too, though it is not sound: `-` and `=` change
    the song's tempo, which goes on to the engine, and the song keeps the
    tempo the engine plays, so while the looper is busy it stays put.
  - Some values stay the same for every song, since they are about how the
    engine works, not how a song sounds: the 5 ms takeover gap, the 10 ms
    volume ease, the 20 ms filter glide, the wave and noise trims, where the
    copies start, the 32 voices, 48 kHz, the filter's limits and the click.
    The voice level and the soft clip become each track's volume and the
    master limiter in step 6.
  - Play plays the open song's profile. Song files keep it from step 5,
    and so do copying a profile from another song and a profile per track.
- **The sound panel** ([ADR 0033](adr/0033-the-sound-panel.md)): in
  Compose, the Sound button opens it over the tape, with pages for the
  oscillators, the notes, the filter, the LFOs and the connections. Every
  change goes to the song's sound profile and its readouts show Rust's
  answer. Nothing is
  saved until song files: each launch starts a new song, at 1 copy, A4 at
  440 Hz, with the filter open. See [Sound panel](#sound-panel).
- **No clicks:** a new song's notes fade in over 5 ms and out over 300 ms,
  and no stage is shorter than 1 ms. A note that
  takes over a busy voice picks up from wherever that voice's wave and
  loudness were, so the sound never jumps. If the new note plays another
  wave, that wave starts at a different height. The gap between them is
  added back and shrinks to nothing over 5 ms, so the sound glides across.
  A silent voice has nothing to glide from, so it gets no gap.
- **The beat:** at the song's tempo, from 40 to 240 beats per minute; a new
  song starts at 120. A beat lasts
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
  notes, `sound_profile.rs` holds a song's sound, `modulation.rs` the LFOs
  and connections, `wav.rs` writes files.
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
  For the oscillators, they check that a new engine plays one triangle,
  that each oscillator plays its own distance from the note, that they add
  up at their levels, that a silent one changes not a single sample, that
  noise has no pitch, is as loud as the triangle and never hisses alike
  twice yet sounds the same on every render, that three oscillators glide
  across to three others without a click, that snap rounds an
  oscillator's pitch, and odd values, Rust's edges and all.
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
  For the filter, they check that an open filter leaves every sample as it
  was, that each harmonic of a saw comes through as much as the analog
  filter's math says (within 1%), 12 dB quieter an octave up, that key
  tracking makes A3, A4 and A5 equally bright, that A7's cutoff at 7040 Hz
  lands right on its pitch, the boost at the cutoff for
  each resonance, the 20 ms glide on notes already sounding, a full sweep
  with no click, full resonance staying steady and below 1.0 with all 32
  voices, the extremes staying finite and ending in exact silence, a note
  on a silent voice starting right at its cutoff, a takeover with no click,
  the same samples in any chunk size, the filter envelope's attack, decay,
  sustain and release, that a held note keeps its envelope, and odd values.
  For unison, they check that 1 copy ignores detune and spread, that 2
  copies fully apart play the detune below on the left and above on the
  right, that 7 copies sit evenly apart in pitch and between the speakers,
  that with no spread both speakers play the same, that each side of 2
  copies fully apart is a lone copy through its own filter, that copies
  never start lined up, that 7 copies are as loud as 1 within 1 dB, a
  takeover with no click, that the filter closes on both sides and opening
  leaves both as they were, that a silent voice keeps nothing of its last
  note's filter on either side, that a layer's volume turns down both
  speakers, that a held note keeps its copies while the next note gets the
  new ones, the same samples in any chunk size, and odd values.
  For the sound profile, they check that a new song starts where a new
  engine does and sounds just like one, that switching steps the first
  oscillator through the kinds in order and the engine plays the one it
  lands on, that setting an oscillator plays it, that the profile
  keeps values as the engine sent them, that every change reaches the
  engine, that sending a profile sends every part (leaving any one out
  changes the sound), and that the backslash key switches the first
  oscillator's kind.
  For the loudness envelope, key tracking and tuning, they check every
  sample of a shaped note through all four stages against the envelope's
  math, that a held note keeps its envelope and its pitch while the next
  note gets the new ones, how far A4's cutoff follows middle C's at 100,
  50 and 0% key tracking, that tuning moves every note with A4, odd values,
  Rust's ranges, and that a new engine starts where a new song does. For
  the song's tempo, they check that `-` and `=` step by 5 and stop at 40
  and 240, that the song keeps the tempo the engine plays while the looper
  locks it, and that sending the song sends its tempo and sound profile.
  For LFOs and the routing list, most tests compare A4 moved by a
  connection with A4 alone: a note turned down is exactly the note times
  its gain, so every sample is checked against the gain the LFO's drawing
  says, in straight lines between each 32 samples. They cover each of the
  8 LFOs, every shape as drawn, smoothing, random steps and smooth random,
  random heights the same every render but different per note and per LFO,
  per note against shared, spread, delay and fade-in, once, speeds in
  beats and at another tempo, a shared LFO lining up with beat 1 and every
  beat after a tempo change, even partway through a block, presses (a
  chord is one, shared ones sliding, the song counting from its start),
  pitch in cents, loudness and pan, every live target against the same
  setting set directly, every target read at a note's start sample for
  sample and with no fade-in, LFO speeds, every other source, the vibrato
  key's rise and fall, scaled by, moves adding up, the pitch's limit,
  snap, the song and the loop each on its own key, the loop playing the
  key back sample for sample, losing focus, a changed LFO reaching
  sounding notes and a shared one gliding, moves of nothing changing no
  sample, the busiest list never blowing up, any chunk size, odd values,
  Rust's ranges matching the engine's edge for edge, and a new engine
  starting where Rust says. The sound profile tests add a new song's
  vibrato, measured cycle by cycle, and the commands tests left Shift,
  left Control, left Option, the names the screen reads and the lane
  leaving out both keys' presses.
  The key map tests check the melody keys finger by finger and each chord,
  that other keys do nothing, that left Option holds the glide key until
  it is let go, and that letting go always stops the notes a
  key started, even after the layout changes, except a note another held
  key still plays. Tests never open speakers.
  For the pulse width, the filter types and glide, see their own parts
  below.

### Pulse width

The square has a width: how much of each cycle it spends up, from 5% to
95% of the cycle. At 50% it is the square it always was, sample for sample;
narrower or wider it is a pulse, thinner and more nasal. It is each
oscillator's own, the song's, and starts at 50%.

- Only the square hears it. Sine, triangle, saw and noise ignore it, so the
  screen dims it for them.
- The square rises at the cycle's start and falls at the width. Both jumps
  are rounded off with PolyBLEP, so a thin pulse stays as clean on high
  notes as the square.
- The wave is moved down by `2 * width - 1`, so it is balanced on the
  middle line at every width and never pushes the speakers one way.
- It keeps the square's trim, so narrower pulses are naturally quieter:
  their RMS is `2 * sqrt(width * (1 - width))` of the square's. A 25% pulse
  is about 1.2 dB quieter and a 5% pulse about 7 dB quieter. The trim is a
  starting value, to be tuned by ear.
- A change reaches notes started after it, like the rest of an oscillator.
  A connection to "osc N width" moves it live, in percent, while a note
  sounds, so an LFO gives pulse width modulation (a slow, chorus-like
  sweep). The move is kept within 5% and 95%.
- The C API ignores a width outside 5% to 95%; Rust brings it within them.
- Tests check each width's harmonics against the pulse's math (a 25% pulse
  has no 4th, 8th or 12th harmonic), that a 5% pulse stays balanced, that
  narrower pulses are quieter by the formula, that thin pulses on A7 keep
  stray tones quiet (so the fall is rounded off at the width), that only
  the square hears it, that a change reaches only new notes, and that the
  routing list moves it live, matching the width set directly.

### Filter types

The filter has four types, each 12 dB per octave, taken from the state
variable filter's own parts, so one filter runs whatever the type:

- **Low-pass** ("takes the top off"): the low part. Open at 20,000 Hz.
- **High-pass** ("takes the bottom off"): the high part. Open at 20 Hz.
- **Band-pass** ("keeps one band"): the band part as it is, so it peaks at
  Q at the cutoff: resonance makes the band narrower and louder. Never open.
- **Notch** ("cuts one band out"): the low and high parts added, so it
  takes out the cutoff and keeps the rest; resonance makes the notch
  narrower. Never open.

How the types behave:

- The type is the song's own, and a new song starts on low-pass, so it
  sounds as before. Picking a type never moves the cutoff or resonance.
- It is live: changing it reaches notes already sounding, fading from the
  old type to the new one over 20 ms, so it never clicks. Opening and
  closing fade over the same 20 ms.
- A connection to "filter type" moves it in whole steps round the four
  types (up to 3), with the same fade.
- Key tracking, the filter envelope and the cutoff's connections work the
  same for every type. A connection moving the cutoff brings an open
  filter in, at either end.
- The C API ignores a type that is not one of the four; Rust names them
  "low-pass", "high-pass", "band-pass" and "notch", and refuses any other
  name.
- Tests check each type's curve against the analog filter's math, the
  boost or notch at the cutoff for each resonance, each type open only at
  its own end, the 20 ms fade on notes already sounding, no click going
  through every type, and the routing list stepping the type round both
  ways.

### Glide

[ADR 0036](adr/0036-glide.md): hold left Option (left Alt on Windows) and
each melody note slides to its pitch from the last one, instead of
jumping.

- **Melody notes.** A note started on its own is a melody note. Notes that
  start together are a chord, and a chord never glides. So the engine can
  tell, a note started while its player holds the glide key waits until
  every note of its moment has come in, then starts on the same sample it
  would have anyway.
- **Bend or slide in.** If the last melody key is still held, its voice
  bends to the new note: one note, no new attack. If it was let go, the new
  note starts with its own attack and slides in from the last note's
  pitch, from wherever that one was along its own slide.
- **Coming back.** Held keys come back: let go of the newest key and the
  voice bends back to the newest one still held, gliding while the glide
  key is held and jumping once it is let go. A note let go meanwhile no
  longer comes back.
- **Time.** Each glide moves in a straight line through the cents and takes
  the song's glide time whatever the distance: 0 to 10,000 ms, 100 ms in a
  new song (0 jumps). A connection to "glide time" moves it in octaves of
  time, read as each glide starts. The filter's key tracking follows the
  gliding pitch.
- **Players.** Each player glides on its own: live play, the song and each
  loop layer, each with its own key and melody. The looper records the key
  as notes on pitch 129, like the vibrato key's 128, and the lane leaves
  them out. Letting go of every note, or removing a layer, lets go of its
  key and forgets its melody.
- **Keys.** Option/Alt is no longer a shortcut key in Jello: it no longer
  blocks keys or lets go of every note. Right Option still plays nothing.
- Tests check the bend with the key held, the slide in after letting go,
  bending back (gliding with the key held, jumping without), the same time
  whatever the distance, 0 ms jumping on one voice, the cutoff following
  the glide, chords never gliding, notes jumping as before without the key,
  the routing list moving the time, letting go of everything, odd times,
  and a loop playing its own glide key back.

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
- Block right Option/Alt, Cmd/Win, Fn and Caps Lock. Space, Tab and left
  Control play chords like any other key, left Shift holds the vibrato
  key and left Option (left Alt on Windows) holds the glide key. While Cmd
  is held, no key plays, so shortcuts like Cmd+Q still work. Pressing Cmd
  also releases every note, because a Mac may never report a key let go
  while Cmd is held. Control and Option/Alt are no shortcut keys in Jello
  ([ADR 0032](adr/0032-the-vibrato-key.md),
  [ADR 0036](adr/0036-glide.md)). Mac brightness and volume keys never
  reach the app.
- Keys pressed without Cmd belong to Jello. The webview's
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
      left) C#m, Tab Ebm, left Control (bottom left) F#m, Z G#m and Space
      (thumb) Bbm. F#m was on left Shift until it became the vibrato key
      ([ADR 0032](adr/0032-the-vibrato-key.md)).
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
    every key playing it is let go. The backslash key (`\`) steps the song's
    first oscillator to its next kind, and left Shift, held, brings in
    whatever the song connects to the vibrato key: vibrato in a new song.
    Left Option, held, makes melody notes glide (see [Glide](#glide)).
    Right Shift runs the [looper](#looper) and Esc removes a layer from it.
    `-` slows the beat down by 5 and `=` speeds it up by 5. Cmd and right
    Option play nothing, saved for layer keys that will switch whole
    mappings.
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
- **Four levels** ([ADR 0034](adr/0034-best-next-chords.md)). Best next:
  up to three chords that follow the song's last chord well (below). Then
  how the chord the key would play right now fits the key: in the key (its
  own chords), or outside it but able to work, for a named reason. The rest
  are outside and dimmed, but still play. Every chord key shows its level,
  since each plays one chord. A best next chord is always in the key or can
  work, never outside. Before ADR 0034, guidance followed the key only.
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
- **Best next** ([ADR 0034](adr/0034-best-next-chords.md), approved
  2026-10-22). After each chord added to the song, guidance ranks what
  comes next from the song's last chord only: up to three chords, strongest
  first, each with its own reason. Trying a chord never changes the
  ranking; adding or deleting one does. Before the first chord it shows the
  best starts, and after a chord outside the key, home. Strongest means
  strongest pull: the counts decide, and where they are close or missing,
  a root falling a fifth or rising a step comes first, and so does the
  order of jobs (away, then tension, then home). Trying shows the best-next
  reason for a chord that is one, and the key's reason otherwise. The look
  is below the tables. Major keys (C major):

  | After | Best next | Why |
  |---|---|---|
  | Nothing yet | C, F, Am | home is the usual start; F starts away, as the royal road does; Am starts on the sad side |
  | C | F, G, Am | the most common move away from home; tension that wants to come back; the sad side |
  | Dm | G, C, F | falls a fifth to the tension chord; straight back home; climbs and stays soft |
  | Em | Am, F | the end of the royal road; steps up (93% of songs go to one of these) |
  | F | C, G, Fm | home, the most common move of all; tension, the royal road's next step; F turns minor, as in "Creep" |
  | G | C, Am, F | settles the pull home; a surprise landing, as in the four chords of pop; steps down, the rock move |
  | Am | F, Dm, G | the most common move after Am; falls a fifth, round the circle to C; steps down toward home |
  | D, E, A, B | G, Am, Dm, Em | each lead-in falls a fifth to the chord it leads into |
  | Fm | C, G | a tender way home; on to the tension chord |
  | Eb | Ab, F, C | falls a fifth, staying in the borrowed colors; back in the key; home |
  | Ab | Bb, C | a heroic climb on to C; straight home, big and warm |
  | Bb | C, F, Ab | steps up home, the rock ending; falls a fifth; steps down, darker |
  | Outside the key | C | going home settles it |

  Fm after F comes from theory: the counts below treat F and Fm as one
  chord. After G, Am comes before F by pull, though rock uses F more. Minor
  keys (A minor), ordered by pull and jobs, since the counts are mostly of
  major-key songs:

  | After | Best next | Why |
  |---|---|---|
  | Nothing yet | Am, F | home is the usual start; many minor-key pop songs start on F |
  | Am | F, Dm, G | the loop Am F C G; falls a fifth, darker still; steps down, starting Am G F E |
  | C | F, Am, G | falls a fifth; back home, as in "Eye in the Sky"; on round the loop |
  | Dm | E, G, Am | away, then the strongest tension; falls a fifth; straight home, as in "The Chain" |
  | Em | Am, F | falls a fifth home, gently; steps up |
  | E | Am, F | the strongest pull home; a surprise landing instead |
  | F | G, C, E | the climb F G Am, and the royal road's next step; round the loop; slips down to the tension chord |
  | G | Am, C, Em | steps up home, the rock ending; falls a fifth to the bright side; the royal road F G Em Am |
  | A | Dm | A major is Dm's own five |
  | D | Am, E, G | back home, the sway of "Breathe"; up to the tension chord; falls a fifth |
  | B | E | falls a fifth to the tension chord |
  | Bb | E, Am | down to the tension chord; slides a half step down home |
  | Outside the key | Am | going home settles it |

  After G in a minor key, Em (the royal road) was picked over F (Am G F
  E). In a major key, G does not light Em, since its pull home is stronger.
- **The look** (built 2026-10-22). Each chord key draws its level in its
  own note color. Best next: the key lights up, the strongest most, with
  three squares in its top right corner lit like signal bars (three for
  the strongest, one for the third). In the key: a solid outline. Can
  work: a dashed outline. Outside: dimmed. A best next key keeps its
  outline, so it still shows how it fits. Trying reads like "Am in the
  key, best next." and then the best-next reason. The hint under the tape
  says "Keys: lit is best next, solid in the key, dashed can work, dim
  outside."
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
- **While the song plays** (built 2026-10-22, ahead of step 5). The song
  already says what comes next. The key for the chord playing glows as if
  held, and the key for the chord after it fills up from the bottom in its
  color, with a bright edge on top, full as that chord starts. The last
  chord fills nothing, and neither does a chord clicked on the tape, since
  it plays alone. Best next waits until the song stops; the outlines stay.
  The hint under the tape says "Keys: glowing plays now, filling plays
  next." Rust finds the keys that play each chord (`now_keys` and
  `next_keys` on the playhead), so chords played together would light
  together; today a song plays one chord at a time.
- Highlights point at what a key already plays; they never change it. Music
  decisions stay in Rust (`src-tauri/src/harmony.rs`, `compose.rs`); the
  UI only draws them.
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
  Best next leans on the jobs of chords
  ([Open Music Theory](https://viva.pressbooks.pub/openmusictheory/chapter/intro-to-harmony/)),
  chord-to-chord counts and root motions in 100 rock songs
  ([de Clercq and Temperley 2011](https://rockcorpus.midside.com/2011_paper/declercq_temperley_2011.pdf),
  Tables 3, 6 and 7), Em going to Am or F 93% of the time
  ([Hooktheory](https://www.hooktheory.com/blog/i-analyzed-the-chords-of-1300-popular-songs-for-patterns-this-is-what-i-found/)),
  the chords minor-key songs use most
  ([Hooktheory](https://www.hooktheory.com/blog/minor-chord-progressions/)),
  J-pop's royal road
  ([Wikipedia](https://en.wikipedia.org/wiki/Royal_road_progression)), the
  four chords of pop
  ([Wikipedia](https://en.wikipedia.org/wiki/I%E2%80%93V%E2%80%93vi%E2%80%93IV_progression))
  and the Andalusian cadence
  ([Wikipedia](https://en.wikipedia.org/wiki/Andalusian_cadence)).

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
    music later. The vibrato key's presses are notes too, on pitch 128, so
    they are recorded, snapped and played back like any note
    ([ADR 0032](adr/0032-the-vibrato-key.md)); the lane leaves them out
    until its design says how to draw them.
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

### Presets

[ADR 0037](adr/0037-presets.md): a song's whole sound, saved by name.

- **Files.** `Music/Jello/Sounds/<name>.sound`, TOML, written by
  `src-tauri/src/sound_file.rs`: `tuning_hz` and `glide_ms`, then 3
  `[[oscillators]]`, `[loudness]`, `[filter]` (with `key_tracking`),
  `[filter_envelope]`, `[unison]`, `[vibrato_key]`, 8 `[[lfos]]` with all
  32 steps, and a `[[connections]]` for each one in use. Choices are words,
  the names the UI shows. Saving then reading gives back exactly the same
  sound.
- **Reading.** Starts from a new song's sound and takes what the text gives.
  Left out, the wrong sort, or an unknown word: a new song's value. Unknown
  names: skipped. Counts below 0 read as 0, fractions round. Out of range:
  kept until the song takes the sound, when the engine brings them in, as
  with any change. Connections fill the routing list from the top in the
  file's order, leaving out any whose source or target Jello does not know,
  and none listed means none. Text that is not TOML can't be read.
- **Names.** Spaces at the ends are left off. Letters, numbers, spaces, `-`
  and `_`, 1 to 40 of them; never CON, PRN, AUX, NUL, COM1 to 9 or LPT1
  to 9 in any case. Saving under a name that differs only in case replaces
  that preset and keeps its name.
- **Page.** Presets, the sound panel's sixth page
  (`ui/SoundPresets.svelte`). Tiles sorted by name, upper and lower case
  together, scrolling. Clicking one loads it; a file that can't be read is
  a dimmed tile saying so. The tile the song sounds exactly like is lit:
  Compose remembers the profile right after loading or saving and compares
  (`Composer::lit_preset`), so changing a value back lights it again. Each
  tile's START mark, faint until hovered, white when marked, makes it the
  starting sound; clicking it again unmarks it. At the right, a name field
  and Save, which reads Replace over an existing name; the hint under it
  gives Rust's reason a name is refused, what it saves over, or where
  presets are kept. The page lists the folder again each time it opens.
- **Typing.** While a text field has the focus, keys go to it and play
  nothing (`ui/main.ts`). Esc and Return leave the field. Let-gos still
  reach Rust, which ignores keys that started nothing.
- **Starting sound.** `starting_sound = "<name>"` in `settings.toml` in
  Jello's settings folder (on a Mac, `~/Library/Application
  Support/com.jello.app`). As Jello starts, before the window opens, the
  song takes that preset; with none marked, or it gone or unreadable, it
  keeps a new song's sound. If the folders can't be found, Jello still
  opens.
- **Commands.** `preset_list`, `load_preset`, `save_preset`,
  `preset_name_problem` and `mark_starting_preset`; the files themselves in
  `src-tauri/src/presets.rs`. `SoundView.preset` names the lit tile.
- **Later.** Renaming and removing on the page; song files writing their
  sound the same way.
- Tests check the round trip, taking a sound setting every part of the
  song's, words in the text, only used connections written, missing, wrong-sort, unknown and out-of-range values, unknown
  connections moving up, the 64 limit, broken text, allowed and refused
  names, saving, listing order, hidden and other files, replacing in another
  case, unreadable files, names reaching outside the folder, the starting
  mark (one at a time, other settings kept, broken settings), the lit tile
  and its name reaching the screen, and the starting sound reaching the engine, or a new song's when its
  preset is gone, unreadable or unmarked.

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
- **Todo: the harness gives each song its starting sound.** Every sound
  setting is the song's own ([ADR 0030](adr/0030-a-sound-profile-per-song.md)),
  so the harness can help set them as a song begins:
  - **Starting a song by talking.** The user explains the song they have in
    mind, and the harness sets its first sound profile (oscillators, filter,
    unison and whatever comes next) from that. It is only a starting point:
    the user changes it from there.
  - **Voice mode in Compose.** The user sings or hums the idea of a tune.
    The harness plays it back a few different ways, each its own reading
    of what was meant, so the user can pick the closest and say what to
    change, by voice, until it is what they were trying to make.

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
[ADR 0016](adr/0016-play-compose-and-produce-tabs.md)) and the tab icons (a
pencil for Compose, faders for Produce). Compose's tape, its chord guidance
and the sound panel (below) are built.

Until 2026-10-22, each screen was explored with mockups and built only
after approval. Since then, UI is built straight in the settled look
(Stage, note colors, white values, lavender only for what is picked, no
pills) and checked in the real app; the user asks for changes while using
Jello. Until a screen is built, it is a plain "dev panel" placeholder that
only exposes the feature for testing.

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
  chord in turn, and the keyboard lights them as they come
  ([chord guidance](#chord-guidance)).
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

### Sound panel

[ADR 0033](adr/0033-the-sound-panel.md): it replaces the sound's dev
panel. The mockups were approved one decision and one page at a time
(2026-10-22).

- **Where.** Compose only. A Sound button at the right end of the tabs (a
  small wave and "Sound", lavender while open) opens it in the tape's
  place: 16 px under the tabs, as wide as the keyboard's panel and 320 px
  tall at fullscreen. The keyboard and the looper's lane stay in view, and
  every key keeps playing. Page tabs along its top: Oscillators, Notes,
  Filter, LFOs, Connections and Presets ([presets](#presets)). It opens on Oscillators, and a note at the
  right end says when changes are heard.
- **Values.** Each one is a name in small capitals, its number in Doto with
  the unit dimmer, and a row of dots lit from the first up to the value.
  Drag along the dots, or click one to jump there. A row lights
  `1 + round(share * (dots - 1))` dots, along the value's own curve: times
  go as `10000 ms * share^3`, so short times, where small changes are
  heard, get most of the row; the cutoff and an LFO's speed in Hz go in
  octaves; cents of a connection's pitch go as a cube too.
- **Picking.** Clicking a value (its name, number or dots), dragging it, or
  a drawing's square picks it: its frame and name turn lavender, its square
  is ringed, the drawn keyboard's arrow keys are framed, and the page's
  note becomes the value's name with "step" and "ten steps" beside the
  arrows. Left and right step it once, up and down ten times, and a held
  arrow keeps stepping. A step is 1 in the value's unit (ms, %, Hz, cents,
  copies, presses, steps), 0.1 for octaves, the shown digit for an LFO's
  Hz (0.01 below 1 Hz, 0.1 below 10, then 1), and the next note length for
  beats (1/16 to 128). Clicking empty panel space, another page or the
  Sound button lets go. The arrows play nothing and never reach Rust; Esc
  keeps its looper job.
- **Look.** The keyboard's dark panel, dotted lines between its parts, Doto
  labels and square corners. Drawings are dotted white lines on a faint
  grid of dots, with square handles. Drawings, numbers and lit dots are
  white; lavender marks only the open page, the picked value and chosen
  tiles. A setting that would do nothing is dimmed to a third.
- **Oscillators.** Three side by side, each "Oscillator 1" to 3 with its
  kind's name as the hint (and the `\` key on oscillator 1, which still
  steps it). A drawing of what it plays around a faint middle line, as
  tall as its level, showing 2 cycles on the note and twice as many each
  octave up (from 0.5 to 16), so the tuning shows too; noise draws a fixed
  scribble. Its square, at the right end, drags the level; a square's
  drawing has a second one on its fall, which drags the width sideways.
  Under it, 5 kind tiles. Then level and width, the width dimmed unless
  the kind is a square. On the row below, octaves, half steps and cents
  with dots growing either way from a middle zero, written as bare
  numbers since their names say the unit (with units, "-12 half steps"
  and "-100 cents" would not fit side by side), dimmed while the level is
  0 or the kind is noise; a silent oscillator's drawing is dimmed too.
  Note: "Changes start with the next key you press".
- **Notes.** Loudness ("how each
  note rises and fades"): the envelope drawn, KEY UP where the key comes
  up, each stage up to 90 px on the time curve; the peak's square drags
  the attack, the next corner the decay sideways and the sustain up and
  down, the KEY UP square the sustain, the last one the release; then
  attack, decay, sustain and release. Unison ("copies of each note"): a
  picture of the copies, not draggable, spread left to right and detuned
  up and down; copies (7 dots, one per count), detune and spread. Tuning:
  A4 in big digits, 415 to 466 Hz. Under it, vibrato rise and vibrato
  fall, then glide time, with no headings or hints of their own, since
  the drawn keyboard marks left Shift VIBRATO and left Option GLIDE (the
  column had no room for both keys' headings). Note: "Changes start with
  the next key you press".
- **Filter.** Filter, its hint saying what the type does ("takes the top
  off", "takes the bottom off", "keeps one band" or "cuts one band out"):
  the type's real curve from 20 Hz to 20 kHz with the resonance's bump or
  notch, and a faint curve, ENVELOPE PEAK, where the envelope takes the
  cutoff. Its one square drags the cutoff sideways and the resonance up
  and down (a notch's only sideways); at a low-pass's far right or a
  high-pass's far left the cutoff reads "open". Under it, four type tiles,
  each with its curve. Picking one keeps the cutoff and resonance. Filter envelope ("opens it as each note starts"):
  drawn like loudness, with its peak and sustain in octaves; amount (0 to 8
  octaves), attack, decay, sustain and release. Key tracking ("follows the
  note"): 0% and 100% as faint lines, the song's line lit through middle C
  and a square at its right end. Note: "Cutoff and resonance move notes
  already sounding".
- **LFOs.** The list: all 8, each with its number, its shape and what it
  moves (from the connections); unconnected ones are dim, and a click opens
  one. Its shape ("LFO 1 shape", the shape's name as the hint): two cycles
  drawn, its start a solid square and, per note, its spread a shaded band
  ending in a hollow square ("each note starts in the shaded part"). The
  steps shape draws its steps instead, a square on each to drag its height
  and one at the right end to drag how many there are (1 to 32). Under it,
  8 shape tiles. Speed: [Per note | Shared], [Hz | Beats | Presses], then
  speed, start, spread and smooth. Delay, fade-in: the wobble waiting, then
  growing, with a square at the end of each; delay and fade-in, in ms or,
  while the speed is in beats, beats; [Keeps going | Once]. Dimmed: smooth
  unless the shape jumps (square, both saws, random steps, steps), and
  spread and once while shared. Note: "Changes here move notes already
  sounding".
- **Connections.** The list ("6 of 64"): only connections in use, numbered
  in order, as "LFO 1 -> pitch" (an arrow on screen); a long one ends in
  "...", and many scroll. The open one is framed, with an x that removes it.
  "+ New connection" fills the first free place: the first LFO nothing uses
  yet, moving the pitch by 0, so nothing changes until its amount does.
  Moved by: the 8 LFOs (number and shape), the vibrato key, note height,
  random and both envelopes. Moves: every target, in four groups (Notes,
  Oscillators, Filter and LFO speed) whose names sit two by two above one
  group's targets at a time. Opening a connection shows its target's group;
  clicking a name shows another. The shown name is lavender with dots under
  it, like the page tabs, and the one holding the target is white. The hint
  says "moves while it sounds" or "read as each note starts". A shared LFO's
  speed is dimmed unless a shared LFO moves it. How much (in the target's
  unit): a picture of the move, not draggable; the amount, its dots growing
  either way from a middle zero; [Both ways | One way]; "Snap to notes",
  dimmed unless the target is a pitch, styled like the looper's snap switch.
  Scaled by: nothing, the 8 LFOs and the other sources. Note: "Changes here
  move notes already sounding".
- **Sending.** Each change sends its whole group, through `set_oscillator`,
  `set_loudness_envelope`, `set_unison`, `set_tuning`,
  `set_vibrato_key_times`, `set_glide_time`, `set_filter`,
  `set_filter_envelope`,
  `set_key_tracking`, `set_lfo` or `set_connection`, and the panel shows
  Rust's answer. Steps start from the value Rust answered last.
- **Code.** `ui/SoundPanel.svelte` holds the frame and the page tabs; a
  component per page (`ui/SoundOscillators.svelte`, `ui/SoundNotes.svelte`,
  `ui/SoundFilter.svelte`,
  `ui/SoundLfos.svelte`, `ui/SoundConnections.svelte`), one for a value
  (`ui/SoundValue.svelte`) and one for a drawing's square
  (`ui/SoundHandle.svelte`). What is picked lives in `ui/Stage.svelte`, so
  the arrow keys (sent from `ui/main.ts` to `stepPicked`) and the framed
  arrow keys reach it. The curves, steps and drawings' math are in
  `ui/sound-scales.ts`; the look is in `ui/sound-panel.css`.

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
