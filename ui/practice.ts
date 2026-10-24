// What the Play tab shows of practice (src-tauri/src/practice.rs): the
// piece set, its settings, the lane and every try's results.

export interface PracticeNote {
  index: number;
  label: string;
  pitches: number[];
  start: number;
  hold: number;
  moved: number | null;
}

export interface PracticePart {
  name: string;
  hand: "left" | "right" | "jello";
  player: boolean;
  notes: PracticeNote[];
}

export interface PracticeLane {
  kind: "lane";
  title: string;
  beats: number;
  tempo: number;
  parts: PracticePart[];
}

export interface Ladder {
  step: number;
  clean: number;
  top: number;
}

export interface PracticeSettings {
  tempo: number;
  swing: number;
  count_in: number;
  click: boolean;
  wait: boolean;
  section: string | null;
  loop: boolean;
  ladder: Ladder | null;
  hands: "both" | "left" | "right";
  mode: "along" | "ear" | "improvise" | "memory";
  coach: boolean;
  level: string;
}

export interface Tally {
  notes: number;
  hits: number;
  missed: number;
  accuracy_percent: number;
  mean_offset_ms: number | null;
  spread_ms: number | null;
  hold_percent: number | null;
}

// A try's results. Which fields come depends on its mode.
export interface PracticeTry {
  try: number;
  piece: string;
  mode: string;
  wait: boolean;
  tempo: number;
  level: string;
  clean: boolean;
  whole?: Tally;
  left?: Tally | null;
  right?: Tally | null;
  sections?: { name: string; tally: Tally }[];
  wrong?: number;
  extras?: number;
  together_ms?: number | null;
  echo?: { steps: number; right: number; first_wrong: number | null; extra_presses: number };
  improvised?: {
    notes: number;
    in_key_percent: number | null;
    chord_tones_on_strong_beats_percent: number | null;
    rhythm_kinds: number;
    motif_repeats: number;
  };
  // In wait mode.
  waited?: { steps: number; wrong: number; seconds: number };
}

export interface PracticeView {
  set: boolean;
  piece?: {
    name: string;
    bars: number;
    octave: number;
    parts: { name: string; hand: string; notes: number; player: boolean }[];
    sections: { name: string; from_bar: number; to_bar: number }[];
  };
  settings?: PracticeSettings;
  window_ms?: number;
  tempo_now?: number;
  tempo_note?: string | null;
  lane?: PracticeLane;
  running?: boolean;
  phase?: string;
  tries?: PracticeTry[];
}

// A key practice marks: the next to press, or one after it, numbered.
export interface PracticeMark {
  key: string;
  label: string;
  state: "next" | "later";
}

export const MODES = ["along", "ear", "improvise", "memory"] as const;
export const HANDS = ["both", "left", "right"] as const;
export const LEVELS = ["beginner", "novice", "intermediate", "advanced", "expert", "master"] as const;
export const SWINGS = [50, 58, 66, 75] as const;

export const MODE_WORDS: Record<string, string> = {
  along: "Play along",
  ear: "By ear",
  improvise: "Improvise",
  memory: "From memory",
};

// The value after `value` in `values`, round again after the last.
export function nextOf<T>(values: readonly T[], value: T): T {
  const place = values.indexOf(value);
  return values[(place + 1) % values.length];
}

// An offset as words: "on time", "12 ms late", "30 ms early".
export function offsetWords(offset: number | null | undefined): string {
  if (offset === null || offset === undefined) {
    return "-";
  }
  if (Math.abs(offset) < 5) {
    return "on time";
  }
  if (offset > 0) {
    return `${Math.round(offset)} ms late`;
  }
  return `${Math.round(-offset)} ms early`;
}

// The try's headline number, as text: accuracy, or right steps, or notes
// in key.
export function headline(attempt: PracticeTry): string {
  if (attempt.whole !== undefined) {
    return `${attempt.whole.accuracy_percent}%`;
  }
  if (attempt.echo !== undefined) {
    return `${attempt.echo.right}/${attempt.echo.steps}`;
  }
  if (attempt.improvised !== undefined) {
    const inKey = attempt.improvised.in_key_percent;
    if (inKey === null) {
      return "-";
    }
    return `${inKey}%`;
  }
  if (attempt.waited !== undefined) {
    return `${attempt.waited.seconds} s`;
  }
  return "-";
}
