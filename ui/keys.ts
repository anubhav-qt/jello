// The song's key mappings as the page reads them (src-tauri/src/keys_tool.rs,
// docs/adr/0047-song-key-mappings.md).

import type { KeyLayout } from "./keyboard";

// What the mapping page shows.
export interface MappingView {
  scale: string | null;
  // The section being edited, from 0, or null for the main mapping.
  section: number | null;
  sections: { name: string; own: boolean }[];
  // Each key's notes, lowest first, and what they are called.
  keys: Record<string, number[]>;
  names: Record<string, string>;
  locked: string[];
  // Every key a mapping can use.
  mappable: string[];
  // What the same keys play on a guitar's track, its strings thickest first.
  guitar: GuitarMapping;
  guitar_strings: string[];
  guitar_shape_keys: string[];
}

// A guitar's six strings, thickest first: each one's note, or null for silent.
export type GuitarStrings = (number | null)[];

export interface GuitarMapping {
  open: GuitarStrings;
  shapes: Record<string, { name: string; strings: GuitarStrings }>;
  // How long each string of a strum waits after the one before, in ms.
  strum_ms: number;
}

// A strum's speed: 0 (all at once) to STRUM_SLOWEST_MS, in steps of
// STRUM_STEP_MS on the page (src-tauri/src/guitar.rs).
export const STRUM_SLOWEST_MS = 100;
export const STRUM_STEP_MS = 5;

// What the mapping page sends to Rust. Each answers with the page as it is
// after, or null if Rust refused, which the top bar then says.
export interface MappingSender {
  look(section: number | null): Promise<MappingView | null>;
  setNotes(section: number | null, key: string, notes: number[]): Promise<MappingView | null>;
  setLocked(section: number | null, key: string, locked: boolean): Promise<MappingView | null>;
  // Puts `strings` on shape key `key`, or with null tunes the open strings.
  setGuitar(section: number | null, key: string | null, strings: GuitarStrings): Promise<MappingView | null>;
  setStrum(section: number | null, ms: number): Promise<MappingView | null>;
  reset(section: number | null): Promise<MappingView | null>;
  soundNote(note: number, on: boolean): void;
}

// The keyboard's layout, with `view`'s notes and chords on its keys instead.
// The mapping page draws what is being edited, whatever the keys play now.
export function layoutOfMapping(view: MappingView, layout: KeyLayout): KeyLayout {
  const notes: Record<string, number> = {};
  const chords: Record<string, { name: string; notes: number[] }> = {};
  for (const [key, played] of Object.entries(view.keys)) {
    if (played.length === 1) {
      notes[key] = played[0];
    } else {
      chords[key] = { name: view.names[key], notes: played };
    }
  }
  return { ...layout, notes, chords, drums: {}, drum_mode: false };
}

// The keyboard's layout with `view`'s guitar on it: the shape keys' names, and
// what the strings play with `shape` held, or open with null.
export function layoutOfGuitar(view: MappingView, layout: KeyLayout, shape: string | null): KeyLayout {
  const notes: Record<string, number> = {};
  const chords: Record<string, { name: string; notes: number[] }> = {};
  let strings = view.guitar.open;
  if (shape !== null && view.guitar.shapes[shape] !== undefined) {
    strings = view.guitar.shapes[shape].strings;
  }
  view.guitar_strings.forEach((key, string) => {
    const note = strings[string];
    if (note !== null) {
      notes[key] = note;
    }
  });
  for (const [key, found] of Object.entries(view.guitar.shapes)) {
    const played = found.strings.filter((note): note is number => note !== null);
    chords[key] = { name: found.name, notes: played };
  }
  return { ...layout, notes, chords, drums: {}, drum_mode: false };
}

// `strings` with string `string` moved `steps` half steps. A silent one starts
// from `from`, the note it would play open. It stays from 0 to 127.
export function withStringMoved(strings: GuitarStrings, string: number, steps: number, from: number): GuitarStrings {
  const moved = [...strings];
  const now = moved[string];
  if (now === null) {
    moved[string] = from;
    return moved;
  }
  moved[string] = Math.max(0, Math.min(127, now + steps));
  return moved;
}

// `notes` with `note` added, lowest first, or taken out if it was there.
export function withNoteSwitched(notes: number[], note: number): number[] {
  if (notes.includes(note)) {
    return notes.filter((held) => held !== note);
  }
  const added = [...notes, note];
  added.sort((low, high) => low - high);
  return added;
}

// The drawn piano, from C1 to C8.
export const PIANO_LOWEST = 24;
export const PIANO_HIGHEST = 108;

const BLACK_PITCHES = [1, 3, 6, 8, 10];

export function isBlackNote(note: number): boolean {
  return BLACK_PITCHES.includes(note % 12);
}

// The home notes to pick from, by pitch, spelled the usual way.
export const HOME_NOTES = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

// The kinds of scale, as Rust's ScaleKind::word says them.
export const SCALE_KINDS = [
  "major",
  "minor",
  "dorian",
  "mixolydian",
  "lydian",
  "harmonic minor",
  "melodic minor",
  "phrygian",
  "major pentatonic",
  "minor pentatonic",
  "blues",
];
