// What Rust says about the sound after each key press (`SoundView` in
// src-tauri/src/commands.rs).
export interface SoundView {
  // The OSCILLATOR_COUNT oscillators new notes play, in order
  // (docs/adr/0035-oscillators-with-kinds.md).
  oscillators: OscillatorView[];
  // The song's tempo, in beats per minute.
  tempo: number;
  // Whether loops snap to the beat: each time around, the loop plays its
  // notes moved to the nearest half beat.
  snap: boolean;
  // What the looper is doing.
  looper: LooperView;
  // The loudness envelope new notes start with.
  loudness: LoudnessView;
  // Where the low-pass filter and its envelope are.
  filter: FilterView;
  // The copies new notes are played by.
  unison: UnisonView;
  // A4's pitch in Hz, which new notes are tuned from.
  tuning_hz: number;
  // All LFO_COUNT LFOs and all CONNECTION_COUNT connections, in order, and
  // the vibrato key's times (docs/adr/0031-lfos-and-a-routing-list.md).
  lfos: LfoView[];
  connections: ConnectionView[];
  vibrato_key: VibratoKeyView;
}

// One oscillator (`OscillatorView` in src-tauri/src/commands.rs).
export interface OscillatorView {
  // "sine", "triangle", "square", "saw" or "noise".
  kind: string;
  // In percent: at 0 it is silent.
  level: number;
  // How far it is tuned from the note, each from minus to plus its most
  // below. Noise has no pitch, so they do nothing to it.
  octaves: number;
  half_steps: number;
  cents: number;
}

// An oscillator as the sound panel sends it (`set_oscillator` in
// src-tauri/src/commands.rs).
export interface Oscillator {
  kind: string;
  level: number;
  octaves: number;
  halfSteps: number;
  cents: number;
}

// How many oscillators play each note, and how far each can be tuned from
// it, either way (`OSCILLATOR_` in src-tauri/src/engine.rs).
export const OSCILLATOR_COUNT = 3;
export const OSCILLATOR_MOST_OCTAVES = 3;
export const OSCILLATOR_MOST_HALF_STEPS = 12;
export const OSCILLATOR_MOST_CENTS = 100;

// The loudness envelope new notes start with (`LoudnessView` in
// src-tauri/src/commands.rs).
export interface LoudnessView {
  attack_ms: number;
  decay_ms: number;
  // In percent of the peak.
  sustain: number;
  release_ms: number;
}

// A loudness envelope as the sound panel sends it
// (`set_loudness_envelope` in src-tauri/src/commands.rs).
export interface LoudnessEnvelope {
  attackMs: number;
  decayMs: number;
  sustain: number;
  releaseMs: number;
}

// The shortest and longest a loudness envelope's stage takes, in ms
// (`LOUDNESS_` in src-tauri/src/engine.rs).
export const LOUDNESS_SHORTEST_MS = 1;
export const LOUDNESS_LONGEST_MS = 10000;

// The lowest and highest A4 notes can be tuned to, in Hz (`TUNING_` in
// src-tauri/src/engine.rs).
export const TUNING_LOWEST_HZ = 415;
export const TUNING_HIGHEST_HZ = 466;

// Where the low-pass filter and its envelope are (`FilterView` in
// src-tauri/src/commands.rs, docs/adr/0028-a-low-pass-filter.md).
export interface FilterView {
  // Middle C's cutoff, in Hz. At FILTER_OPEN_HZ the filter is open: off.
  cutoff_hz: number;
  // In percent.
  resonance: number;
  // How far the filter envelope opens the cutoff, in octaves.
  amount: number;
  attack_ms: number;
  decay_ms: number;
  // In percent of the amount.
  sustain: number;
  release_ms: number;
  // How much each new note's cutoff follows middle C's, in percent.
  key_tracking: number;
}

// A filter envelope as the sound panel sends it
// (`set_filter_envelope` in src-tauri/src/commands.rs).
export interface FilterEnvelope {
  amount: number;
  attackMs: number;
  decayMs: number;
  sustain: number;
  releaseMs: number;
}

// The filter's ranges (`FILTER_` in src-tauri/src/engine.rs): the lowest
// cutoff and the cutoff that means open, in Hz, the most octaves the
// envelope opens it, and the longest any of its stages takes.
export const FILTER_LOWEST_HZ = 20;
export const FILTER_OPEN_HZ = 20000;
export const FILTER_MOST_OCTAVES = 8;
export const FILTER_LONGEST_MS = 10000;

// The copies new notes are played by (`UnisonView` in
// src-tauri/src/commands.rs, docs/adr/0029-unison.md).
export interface UnisonView {
  // From 1, which is off, to UNISON_MOST_COPIES.
  copies: number;
  // How far the highest copy is tuned above the note, and the lowest below
  // it, in cents: hundredths of a half step.
  detune_cents: number;
  // How far apart they sit between the speakers, in percent.
  spread: number;
}

// Unison as the sound panel sends it (`set_unison` in
// src-tauri/src/commands.rs).
export interface Unison {
  copies: number;
  detuneCents: number;
  spread: number;
}

// Unison's ranges (`UNISON_` in src-tauri/src/engine.rs): the most copies a
// note can have, and the most cents the highest is tuned above the note.
export const UNISON_MOST_COPIES = 7;
export const UNISON_MOST_DETUNE_CENTS = 50;

// One LFO (`LfoView` in src-tauri/src/commands.rs), as Rust sends it and
// the sound panel sends it back to `set_lfo`.
export interface LfoView {
  // "sine", "triangle", "square", "saw up", "saw down", "random steps",
  // "smooth random" or "steps".
  shape: string;
  // For the steps shape: how many steps play, from 1 to LFO_MOST_STEPS, and
  // every step's height in percent, from -100 to 100. `steps` always lists
  // LFO_MOST_STEPS of them.
  step_count: number;
  steps: number[];
  // True: every note starts its own copy of it. False: one runs for every
  // note.
  per_note: boolean;
  // What its speed is counted in, "hz", "beats" or "presses", and its
  // speed in each.
  speed: string;
  hz: number;
  beats: number;
  presses: number;
  // Where in its cycle it starts, and for a per-note LFO how far each
  // note's start is moved at random on top, in percent of a cycle.
  start: number;
  spread: number;
  // How long after each note starts it waits, then how long it takes to
  // grow to its full size: in ms, or in beats while its speed is in beats.
  delay_ms: number;
  fade_ms: number;
  delay_beats: number;
  fade_beats: number;
  // How much of the time between jumps is spent sliding instead, in
  // percent.
  smooth: number;
  // For a per-note LFO: one cycle, then it holds where it ended.
  once: boolean;
}

// One connection of the routing list (`ConnectionView` in
// src-tauri/src/commands.rs): `source` moves `target` by `amount`, in the
// target's unit. An unused one's source is "none".
export interface ConnectionView {
  // "none", "lfo 1" to "lfo 8", "vibrato key", "note height", "random",
  // "loudness envelope" or "filter envelope".
  source: string;
  // Like "pitch", "filter attack" or "lfo 2 speed".
  target: string;
  amount: number;
  // Both ways, the target moves from -amount to amount; one way, from 0 to
  // amount.
  one_way: boolean;
  // For pitch only: the move is rounded to whole half steps.
  snap: boolean;
  // Another source the amount is multiplied by, or "none".
  scaled_by: string;
  // The target's unit, like "cents". Rust needs no unit sent back.
  unit: string;
}

// How long the vibrato key takes to rise when held and fall when let go,
// in ms (`VibratoKeyView` in src-tauri/src/commands.rs).
export interface VibratoKeyView {
  rise_ms: number;
  fall_ms: number;
}

// How many LFOs and connections a song has, and the most steps an LFO's
// steps shape can have (src-tauri/src/modulation.rs).
export const LFO_COUNT = 8;
export const CONNECTION_COUNT = 64;
export const LFO_MOST_STEPS = 32;

// An LFO's ranges: its speed in Hz and in key presses a cycle, and its
// delay and fade-in in ms and in beats (`LFO_` in
// src-tauri/src/modulation.rs). Its beats come from a list of note lengths
// (ui/sound-scales.ts).
export const LFO_SLOWEST_HZ = 0.01;
export const LFO_FASTEST_HZ = 100;
export const LFO_MOST_PRESSES = 32;
export const LFO_LONGEST_MS = 10000;

// The longest the vibrato key takes to rise or fall, in ms.
export const VIBRATO_KEY_LONGEST_MS = 10000;

// What the sound panel sends to Rust. ui/main.ts sends each in order, and
// Rust's answer comes back through `showSound`.
export interface SoundSender {
  // `index` counts from 0.
  setOscillator(index: number, oscillator: Oscillator): void;
  setLoudnessEnvelope(envelope: LoudnessEnvelope): void;
  // `cutoffHz` is middle C's cutoff, and `resonance` is in percent.
  setFilter(cutoffHz: number, resonance: number): void;
  setFilterEnvelope(envelope: FilterEnvelope): void;
  // In percent.
  setKeyTracking(keyTracking: number): void;
  setUnison(unison: Unison): void;
  // A4's pitch in Hz.
  setTuning(a4Hz: number): void;
  setVibratoKeyTimes(riseMs: number, fallMs: number): void;
  // `index` counts from 0.
  setLfo(index: number, lfo: LfoView): void;
  setConnection(index: number, connection: ConnectionView): void;
}

// What the looper is doing (`LooperView` in src-tauri/src/commands.rs).
export interface LooperView {
  // "idle", "counting in", "recording", "looping" or "layering": playing
  // the loop and recording a layer over it.
  state: string;
  // While counting in, the beat (1 to 4). While recording, the bar being
  // recorded, from 1. While looping or layering, how many bars the loop
  // lasts.
  number: number;
  // True from the first press until the loop is cleared.
  busy: boolean;
  // How many beats into the count-in, the recording or the repeat playing
  // now, when Rust answered. Each starts on beat 1 of a bar. 0 when idle.
  beats_in: number;
  // The loop's layers while recording, looping or layering, oldest first
  // (docs/adr/0026-layers.md). Empty otherwise.
  layers: LayerView[];
  // While Esc waits for a layer's number, the layer it removes if none
  // comes: the newest, counted from 1. null otherwise.
  removing: number | null;
  // A message for the top bar, like "There is no layer 7", sent once.
  // null otherwise.
  message: string | null;
}

// One layer of the loop (`LayerView` in src-tauri/src/commands.rs).
export interface LayerView {
  // While it is recorded, its notes as played, in the order they started.
  // Once it plays, where each time around plays them, in the order they
  // start.
  notes: LoopNote[];
  // How loud it plays, in percent, from 0 to 100.
  volume: number;
  // True while it is being recorded.
  recording: boolean;
}

// One note of the loop, in beats from the loop's start.
export interface LoopNote {
  note: number;
  start: number;
  // null while its key is still held.
  end: number | null;
}

// How many beats make a bar (`BEATS_PER_BAR` in src-tauri/src/engine.rs).
export const BEATS_PER_BAR = 4;

// The looper's look (docs/adr/0024-the-looper-s-look.md) moves every frame,
// but Rust only answers every 50 ms. In between, the page counts on from
// Rust's last answer by its own clock: `msSince` is how long ago that was.
export function beatsNow(view: SoundView, msSince: number): number {
  const looper = view.looper;
  if (looper.state === "idle") {
    return 0;
  }
  const beats = looper.beats_in + (msSince * view.tempo) / 60000;

  // The count-in lasts one bar. Recording starts right after it, but until
  // Rust says so, the count-in stays on its last beat.
  if (looper.state === "counting in") {
    return Math.min(beats, BEATS_PER_BAR - 0.001);
  }
  // A loop starts over at its end.
  if (isLoopPlaying(looper)) {
    return beats % loopBeats(looper);
  }
  return beats;
}

// True while the loop plays: looping, or recording a layer over it.
export function isLoopPlaying(looper: LooperView): boolean {
  if (looper.state === "looping") {
    return true;
  }
  return looper.state === "layering";
}

// True while right Shift records: the first loop, or a layer.
export function isRecording(looper: LooperView): boolean {
  if (looper.state === "recording") {
    return true;
  }
  return looper.state === "layering";
}

// How many beats the loop lasts.
export function loopBeats(looper: LooperView): number {
  return Math.max(1, looper.number) * BEATS_PER_BAR;
}

// Which of the four beat lights is lit, 1 to 4, or 0 when idle.
export function litBeat(looper: LooperView, beats: number): number {
  if (looper.state === "idle") {
    return 0;
  }
  return Math.floor(beats % BEATS_PER_BAR) + 1;
}

// The bar being recorded, from 1.
export function barNow(beats: number): number {
  return Math.floor(beats / BEATS_PER_BAR) + 1;
}

// What right Shift says.
export function looperWords(view: SoundView, beats: number): string {
  const looper = view.looper;
  if (looper.state === "counting in") {
    return `count-in ${litBeat(looper, beats)}`;
  }
  if (looper.state === "recording") {
    return `rec bar ${barNow(beats)}`;
  }
  if (looper.state === "looping") {
    if (looper.number === 1) {
      return "loop 1 bar";
    }
    return `loop ${looper.number} bars`;
  }
  // The layer being recorded is always the last.
  if (looper.state === "layering") {
    return `rec layer ${looper.layers.length}`;
  }
  return `looper, ${view.tempo} bpm`;
}

// How full right Shift's bar is, from 0 to 1. While the first loop is
// recorded its length isn't known yet, so the bar fills once per bar.
export function looperFill(looper: LooperView, beats: number): number {
  if (looper.state === "recording") {
    return (beats % BEATS_PER_BAR) / BEATS_PER_BAR;
  }
  if (isLoopPlaying(looper)) {
    return beats / loopBeats(looper);
  }
  return 0;
}

// What Esc says: "clear" while the looper has something to clear, and
// nothing otherwise.
export function clearLabel(view: SoundView): string {
  if (view.looper.state === "idle") {
    return "";
  }
  return "clear";
}
