# Jello design

This is the living design for Jello. Decisions that are settled also have an
ADR in [adr/](adr/README.md). When this file and an ADR disagree, the ADR wins
and this file should be fixed.

## Vision

Jello is a desktop synthesizer and music studio with three long-term goals:

1. **Make real music.** Full songs at the level of the user's references:
   YOASOBI, Ado, Pink Floyd, Massive Attack, Radiohead.
2. **Learn to play.** A follow-along mode that trains both hands to play in
   sync. Sync is the goal; speed is secondary.
3. **Make music with AI.** An AI can drive Jello from outside (MCP server) or
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
- Block Shift, Ctrl, Option/Alt, Cmd/Win, Fn and Caps Lock. Mac brightness and
  volume keys never reach the app.
- Mappings are saved as named presets. A default preset ships: Z row is the
  lower octave (S D G H J are black keys), Q row is the upper octave
  (2 3 5 6 7 are black keys).
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

CI builds both platforms on every push so one machine never silently breaks
the other.

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
- C++ engine design and audio library. Walk through with the user before
  writing any engine code.
