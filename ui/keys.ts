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
}

// What the mapping page sends to Rust. Each answers with the page as it is
// after, or null if Rust refused, which the top bar then says.
export interface MappingSender {
  look(section: number | null): Promise<MappingView | null>;
  setNotes(section: number | null, key: string, notes: number[]): Promise<MappingView | null>;
  setLocked(section: number | null, key: string, locked: boolean): Promise<MappingView | null>;
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
