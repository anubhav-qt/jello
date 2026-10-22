// The sound panel's values (docs/design.md#sound-panel): how each one lights
// its row of dots, what one arrow step is and how it is written, and the
// maths behind the panel's drawings.

import {
  FILTER_LONGEST_MS,
  FILTER_LOWEST_HZ,
  FILTER_OPEN_HZ,
  LFO_FASTEST_HZ,
  LFO_MOST_PRESSES,
  LFO_SLOWEST_HZ,
  LOUDNESS_LONGEST_MS,
  LOUDNESS_SHORTEST_MS,
  type LfoView,
} from "./sound";

// A value picked in the sound panel for the arrow keys to step.
export interface Picked {
  // Which value, like "loudness decay" or "lfo 3 speed hz", so its field
  // and its drawing's square know they are picked.
  id: string;
  // Its name, for the panel's top, like "Decay".
  name: string;
  // Moves it `steps` steps: 1 or -1 for right and left, 10 or -10 for up
  // and down.
  step: (steps: number) => void;
}

// How a value sits on its row of dots, steps and is written.
export interface Scale {
  // The share of its row it lights up to, from 0 to 1. A connection's
  // amount goes from -1 to 1, either way from its middle zero.
  share(value: number): number;
  // The value at a share of its row, rounded to a whole step.
  at(share: number): number;
  // The value `steps` steps on from `value`, kept within its range.
  step(value: number, steps: number): number;
  // Its number as shown, like "420" or "5.5", and its unit, like "ms".
  words(value: number): string;
  unit(value: number): string;
}

// One value the panel shows and sets, like the loudness envelope's decay.
export interface Setting {
  id: string;
  name: string;
  scale: Scale;
  // Its value now, from Rust's last answer. Steps start from it.
  now: () => number;
  // Sends a new value, with the rest of its group as they are.
  set: (value: number) => void;
}

// What the arrow keys step while `setting` is picked.
export function pickedOf(setting: Setting): Picked {
  return {
    id: setting.id,
    name: setting.name,
    step: (steps: number) => {
      const value = setting.scale.step(setting.now(), steps);
      setting.set(value);
    },
  };
}

// Whether the value called `id` is the one picked.
export function isPicked(picked: Picked | null, id: string): boolean {
  if (picked === null) {
    return false;
  }
  return picked.id === id;
}

// How many of a row's `dots` are lit at `share`. The first always is, so
// a value at its lowest still shows where its row starts.
export function litDots(share: number, dots: number): number {
  return 1 + Math.round(share * (dots - 1));
}

// How many dots a connection's amount lights on one side of its middle
// zero, out of `side`. Any amount but 0 lights at least one.
export function litSideDots(share: number, side: number): number {
  if (share === 0) {
    return 0;
  }
  const lit = Math.round(Math.abs(share) * side);
  return Math.max(1, lit);
}

// `value` kept within `lowest` and `highest`.
function within(value: number, lowest: number, highest: number): number {
  if (value < lowest) {
    return lowest;
  }
  if (value > highest) {
    return highest;
  }
  return value;
}

// `value` rounded to a whole number of `step`s. toFixed drops what the
// sums leave over, so 0.1 three times is 0.3, not 0.30000000000000004.
function roundedTo(value: number, step: number): number {
  const steps = Math.round(value / step);
  return Number((steps * step).toFixed(6));
}

// `value` with as many decimals as `step` has, and none it doesn't need:
// "5.5", "440", "0.25".
function numberWords(value: number, step: number): string {
  let decimals = 0;
  if (step < 1) {
    decimals = 1;
  }
  if (step < 0.1) {
    decimals = 2;
  }
  const rounded = Number(value.toFixed(decimals));
  return String(rounded);
}

// A unit that has one name for exactly one, like "press", and another for
// any other number, like "presses".
function unitFor(value: number, one: string, many: string): string {
  if (value === 1) {
    return one;
  }
  return many;
}

// A time in ms, from `shortest` to `longest`, on a curved row: the share
// is cubed, so short times, where small changes are heard, get most of it.
// Halfway along is an eighth of the longest. A step is 1 ms.
export function timeScale(shortest: number, longest: number): Scale {
  return {
    share(ms: number) {
      return Math.cbrt(ms / longest);
    },
    at(share: number) {
      const ms = Math.round(longest * share * share * share);
      return within(ms, shortest, longest);
    },
    step(ms: number, steps: number) {
      const stepped = Math.round(ms) + steps;
      return within(stepped, shortest, longest);
    },
    words(ms: number) {
      return String(Math.round(ms));
    },
    unit() {
      return "ms";
    },
  };
}

// A value from `lowest` to `highest` on an even row, stepped by `step`.
// Its unit is `one` for exactly one and `many` otherwise.
export function evenScale(lowest: number, highest: number, step: number, one: string, many: string): Scale {
  return {
    share(value: number) {
      return (value - lowest) / (highest - lowest);
    },
    at(share: number) {
      const value = lowest + share * (highest - lowest);
      const rounded = roundedTo(value, step);
      return within(rounded, lowest, highest);
    },
    step(value: number, steps: number) {
      const stepped = roundedTo(value + steps * step, step);
      return within(stepped, lowest, highest);
    },
    words(value: number) {
      return numberWords(value, step);
    },
    unit(value: number) {
      return unitFor(value, one, many);
    },
  };
}

// A percent, from 0 to 100.
export const PERCENT = evenScale(0, 100, 1, "%", "%");

// A whole number from minus to plus `most`, on a row lit either way from
// its middle zero, like an oscillator's octaves. A step is 1. Its unit is
// `one` for exactly one either way and `many` otherwise.
export function signedScale(most: number, one: string, many: string): Scale {
  return {
    share(value: number) {
      return value / most;
    },
    at(share: number) {
      const value = Math.round(share * most);
      return within(value, -most, most);
    },
    step(value: number, steps: number) {
      const stepped = Math.round(value) + steps;
      return within(stepped, -most, most);
    },
    words(value: number) {
      return String(value);
    },
    unit(value: number) {
      return unitFor(Math.abs(value), one, many);
    },
  };
}

// The loudness envelope's times. It needs at least 1 ms, or a note would
// click.
export const LOUDNESS_TIME = timeScale(LOUDNESS_SHORTEST_MS, LOUDNESS_LONGEST_MS);

// The filter envelope's times, an LFO's delay and fade-in in ms, and the
// vibrato key's rise and fall: all from 0 to 10,000 ms.
export const TIME = timeScale(0, FILTER_LONGEST_MS);

// The cutoff in Hz, from 20 Hz to open at 20,000 Hz. Every dot is the same
// number of octaves, so low cutoffs get as much of the row as high ones. A
// step is 1 Hz.
export const CUTOFF: Scale = {
  share(hz: number) {
    return Math.log(hz / FILTER_LOWEST_HZ) / Math.log(FILTER_OPEN_HZ / FILTER_LOWEST_HZ);
  },
  at(share: number) {
    const hz = FILTER_LOWEST_HZ * Math.pow(FILTER_OPEN_HZ / FILTER_LOWEST_HZ, share);
    const rounded = Math.round(hz);
    return within(rounded, FILTER_LOWEST_HZ, FILTER_OPEN_HZ);
  },
  step(hz: number, steps: number) {
    const stepped = Math.round(hz) + steps;
    return within(stepped, FILTER_LOWEST_HZ, FILTER_OPEN_HZ);
  },
  // At the top the filter is open: off.
  words(hz: number) {
    if (hz >= FILTER_OPEN_HZ) {
      return "open";
    }
    return String(Math.round(hz));
  },
  unit(hz: number) {
    if (hz >= FILTER_OPEN_HZ) {
      return "";
    }
    return "Hz";
  },
};

// How fine an LFO's speed in Hz steps at `hz`: the digit it is shown to.
function hzStepAt(hz: number): number {
  if (hz < 1) {
    return 0.01;
  }
  if (hz < 10) {
    return 0.1;
  }
  return 1;
}

// The same, going down from `hz`: from 1 Hz down, it steps by 0.01.
function hzStepBelow(hz: number): number {
  if (hz <= 1) {
    return 0.01;
  }
  if (hz <= 10) {
    return 0.1;
  }
  return 1;
}

// An LFO's speed in Hz, from 0.01 to 100. Every dot is the same number of
// octaves, so slow speeds get as much of the row as fast ones. A step is
// 0.01 below 1 Hz, 0.1 below 10 Hz, then 1.
export const LFO_HZ: Scale = {
  share(hz: number) {
    return Math.log(hz / LFO_SLOWEST_HZ) / Math.log(LFO_FASTEST_HZ / LFO_SLOWEST_HZ);
  },
  at(share: number) {
    const hz = LFO_SLOWEST_HZ * Math.pow(LFO_FASTEST_HZ / LFO_SLOWEST_HZ, share);
    const rounded = roundedTo(hz, hzStepAt(hz));
    return within(rounded, LFO_SLOWEST_HZ, LFO_FASTEST_HZ);
  },
  // One step at a time, as each step's size depends on where it starts.
  step(hz: number, steps: number) {
    let value = hz;
    for (let taken = 0; taken < Math.abs(steps); taken += 1) {
      if (steps > 0) {
        const size = hzStepAt(value);
        value = roundedTo(value + size, size);
      } else {
        const size = hzStepBelow(value);
        value = roundedTo(value - size, size);
      }
    }
    return within(value, LFO_SLOWEST_HZ, LFO_FASTEST_HZ);
  },
  words(hz: number) {
    return numberWords(hz, hzStepAt(hz));
  },
  unit() {
    return "Hz";
  },
};

// The note lengths an LFO's cycle can last, in beats, shortest first: from
// a sixteenth of a beat to 32 bars, with the triplet and dotted lengths
// between. A step is the next length.
const BEAT_LENGTHS = [
  1 / 16,
  1 / 8,
  1 / 6,
  1 / 4,
  1 / 3,
  1 / 2,
  2 / 3,
  3 / 4,
  1,
  1.5,
  2,
  3,
  4,
  6,
  8,
  12,
  16,
  24,
  32,
  48,
  64,
  96,
  128,
];
const BEAT_WORDS = [
  "1/16",
  "1/8",
  "1/6",
  "1/4",
  "1/3",
  "1/2",
  "2/3",
  "3/4",
  "1",
  "1.5",
  "2",
  "3",
  "4",
  "6",
  "8",
  "12",
  "16",
  "24",
  "32",
  "48",
  "64",
  "96",
  "128",
];

// A value from a list, like the beat lengths, where each dot or step is
// the next one on the list. `words` says how each is written.
function listScale(values: number[], words: string[]): Scale {
  const last = values.length - 1;

  // Where on the list the value nearest `value` is.
  function nearest(value: number): number {
    let best = 0;
    for (let place = 1; place <= last; place += 1) {
      const off = Math.abs(values[place] - value);
      if (off < Math.abs(values[best] - value)) {
        best = place;
      }
    }
    return best;
  }

  return {
    share(value: number) {
      return nearest(value) / last;
    },
    at(share: number) {
      return values[Math.round(share * last)];
    },
    step(value: number, steps: number) {
      const place = within(nearest(value) + steps, 0, last);
      return values[place];
    },
    // A value not on the list, from a song file, is written as it is.
    words(value: number) {
      const place = nearest(value);
      if (Math.abs(values[place] - value) < 0.000001) {
        return words[place];
      }
      return numberWords(value, 0.01);
    },
    unit(value: number) {
      if (value > 0) {
        if (value <= 1) {
          return "beat";
        }
      }
      return "beats";
    },
  };
}

// An LFO's speed in beats a cycle, and its delay and fade-in in beats,
// which can also be none.
export const LFO_BEATS = listScale(BEAT_LENGTHS, BEAT_WORDS);
export const LFO_DELAY_BEATS = listScale([0, ...BEAT_LENGTHS], ["0", ...BEAT_WORDS]);

// The targets moved in cents and in octaves (`Target::unit` in
// src-tauri/src/modulation.rs). LFOs' speeds, "lfo 1 speed" and so on, are
// in octaves too, and the oscillators' pitches, "osc 1 pitch" and so on, in
// cents.
const CENTS_TARGETS = ["pitch", "detune"];
const OCTAVE_TARGETS = [
  "cutoff",
  "filter amount",
  "loudness attack",
  "loudness decay",
  "loudness release",
  "filter attack",
  "filter decay",
  "filter release",
];

// The targets read once as each note starts. Every other one moves while
// the note sounds. The oscillators' kinds, "osc 1 kind" and so on, are read
// as it starts too.
const START_TARGETS = [
  "copies",
  "loudness attack",
  "loudness decay",
  "loudness release",
  "filter attack",
  "filter decay",
  "filter release",
];

// Whether `target` is an oscillator's kind, like "osc 2 kind".
function isOscillatorKind(target: string): boolean {
  if (!target.startsWith("osc ")) {
    return false;
  }
  return target.endsWith(" kind");
}

// Whether `target` moves a pitch: the note's, or an oscillator's, like
// "osc 2 pitch". Only those can snap to notes.
export function isPitch(target: string): boolean {
  if (target === "pitch") {
    return true;
  }
  if (!target.startsWith("osc ")) {
    return false;
  }
  return target.endsWith(" pitch");
}

// The unit a connection moves `target` in: "cents", "octaves", "steps"
// (through the oscillators' kinds), "copies" or "%" (of its whole range).
export function targetUnit(target: string): string {
  if (CENTS_TARGETS.includes(target)) {
    return "cents";
  }
  if (isPitch(target)) {
    return "cents";
  }
  if (OCTAVE_TARGETS.includes(target)) {
    return "octaves";
  }
  if (target.endsWith(" speed")) {
    return "octaves";
  }
  if (isOscillatorKind(target)) {
    return "steps";
  }
  if (target === "copies") {
    return "copies";
  }
  return "%";
}

// Whether a connection moves `target` while a note sounds, rather than
// once as it starts.
export function movesLive(target: string): boolean {
  if (isOscillatorKind(target)) {
    return false;
  }
  return !START_TARGETS.includes(target);
}

// The most a connection can move `target` either way, in its unit
// (`Target::most_amount` in src-tauri/src/modulation.rs).
export function mostAmount(target: string): number {
  if (isPitch(target)) {
    return 4800;
  }
  if (target === "detune") {
    return 50;
  }
  if (target === "cutoff") {
    return 10;
  }
  if (target === "filter amount") {
    return 8;
  }
  // Round through all five kinds.
  if (isOscillatorKind(target)) {
    return 4;
  }
  if (target === "copies") {
    return 6;
  }
  // An LFO's speed or an envelope's time, both in octaves, run over about
  // 13 octaves.
  if (targetUnit(target) === "octaves") {
    return 14;
  }
  return 100;
}

// A connection's amount for `target`, from minus to plus its most. Cents
// are on a curved row, cubed like times, so the few cents vibrato uses get
// most of it; the rest are even. A step is 0.1 in octaves, else 1.
export function amountScale(target: string): Scale {
  const unit = targetUnit(target);
  const most = mostAmount(target);
  let step = 1;
  if (unit === "octaves") {
    step = 0.1;
  }
  const curved = unit === "cents";

  return {
    share(amount: number) {
      const share = amount / most;
      if (!curved) {
        return share;
      }
      return Math.cbrt(share);
    },
    at(share: number) {
      let amount = share * most;
      if (curved) {
        amount = share * share * share * most;
      }
      const rounded = roundedTo(amount, step);
      return within(rounded, -most, most);
    },
    step(amount: number, steps: number) {
      const stepped = roundedTo(amount + steps * step, step);
      return within(stepped, -most, most);
    },
    words(amount: number) {
      return numberWords(amount, step);
    },
    unit(amount: number) {
      if (unit === "octaves") {
        return unitFor(Math.abs(amount), "octave", "octaves");
      }
      if (unit === "steps") {
        return unitFor(Math.abs(amount), "step", "steps");
      }
      if (unit === "copies") {
        return unitFor(Math.abs(amount), "copy", "copies");
      }
      return unit;
    },
  };
}

// How loud the low-pass filter leaves `hz`, in dB, with its cutoff at
// `cutoffHz` and its resonance in percent: the 2-pole low-pass of
// engine/src/filter.cpp. Its Q grows with the resonance squared, from
// 0.7071, no bump at all, to 10. An open filter leaves everything as it is.
export function filterDb(hz: number, cutoffHz: number, resonance: number): number {
  if (cutoffHz >= FILTER_OPEN_HZ) {
    return 0;
  }
  const q = filterQ(resonance);
  const w = hz / cutoffHz;
  const below = 1 - w * w;
  const bump = w / q;
  return -10 * Math.log10(below * below + bump * bump);
}

const LOWEST_Q = 0.7071;
const HIGHEST_Q = 10;

function filterQ(resonance: number): number {
  const share = resonance / 100;
  return LOWEST_Q + share * share * (HIGHEST_Q - LOWEST_Q);
}

// The resonance, in percent, that makes the filter's bump `db` loud at the
// cutoff. There the low-pass is as loud as its Q, in dB.
export function resonanceForDb(db: number): number {
  const q = Math.pow(10, db / 20);
  const squared = (q - LOWEST_Q) / (HIGHEST_Q - LOWEST_Q);
  const share = Math.sqrt(within(squared, 0, 1));
  return Math.round(share * 100);
}

// Slides from `before` to `height` over the first `smooth` share of a
// stretch, `into` of the way through it, then holds `height`, as the
// engine's `hold` does (engine/src/lfo.cpp).
function hold(before: number, height: number, into: number, smooth: number): number {
  if (into < smooth) {
    return before + ((height - before) * into) / smooth;
  }
  return height;
}

function triangle(phase: number): number {
  if (phase < 0.25) {
    return 4 * phase;
  }
  if (phase < 0.75) {
    return 2 - 4 * phase;
  }
  return 4 * phase - 4;
}

// The jump back down is the first `smooth` share of each cycle, and the
// rise the rest.
function sawUp(phase: number, smooth: number): number {
  if (phase < smooth) {
    return 1 - (2 * phase) / smooth;
  }
  const risen = (phase - smooth) / (1 - smooth);
  return -1 + 2 * risen;
}

// The random shapes have no heights to show until a note plays, so their
// drawings always use these.
const RANDOM_HEIGHTS = [0.55, -0.35, 0.9, -0.8, 0.1, -0.15, 0.7, -0.6];

function randomHeight(cycle: number): number {
  const count = RANDOM_HEIGHTS.length;
  const place = ((cycle % count) + count) % count;
  return RANDOM_HEIGHTS[place];
}

function stepsHeight(lfo: LfoView, phase: number, smooth: number): number {
  const count = lfo.step_count;
  const place = phase * count;
  // A nudge, as a step's start worked out as a fraction like 1/3 can land
  // a hair before it.
  let step = Math.floor(place + 0.000000001);
  if (step > count - 1) {
    step = count - 1;
  }
  const into = Math.max(0, place - step);
  // The step before the first is the last, as the steps go round.
  let before = step - 1;
  if (before < 0) {
    before = count - 1;
  }
  return hold(lfo.steps[before] / 100, lfo.steps[step] / 100, into, smooth);
}

// The height of `lfo`, from -1 to 1, `cycles` cycles after its start, as
// the engine works it out (engine/src/lfo.cpp).
export function lfoHeight(lfo: LfoView, cycles: number): number {
  const whole = Math.floor(cycles);
  const phase = cycles - whole;
  const smooth = lfo.smooth / 100;

  if (lfo.shape === "sine") {
    return Math.sin(2 * Math.PI * phase);
  }
  if (lfo.shape === "triangle") {
    return triangle(phase);
  }
  if (lfo.shape === "square") {
    if (phase < 0.5) {
      return hold(-1, 1, phase / 0.5, smooth);
    }
    return hold(1, -1, (phase - 0.5) / 0.5, smooth);
  }
  if (lfo.shape === "saw up") {
    return sawUp(phase, smooth);
  }
  // A saw up upside down.
  if (lfo.shape === "saw down") {
    return -sawUp(phase, smooth);
  }
  if (lfo.shape === "random steps") {
    return hold(randomHeight(whole - 1), randomHeight(whole), phase, smooth);
  }
  // A half cosine from one height to the next, so it leaves and lands
  // gently.
  if (lfo.shape === "smooth random") {
    const from = randomHeight(whole);
    const to = randomHeight(whole + 1);
    const done = (1 - Math.cos(Math.PI * phase)) / 2;
    return from + (to - from) * done;
  }
  return stepsHeight(lfo, phase, smooth);
}

// Whether `shape` jumps, so its smoothing does something.
export function shapeJumps(shape: string): boolean {
  if (shape === "sine") {
    return false;
  }
  if (shape === "triangle") {
    return false;
  }
  return shape !== "smooth random";
}

// The oscillators' kinds, in Rust's order, each with a small drawing of it,
// 20 by 12, for the kind tiles.
export const KINDS = [
  { name: "sine", glyph: "M1 6 C 4 -3, 7 -3, 10 6 S 16 15, 19 6" },
  { name: "triangle", glyph: "M1 6 L 5.5 1 L 14.5 11 L 19 6" },
  { name: "square", glyph: "M1 11 L 1 1 L 10 1 L 10 11 L 19 11 L 19 1" },
  { name: "saw", glyph: "M1 6 L 10 1 L 10 11 L 19 6" },
  { name: "noise", glyph: "M1 6 L 3 2 L 5 9 L 7 4 L 9 11 L 11 3 L 13 8 L 15 1 L 17 7 L 19 5" },
];

// The LFO shapes, in Rust's order, each with a small drawing of it, 20 by
// 12, for the LFO lists and the shape tiles.
export const LFO_SHAPES = [
  { name: "sine", glyph: "M1 6 C 4 -3, 7 -3, 10 6 S 16 15, 19 6" },
  { name: "triangle", glyph: "M1 6 L 5.5 1 L 14.5 11 L 19 6" },
  { name: "square", glyph: "M1 11 L 1 1 L 10 1 L 10 11 L 19 11 L 19 1" },
  { name: "saw up", glyph: "M1 11 L 10 1 L 10 11 L 19 1 L 19 11" },
  { name: "saw down", glyph: "M1 1 L 10 11 L 10 1 L 19 11 L 19 1" },
  { name: "random steps", glyph: "M1 8 L 5 8 L 5 2 L 9 2 L 9 10 L 13 10 L 13 4 L 19 4" },
  { name: "smooth random", glyph: "M1 8 C 3.5 8, 3.5 2, 6 2 S 9.5 10, 12 10 S 15.5 4, 19 4" },
  { name: "steps", glyph: "M2 11 L 2 6 M6 11 L 6 2 M10 11 L 10 8 M14 11 L 14 1 M18 11 L 18 5" },
];

// The small drawing of the shape called `name`.
export function shapeGlyph(name: string): string {
  for (const shape of LFO_SHAPES) {
    if (shape.name === name) {
      return shape.glyph;
    }
  }
  return LFO_SHAPES[0].glyph;
}

// The LFO a source or target names, counted from 0, like 2 for "lfo 3" or
// "lfo 3 speed", or null for one that names no LFO.
export function lfoIndexOf(name: string): number | null {
  if (!name.startsWith("lfo ")) {
    return null;
  }
  const number = parseInt(name.slice(4), 10);
  if (Number.isNaN(number)) {
    return null;
  }
  return number - 1;
}

// A source or target as the panel writes it: "LFO 1" for "lfo 1" and
// "LFO 2 speed" for "lfo 2 speed". Others keep their own names.
export function nameWords(name: string): string {
  if (name.startsWith("lfo ")) {
    return "LFO " + name.slice(4);
  }
  return name;
}

// An LFO's speed in key presses a cycle. A chord is one press.
export const LFO_PRESSES = evenScale(1, LFO_MOST_PRESSES, 1, "press", "presses");

// The scale an LFO's speed is on now, and its speed on it.
export function speedScale(lfo: LfoView): Scale {
  if (lfo.speed === "beats") {
    return LFO_BEATS;
  }
  if (lfo.speed === "presses") {
    return LFO_PRESSES;
  }
  return LFO_HZ;
}

export function speedOf(lfo: LfoView): number {
  if (lfo.speed === "beats") {
    return lfo.beats;
  }
  if (lfo.speed === "presses") {
    return lfo.presses;
  }
  return lfo.hz;
}

// An LFO's speed in words, like "5.5 Hz", "1/4 beat" or "8 presses".
export function speedWords(lfo: LfoView): string {
  const scale = speedScale(lfo);
  const speed = speedOf(lfo);
  return `${scale.words(speed)} ${scale.unit(speed)}`;
}
