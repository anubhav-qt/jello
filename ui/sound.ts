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
  // Where the filter and its envelope are.
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
  // How long each melody note played with the glide key held takes to
  // slide in, in ms (docs/adr/0036-glide.md).
  glide_ms: number;
  // The preset the song sounds exactly like, whose tile is lit, or null
  // (docs/adr/0037-presets.md).
  preset: string | null;
  // The song's drum kit and its pads (docs/design.md#sampler).
  drums: DrumsView;
}

// Where an instrument is (`InstrumentView` in src-tauri/src/commands.rs):
// "none" when none is picked, "ready", "loading", or "failed" with why.
export interface InstrumentView {
  name: string | null;
  state: string;
  problem: string | null;
}

// The song's drums (`DrumsView` in src-tauri/src/commands.rs): the kit,
// and its DRUM_PAD_COUNT pads in order.
export interface DrumsView {
  kit: InstrumentView;
  pads: PadView[];
}

// One drum pad, like the snare.
export interface PadView {
  name: string;
  // In percent.
  level: number;
  // How far it is tuned, from minus to plus DRUM_MOST_HALF_STEPS.
  half_steps: number;
  // In percent, from -100 (left) to 100 (right).
  pan: number;
  // Hitting a pad stops the others in its group, like the closed hat
  // stopping the open hat. 0 is no group.
  group: number;
}

// How many pads a kit has, how far each can be tuned either way, and the
// highest group (`DRUM_` in src-tauri/src/drums.rs).
export const DRUM_PAD_COUNT = 16;
export const DRUM_MOST_HALF_STEPS = 12;
export const DRUM_MOST_GROUP = 4;

// The instruments in Music/Jello/Instruments and the starters Jello can
// fetch (`InstrumentsView` in src-tauri/src/commands.rs).
export interface InstrumentsView {
  // The folder, or null if it can't be found.
  folder: string | null;
  names: string[];
  starters: StarterView[];
}

// A starter instrument (`StarterView` in src-tauri/src/starters.rs).
export interface StarterView {
  name: string;
  megabytes: number;
  // Whether it plays drums, for the Drums page, or notes.
  drums: boolean;
  license: string;
  credit: string;
  // True once it is in the folder.
  fetched: boolean;
  downloading: boolean;
}

// How far a starter's download has got, as Rust tells it
// ("starter-download" in src-tauri/src/starters.rs).
export interface StarterProgress {
  name: string;
  received: number;
  total: number;
}

// One preset in Music/Jello/Sounds (`PresetView` in
// src-tauri/src/presets.rs).
export interface PresetView {
  name: string;
  // False for a file that is not a sound file: its tile is dimmed.
  readable: boolean;
  // Whether new songs start with it.
  starting: boolean;
}

// One oscillator (`OscillatorView` in src-tauri/src/commands.rs).
export interface OscillatorView {
  // "sine", "triangle", "square", "saw", "noise" or "sample".
  kind: string;
  // In percent: at 0 it is silent.
  level: number;
  // How far it is tuned from the note, each from minus to plus its most
  // below. Noise has no pitch, so they do nothing to it.
  octaves: number;
  half_steps: number;
  cents: number;
  // How much of each cycle a square spends up, in percent, from
  // PULSE_NARROWEST_PERCENT to PULSE_WIDEST_PERCENT. Only a square uses it
  // (docs/design.md#pulse-width).
  width: number;
  // The instrument a sample plays (docs/design.md#sampler).
  instrument: InstrumentView;
}

// An oscillator as the sound panel sends it (`set_oscillator` in
// src-tauri/src/commands.rs).
export interface Oscillator {
  kind: string;
  level: number;
  octaves: number;
  halfSteps: number;
  cents: number;
  width: number;
}

// How many oscillators play each note, and how far each can be tuned from
// it, either way (`OSCILLATOR_` in src-tauri/src/engine.rs).
export const OSCILLATOR_COUNT = 3;
export const OSCILLATOR_MOST_OCTAVES = 3;
export const OSCILLATOR_MOST_HALF_STEPS = 12;
export const OSCILLATOR_MOST_CENTS = 100;

// How much of each cycle a square can spend up, in percent (`PULSE_` in
// src-tauri/src/engine.rs). 50 is the square.
export const PULSE_NARROWEST_PERCENT = 5;
export const PULSE_WIDEST_PERCENT = 95;

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

// Where the filter and its envelope are (`FilterView` in
// src-tauri/src/commands.rs, docs/adr/0028-a-low-pass-filter.md).
export interface FilterView {
  // "low-pass", "high-pass", "band-pass" or "notch"
  // (docs/design.md#filter-types).
  filter_type: string;
  // Middle C's cutoff, in Hz. A low-pass at FILTER_OPEN_HZ, or a high-pass
  // at FILTER_LOWEST_HZ, is open: off.
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
// cutoff and the highest, in Hz, the most octaves the envelope opens it,
// and the longest any of its stages takes.
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

// The longest a glide takes, in ms (`GLIDE_LONGEST_MS` in
// src-tauri/src/engine.rs).
export const GLIDE_LONGEST_MS = 10000;

// What the sound panel sends to Rust. ui/main.ts sends each in order, and
// Rust's answer comes back through `showSound`.
export interface SoundSender {
  // `index` counts from 0.
  setOscillator(index: number, oscillator: Oscillator): void;
  setLoudnessEnvelope(envelope: LoudnessEnvelope): void;
  // `filterType` is "low-pass", "high-pass", "band-pass" or "notch",
  // `cutoffHz` is middle C's cutoff, and `resonance` is in percent.
  setFilter(filterType: string, cutoffHz: number, resonance: number): void;
  setFilterEnvelope(envelope: FilterEnvelope): void;
  // In percent.
  setKeyTracking(keyTracking: number): void;
  setUnison(unison: Unison): void;
  // A4's pitch in Hz.
  setTuning(a4Hz: number): void;
  setVibratoKeyTimes(riseMs: number, fallMs: number): void;
  setGlideTime(glideMs: number): void;
  // `index` counts from 0.
  setLfo(index: number, lfo: LfoView): void;
  setConnection(index: number, connection: ConnectionView): void;
  // The presets, as Rust lists them now (docs/adr/0037-presets.md).
  listPresets(): Promise<PresetView[]>;
  // Copies preset `name` into the song.
  loadPreset(name: string): void;
  // Saves the song's sound as preset `name`. Done once it is saved, or
  // once saving failed and the top bar says why.
  savePreset(name: string): Promise<void>;
  // Why `name` can't name a preset, or null if it can.
  presetNameProblem(name: string): Promise<string | null>;
  // Marks preset `name` as the starting sound, or with null, unmarks it.
  markStartingPreset(name: string | null): Promise<void>;
  // Picks instrument `name` for oscillator `index`, from 0, or with null,
  // none (docs/design.md#sampler).
  setInstrument(index: number, name: string | null): void;
  // Picks instrument `name` as the drum kit, or with null, none.
  setDrumKit(name: string | null): void;
  // Sets pad `pad`, from 0, from the next hit on.
  setDrumPad(pad: number, settings: PadView): void;
  // The instruments, as Rust lists them now.
  listInstruments(): Promise<InstrumentsView | null>;
  openInstrumentsFolder(): void;
  // Fetches starter `name` into the folder. Done once it is kept, or once
  // it failed and the top bar says why.
  downloadStarter(name: string): Promise<void>;
}

// What the looper is doing (`LooperView` in src-tauri/src/commands.rs).
export interface LooperView {
  // "idle", "counting in", "recording", "looping", "layering": playing
  // the loop and recording a layer over it, or "paused"
  // (docs/adr/0038-song-files.md).
  state: string;
  // While counting in, the beat (1 to 4). While recording, the bar being
  // recorded, from 1. While looping, layering or paused, how many bars the
  // loop lasts.
  number: number;
  // True from the first press until the loop is cleared.
  busy: boolean;
  // How many beats into the count-in, the recording or the repeat playing
  // now, when Rust answered. Each starts on beat 1 of a bar. 0 when idle.
  beats_in: number;
  // The loop's layers while recording, looping, layering or paused, oldest
  // first (docs/adr/0026-layers.md). Empty otherwise.
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
  bars: number;
  // While it is recorded, its notes as played, in the order they started.
  // Once it plays, where each time around plays them, in the order they
  // start.
  notes: LoopNote[];
  // How loud it plays, in percent, from 0 to 100.
  volume: number;
  // False while it is paused: silent, with its volume kept.
  playing: boolean;
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
  // A paused loop plays again from beat 1.
  if (looper.state === "paused") {
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

// True while there is a loop, playing or paused.
export function hasLoop(looper: LooperView): boolean {
  if (isLoopPlaying(looper)) {
    return true;
  }
  return looper.state === "paused";
}

// How many beats the loop lasts.
export function loopBeats(looper: LooperView): number {
  return Math.max(1, looper.number) * BEATS_PER_BAR;
}

// Which of the four beat lights is lit, 1 to 4, or 0 when idle or paused.
export function litBeat(looper: LooperView, beats: number): number {
  if (looper.state === "idle") {
    return 0;
  }
  if (looper.state === "paused") {
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
    return `count ${litBeat(looper, beats)}`;
  }
  if (looper.state === "recording") {
    return `rec ${barNow(beats)}`;
  }
  if (looper.state === "looping") {
    if (looper.number === 1) {
      return "1 bar";
    }
    return `${looper.number} bars`;
  }
  // The layer being recorded is always the last.
  if (looper.state === "layering") {
    return `layer ${looper.layers.length}`;
  }
  if (looper.state === "paused") {
    return "paused";
  }
  return "looper";
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
