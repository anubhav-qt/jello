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
2. **Laptop synth.** Play notes with the computer keyboard. Key mapping
   (custom and guided). On-screen piano.
3. **MIDI keyboard input.** Device picker, velocity, sustain pedal, hot-plug.
4. **Sound.** A deep synth and a sampler (for piano and drums).
5. **Arrange.** Tracks, sequencer, piano roll, recording from the keyboard,
   tempo and key changes, the text song format.
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
   plays the default preset, "/" switches waves and the dev panel shows the
   current wave. Then tune the voice level, wave trims and envelope by ear.

After the slices: pick the frontend framework (with an ADR), then key mapping
and the on-screen piano.

### v1.5: vocals

Audio tracks, importing and recording vocals, a vocal effects chain, live
pitch correction and hum-to-notes. See [Vocals](#vocals-v15).

### v2: learning

Follow-along mode, levels, AI-generated exercises, note-by-note vocal editing.
See [Learning mode](#learning-mode-v2).

### v3: AI composer

MCP server, in-app AI harness, AI voice conversion. See [AI](#ai-v3).

### After v1, in any order

- **Artist sound studies.** Pick one reference artist, build the modules that
  sound needs, make a track in that style.
- **Guitars.** Their own section: recording into audio tracks, amp and cabinet
  simulation, pedal effects, a tuner. Synths never try to imitate guitars.
- **Third-party plugins (AU/VST3).** Maybe. A big feature on its own.

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

1. The live path (key or MIDI to sound) never goes through the webview.
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
  set wave, sample rate, render.
- **Files:**
  - `engine.cpp`: the C API and all device code.
  - `mailbox.*`: carries note and wave messages to the audio thread, without
    locks.
  - `synth.*`: the instrument. At the start of each chunk of sound it acts on
    every waiting message, then adds up its voices and puts the sum through
    the soft clip. It also picks the voice for each new note, and remembers
    the wave new notes play.
  - `voice.*`: plays one note on one of the four waves. The wave trim, the
    velocity curve and the voice level set how loud it is.
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
  voice still ringing that same note, or else a free voice. When all 32 are
  busy, it takes over the quietest voice fading out, or else the note held
  the longest. Every key press gets the next number from a counter, which is
  how the engine knows which note is oldest.
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
- **Soft clip:** after the voices are added up, samples between -0.8 and 0.8
  pass unchanged. Louder ones are bent toward 1.0 without ever reaching it,
  so the sound never clips. The master limiter takes over this job in step 6.
- **Offline render:** Rust makes a second engine with no speakers and calls
  render, the same code the speakers hear. Rust writes 32-bit float WAV files
  (`hound` crate) to `Music/Jello/`.
- **Rust side:** `src-tauri/src/engine.rs` is the safe wrapper,
  `commands.rs` holds the commands, `wav.rs` writes files.
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
  Tests never open speakers.

## Input

### Computer keyboard

Two mapping modes. Both produce the same thing: a table from a physical key
to an action. Guided mode is custom mode where the app picks the next note.

**Fully custom.** Press a key, it shows on screen ("Q"), then click the piano
key on screen it should play. A key can also be a control: octave up or down,
sustain, change sound, start or stop the song. Music software calls this
"MIDI learn"; the same feature later maps knobs and pads on MIDI controllers.

**Guided.** The user picks:

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

**Rules for both modes:**

- Store keys by physical position (`event.code`). Show the label the user saw.
- Ignore auto-repeat (`event.repeat`).
- Release every note when the window loses focus.
- Block Shift, Ctrl, Option/Alt, Cmd/Win, Fn and Caps Lock. While Cmd, Ctrl
  or Option/Alt is held, no key plays, so shortcuts like Cmd+Q still work.
  Mac brightness and volume keys never reach the app.
- Mappings are saved as named presets. A default preset ships: the Z row
  plays C3 to B3 (S D G H J are the black keys) and the Q row plays C4 to B4
  (2 3 5 6 7 are the black keys), so Q is middle C. "/" steps to the next
  wave.
- Laptop keyboards drop some 3+ key chords (ghosting). Mechanical keyboards
  with NKRO do not.

### MIDI keyboard

- Target hardware: 61 keys, full-size, velocity sensitive, sustain pedal jack,
  USB class compliant.
- 61 keys are usually MIDI notes 36 to 96. The app knows the keyboard's range.
- Handle velocity, sustain pedal (CC64), pitch bend, mod wheel, hot-plug, and
  release all notes when a device disconnects.
- Testing without hardware: IAC Driver (macOS), loopMIDI (Windows).

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
- A 61-key keyboard limits some Master-level pieces; only offer pieces that
  fit the keyboard's range.

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
- The in-app harness uses the user's API key, stored in the OS keychain.
  A local model on the GPU is a later option.

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

1. Explore directions with mockups before any UI code.
2. Settle a design system with the user (colors, type, spacing, components).
3. Design one screen or component at a time; build it only after approval.
4. Until a screen is approved, it is a plain "dev panel" placeholder that only
   exposes the feature for testing.

## Open questions

Ask the user one at a time, when the step that needs the answer comes up.

- Is a level a version of a song (5 difficulties per song) or a tier of the
  course (different material per level)? Leaning: tiers.
- UI direction and design system. A long, mockup-first process (see
  [UI design process](#ui-design-process)).
- Frontend framework. The skeleton uses plain TypeScript; decide at step 2.
- Song file format details. Decide at step 5.
- Laptop keyboard only, or a MIDI keyboard too? Laptop keys have no velocity
  and drop some chords. Decide after playing step 2 for a while. The answer
  shapes step 3 and the learning mode (v2).
