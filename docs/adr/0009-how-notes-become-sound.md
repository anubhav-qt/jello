# 0009: How notes become sound

- Status: Accepted; fixed envelope superseded by 0030, Ctrl as a shortcut key by 0032, one wave per note by 0035
- Date: 2026-10-19

## Context

Step 2 turns the test tone into an instrument. A key press has to become a
note almost at once, many notes have to sound together, and the audio thread
must never lock, allocate memory or wait (design rule 2). We designed this
one piece at a time before writing any engine code.

## Decision

**From key to engine.**

- The webview sends the physical key (`event.code`). It skips auto-repeat and
  ignores keys while Cmd, Ctrl or Option/Alt is held, so shortcuts still work.
- Rust's key map turns keys into note events (ADR 0002). Laptop keys always
  use velocity 100. Rust remembers which note each held key started, so
  letting go stops the right note even if the map changed meanwhile. All
  notes are released when the window loses focus.
- The C API gets note on (note, velocity), note off (note), all notes off and
  set wave. Playing a note, releasing it, releasing all notes and switching
  the wave are named commands (ADR 0003).

**The mailbox.**

- Only the audio thread touches the voices. Everything else sends it events
  through a mailbox: a ring of 1024 slots and two atomic counters. The writer
  puts an event in a slot, then moves the write counter. The audio thread
  reads the event, then moves the read counter. Rust keeps the engine behind
  a Mutex, so there is only ever one writer.
- If the mailbox is full, new events are dropped.
- The audio thread empties the mailbox at the start of every chunk of audio
  (about every 10 ms), so a note starts at the next chunk. Offline render
  empties it the same way, so renders stay deterministic (design rule 6).
- We write the mailbox ourselves (about 50 lines) instead of adding a library.

**Voices.**

- 32 voices, made once when the engine is created.
- A voice knows its note (or that it is free), its wave, phase, envelope and
  velocity, and when it started.
- Pressing a note that is still ringing reuses its voice.
- When every voice is busy, a new note takes over the quietest voice that is
  releasing, or else the oldest one that is held. The voice keeps its phase
  and its current loudness, so taking it over never clicks.
- Note off starts the release, and the voice is free once it is silent. All
  notes off releases every voice. Notes always fade out, never cut off.

**The envelope.**

- ADSR made of straight lines. Starting values: attack 5 ms, decay 300 ms,
  sustain at 70% of the peak, release 300 ms.
- One set of settings shared by all voices. Each voice has its own place in
  the envelope.
- Every stage moves in a straight line from the current level, so a stolen
  or re-pressed voice never jumps. Straight lines reach exactly zero, so a
  voice knows when it is free and tests can check for exact silence.
- Velocity scales the whole shape.

**Waves.**

- Sine, triangle, square and saw. Square and saw use PolyBLEP, so high notes
  do not alias.
- One key steps to the next wave: sine, triangle, square, saw, then sine
  again. Jello starts on triangle.
- A voice keeps the wave it started with. Only new notes use a new wave.

**Volume.**

- Each voice plays: wave × wave trim × velocity gain × envelope × voice level.
- Wave trims make the four waves equally loud.
- Velocity gain is (velocity / 127)², the standard MIDI curve.
- One voice level for all voices. It starts with one note a bit quieter than
  the step 1 test tone, so a 4-note chord is about as loud as that tone.
- All voices are added together. The same sum goes to both speakers.
- A soft clip leaves everything below 0.8 untouched and bends louder peaks so
  they never reach 1.0. The master limiter (step 6) takes over this job.

## Consequences

- Playing notes never makes the audio thread lock, allocate or wait.
- A note sounds about 10 to 20 ms after the key press, to be measured.
- Up to 32 notes sound at once. Past that, new notes take over voices without
  clicks.
- More than 1024 events between two chunks would lose events. Real playing
  never comes close.
- Changing the wave never changes notes that are already ringing.
- The envelope times, wave trims and voice level are starting values. They
  get tuned by ear, and step 4 turns the envelope into knobs. Changing these
  numbers does not need a new ADR.
- The default key layout and the wave key live in
  [design.md](../design.md#computer-keyboard), because key mappings are
  presets the user can change.
