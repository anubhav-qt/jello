// The Mix panel's view of the song's mix (src-tauri/src/mix_commands.rs),
// what it sends back, and how each effect value sits on its row of dots
// (docs/design.md#mix).

import type { Scale, Setting } from "./sound-scales";

// How many slots each track, bus and the master has, and the most buses.
export const EFFECT_SLOTS = 8;
export const MOST_BUSES = 4;
// How many points a volume shaper can have.
export const SHAPER_MOST_POINTS = 16;
// How far a gate reads turned down when it is closed to silence.
export const GATE_SILENT_DB = -80;

// A value as the song file writes it: a number, a switch or a word.
export type Written = number | boolean | string;

// One slot's effect: its kind's word, whether it is on, and every value by
// its word, like "cutoff_hz".
export interface SlotView {
  kind: string;
  on: boolean;
  values: Record<string, Written>;
}

export type Chain = (SlotView | null)[];

export interface TrackStripView {
  name: string;
  volume: number;
  // From -100, fully left, to 100, fully right.
  pan: number;
  // One for each of the song's buses, in percent.
  sends: number[];
  chain: Chain;
}

export interface BusView {
  name: string;
  volume: number;
  pan: number;
  chain: Chain;
}

export interface Limiter {
  boost_db: number;
  ceiling_db: number;
  release_ms: number;
}

export interface MixView {
  tracks: TrackStripView[];
  buses: BusView[];
  master: { chain: Chain; limiter: Limiter };
}

// What the meters read since the panel last asked (`MetersView` in
// src-tauri/src/mix_commands.rs). Peaks are levels from 0: the tracks', then
// the buses' (4 places, used or not), then the master's.
export interface MetersView {
  peaks: number[];
  true_peak: number;
  limiter_reduction_db: number;
  short_term_lufs: number;
  // How far each slot of the place asked about turns the sound down now,
  // in dB.
  reductions: number[];
}

// One strip as the Mix panel's levels show it.
export interface Strip {
  place: string;
  // Its short name, like "6", "B1" or "M".
  label: string;
  title: string;
  // In percent, or null for the master, which has none.
  volume: number | null;
  // From -100 to 100, or null for the master.
  pan: number | null;
  // Its meter, in dB.
  db: number;
  // Where a dotted line goes before it: between the tracks and the
  // buses, and before the master.
  gap: boolean;
}

// Where in the peaks the buses and the master are.
export const FIRST_BUS_CHAIN = 9;
export const MASTER_CHAIN = 13;

// What the Mix panel sends to Rust. Places are "track 2", "bus 1" or
// "master"; slots, tracks and buses count from 1.
export interface MixSender {
  setEffect(place: string, slot: number, kind: string): void;
  moveEffect(place: string, from: number, to: number): void;
  setEffectOn(place: string, slot: number, on: boolean): void;
  setEffectValue(place: string, slot: number, word: string, value: Written): void;
  setPan(place: string, pan: number): void;
  setSend(track: number, bus: number, amount: number): void;
  setBusVolume(bus: number, volume: number): void;
  addBus(): void;
  removeBus(bus: number): void;
  renameBus(bus: number, name: string): void;
  setLimiter(limiter: Limiter): void;
  // Asks for the whole mix again.
  refresh(): void;
}

// The kinds the "+ Add effect" list offers, by their word, with names and
// what each does.
export const KINDS: { kind: string; name: string; words: string }[] = [
  { kind: "eq", name: "EQ", words: "Lifts or cuts bands of frequencies" },
  { kind: "compressor", name: "Compressor", words: "Turns loud parts down, evening it out" },
  { kind: "gate", name: "Gate", words: "Closes on quiet parts" },
  { kind: "saturation", name: "Saturation", words: "Drives it into warm or hard distortion" },
  { kind: "filter", name: "Filter", words: "Cuts above or below a cutoff, can follow loudness" },
  { kind: "chorus", name: "Chorus", words: "Copies that swing in time, wider and thicker" },
  { kind: "delay", name: "Delay", words: "Echoes, left and right" },
  { kind: "reverb", name: "Reverb", words: "A room, a hall or a plate" },
  { kind: "volume-shaper", name: "Volume shaper", words: "Volume drawn over a cycle, the pump" },
];

export function kindName(kind: string): string {
  for (const each of KINDS) {
    if (each.kind === kind) {
      return each.name;
    }
  }
  return kind;
}

// A level from 0 in dB, with silence at -120.
export function decibelsOf(level: number): number {
  if (level <= 0.000001) {
    return -120;
  }
  return 20 * Math.log10(level);
}

// A pan as written short: "C", "L24" or "R12".
export function panWords(pan: number): string {
  if (pan === 0) {
    return "C";
  }
  if (pan < 0) {
    return `L${-pan}`;
  }
  return `R${pan}`;
}

// A number with up to `decimals` decimals and none it doesn't need.
function trimmed(value: number, decimals: number): string {
  return String(Number(value.toFixed(decimals)));
}

function within(value: number, lowest: number, highest: number): number {
  if (value < lowest) {
    return lowest;
  }
  if (value > highest) {
    return highest;
  }
  return value;
}

function roundedTo(value: number, step: number): number {
  const steps = Math.round(value / step);
  return Number((steps * step).toFixed(6));
}

// A frequency in Hz, from `lowest` to `highest`, with every dot the same
// number of octaves. Below 1 kHz a step is 1 Hz, above it 10 Hz. Written
// "80 Hz" or "2.8 kHz".
export function hzScale(lowest: number, highest: number): Scale {
  return {
    share(hz: number) {
      return Math.log(hz / lowest) / Math.log(highest / lowest);
    },
    at(share: number) {
      const hz = lowest * Math.pow(highest / lowest, share);
      return within(Math.round(hz), lowest, highest);
    },
    step(hz: number, steps: number) {
      let step = 1;
      if (hz >= 1000) {
        step = 10;
      }
      return within(roundedTo(hz + steps * step, step), lowest, highest);
    },
    words(hz: number) {
      if (hz >= 1000) {
        return trimmed(hz / 1000, 1);
      }
      return String(Math.round(hz));
    },
    unit(hz: number) {
      if (hz >= 1000) {
        return "kHz";
      }
      return "Hz";
    },
  };
}

// A time from `lowest` to `highest` on a curved row, so short times, where
// small changes are heard, get most of it. A step is 0.1 below 10 and 1
// above. `unit` is "ms" or "s".
export function curvedScale(lowest: number, highest: number, unit: string): Scale {
  return {
    share(value: number) {
      return Math.cbrt((value - lowest) / (highest - lowest));
    },
    at(share: number) {
      const value = lowest + (highest - lowest) * share * share * share;
      let step = 1;
      if (value < 10) {
        step = 0.1;
      }
      return within(roundedTo(value, step), lowest, highest);
    },
    step(value: number, steps: number) {
      let step = 1;
      if (value < 10 || (value === 10 && steps < 0)) {
        step = 0.1;
      }
      return within(roundedTo(value + steps * step, step), lowest, highest);
    },
    words(value: number) {
      return trimmed(value, 1);
    },
    unit() {
      return unit;
    },
  };
}

// A number from `lowest` to `highest` on an even row, stepped by `step`,
// like a gain in dB. `signed` writes a plus before numbers over 0.
export function linearScale(lowest: number, highest: number, step: number, unit: string, signed = false): Scale {
  let decimals = 0;
  if (step < 1) {
    decimals = 1;
  }
  if (step < 0.1) {
    decimals = 2;
  }
  return {
    share(value: number) {
      return (value - lowest) / (highest - lowest);
    },
    at(share: number) {
      return within(roundedTo(lowest + share * (highest - lowest), step), lowest, highest);
    },
    step(value: number, steps: number) {
      return within(roundedTo(value + steps * step, step), lowest, highest);
    },
    words(value: number) {
      const words = trimmed(value, decimals);
      if (signed && value > 0) {
        return `+${words}`;
      }
      return words;
    },
    unit() {
      return unit;
    },
  };
}

// One of `steps` only, like a cut's slope or a note length in beats, each
// dot one step. `words` writes one and `unit` its unit.
export function stepsScale(
  steps: number[],
  words: (value: number) => string,
  unit: (value: number) => string,
): Scale {
  function nearest(value: number): number {
    let best = 0;
    for (let place = 1; place < steps.length; place += 1) {
      if (Math.abs(steps[place] - value) < Math.abs(steps[best] - value)) {
        best = place;
      }
    }
    return best;
  }
  return {
    share(value: number) {
      return nearest(value) / (steps.length - 1);
    },
    at(share: number) {
      const place = Math.round(within(share, 0, 1) * (steps.length - 1));
      return steps[place];
    },
    step(value: number, count: number) {
      const place = within(nearest(value) + count, 0, steps.length - 1);
      return steps[place];
    },
    words(value: number) {
      return words(value);
    },
    unit(value: number) {
      return unit(value);
    },
  };
}

// Note lengths in beats, with dotted and triplet ones, from 1/16 to `most`.
const BEAT_STEPS = [
  1 / 16, 1 / 12, 3 / 32, 1 / 8, 1 / 6, 3 / 16, 1 / 4, 1 / 3, 3 / 8, 1 / 2, 2 / 3, 3 / 4, 1, 4 / 3, 3 / 2, 2, 3, 4, 6, 8,
  12, 16, 24, 32,
];

// A length in beats written as a fraction: "3/16", "1/3", "2".
export function beatWords(beats: number): string {
  if (beats >= 1 && Number.isInteger(Number(beats.toFixed(4)))) {
    return String(Math.round(beats));
  }
  for (let under = 2; under <= 32; under += 1) {
    const over = beats * under;
    if (Math.abs(over - Math.round(over)) < 0.0001) {
      return `${Math.round(over)}/${under}`;
    }
  }
  return trimmed(beats, 3);
}

export function beatScale(most: number): Scale {
  const steps: number[] = [];
  for (const step of BEAT_STEPS) {
    if (step <= most) {
      steps.push(step);
    }
  }
  return stepsScale(steps, beatWords, (beats) => {
    if (beats === 1) {
      return "beat";
    }
    return "beats";
  });
}

// A compressor's ratio: 1:1 to 20:1, then "limit", kept here as 21.
export const LIMIT_RATIO = 21;
export const RATIO: Scale = {
  share(ratio: number) {
    return (ratio - 1) / (LIMIT_RATIO - 1);
  },
  at(share: number) {
    return within(roundedTo(1 + share * (LIMIT_RATIO - 1), 0.5), 1, LIMIT_RATIO);
  },
  step(ratio: number, steps: number) {
    return within(roundedTo(ratio + steps * 0.5, 0.5), 1, LIMIT_RATIO);
  },
  words(ratio: number) {
    if (ratio >= LIMIT_RATIO) {
      return "limit";
    }
    return trimmed(ratio, 1);
  },
  unit(ratio: number) {
    if (ratio >= LIMIT_RATIO) {
      return "";
    }
    return ":1";
  },
};

// A share from -1 to 1 written in percent with its sign, like a
// saturation's tone.
export const SIGNED_SHARE: Scale = {
  share(value: number) {
    return (value + 1) / 2;
  },
  at(share: number) {
    return within(roundedTo(share * 2 - 1, 0.01), -1, 1);
  },
  step(value: number, steps: number) {
    return within(roundedTo(value + steps * 0.01, 0.01), -1, 1);
  },
  words(value: number) {
    const percent = Math.round(value * 100);
    if (percent > 0) {
      return `+${percent}`;
    }
    return String(percent);
  },
  unit() {
    return "%";
  },
};

// A percent from 0 to `most`.
export function percentScale(most: number): Scale {
  return linearScale(0, most, 1, "%");
}

// A pan from fully left to fully right, written "L24", "C" or "R12".
export const PAN: Scale = {
  share(pan: number) {
    return pan / 100;
  },
  at(share: number) {
    return within(Math.round(share * 100), -100, 100);
  },
  step(pan: number, steps: number) {
    return within(Math.round(pan) + steps, -100, 100);
  },
  words(pan: number) {
    return panWords(pan);
  },
  unit() {
    return "";
  },
};

// A value of one slot's effect, as the arrow keys and its row see it. The
// panel's ids start with the place and slot, so a value stays picked only
// in its own slot.
export function effectSetting(
  place: string,
  slot: number,
  word: string,
  name: string,
  scale: Scale,
  now: () => number,
  send: MixSender,
): Setting {
  return {
    id: `mix ${place} ${slot} ${word}`,
    name,
    scale,
    now,
    set: (value) => send.setEffectValue(place, slot, word, value),
  };
}

// A value's number, or `fallback` for a word or a switch.
export function numberOf(value: Written | undefined, fallback = 0): number {
  if (typeof value === "number") {
    return value;
  }
  return fallback;
}

// The places a "listens to" can name, other than its own input: every
// track but its own, and the buses, as "track 2" and "bus 1" with short
// names for the tiles.
export function listenPlaces(view: MixView, place: string): { word: string; name: string }[] {
  const places: { word: string; name: string }[] = [];
  for (let track = 1; track <= view.tracks.length; track += 1) {
    const word = `track ${track}`;
    if (word !== place) {
      places.push({ word, name: String(track) });
    }
  }
  for (let bus = 1; bus <= view.buses.length; bus += 1) {
    const word = `bus ${bus}`;
    if (word !== place) {
      places.push({ word, name: `B${bus}` });
    }
  }
  return places;
}
