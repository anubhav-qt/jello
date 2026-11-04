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
  // The piece's section played, like "Bar 2", or null for all of it.
  section?: string | null;
  whole?: Tally;
  left?: Tally | null;
  right?: Tally | null;
  sections?: { name: string; tally: Tally }[];
  notes?: NoteResult[];
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
    // Set when the piece is a section of the player's own song.
    song?: SongPiece | null;
  };
  settings?: PracticeSettings;
  window_ms?: number;
  tempo_now?: number;
  tempo_note?: string | null;
  lane?: PracticeLane;
  // A song piece's lane over all its bars.
  whole_lane?: PracticeLane | null;
  clean_in_row?: number;
  running?: boolean;
  phase?: string;
  tries?: PracticeTry[];
}

// A section of the player's song as a piece (src-tauri/src/practice_song.rs):
// the key each note is played on, by the note's place, and what each bar
// needs, from bar 1.
export interface SongPiece {
  song: string;
  section: string;
  track: string;
  keys: string[];
  bar_skills: string[][];
  // Presses of the track that no key plays now, left out of the piece.
  left_out: number;
}

// One note's result in a try.
export interface NoteResult {
  index: number;
  hit: boolean;
  offset_ms: number | null;
}

// The key's letter as printed, like "C" for "KeyC" or "6" for "Digit6".
export function keyLetter(key: string): string {
  if (key.startsWith("Key")) {
    return key.slice(3);
  }
  if (key.startsWith("Digit")) {
    return key.slice(5);
  }
  const printed: Record<string, string> = {
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
  };
  return printed[key] ?? key;
}

// The tempos of a ladder from `start` to its top, in its steps.
export function ladderSteps(start: number, ladder: Ladder | null): number[] {
  if (ladder === null) {
    return [start];
  }
  const steps = [start];
  let tempo = start;
  while (tempo < ladder.top) {
    tempo = Math.min(tempo + ladder.step, ladder.top);
    steps.push(tempo);
  }
  return steps;
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

// An offset as words: "on time", "12 ms late", "30 ms early", or "none
// hit" when no note was hit, so there is no timing.
export function offsetWords(offset: number | null | undefined): string {
  if (offset === null || offset === undefined) {
    return "none hit";
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
