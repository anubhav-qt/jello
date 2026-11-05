// The keys the picked track's loop presses as it plays, so the keyboard can
// light them (docs/design.md#tracks-and-sections). The lane shows the
// picked track's layers; each press in them, the notes that start and end
// together, lights the key that plays exactly those notes now, at the keys'
// octave. A press no key plays now lights nothing.

import type { KeyLayout } from "./keyboard";
import type { LayerView, LoopNote, LooperView } from "./sound";

// How many beats make a bar (`BEATS_PER_BAR` in src-tauri/src/engine.rs).
const BEATS_PER_BAR = 4;

// The drum pads' notes start here (`DRUM_FIRST_NOTE` in
// src-tauri/src/drums.rs), one per pad, in the kit's pad order.
const DRUM_FIRST_NOTE = 130;

// One press of a layer: the key that plays it and when, in beats from the
// layer's start.
export interface KeyPress {
  key: string;
  start: number;
  end: number;
}

// One layer's presses, and whether the layer is heard: playing, and not
// being recorded, since while recording the player's own presses light
// their keys.
export interface LayerPresses {
  bars: number;
  heard: boolean;
  presses: KeyPress[];
}

// Orders key names the way Rust sorts strings, letter code by letter code.
function byName(a: string, b: string): number {
  if (a < b) {
    return -1;
  }
  if (a > b) {
    return 1;
  }
  return 0;
}

// A list of notes as one word, lowest first, like "60,64,67".
function notesWord(notes: number[]): string {
  const sorted = [...notes];
  sorted.sort((a, b) => a - b);
  return sorted.join(",");
}

// Each set of notes a key plays now, as a word, with the key that plays
// it. Where two keys play the same notes, the first by name lights, the
// way Rust picks one (`KeyMap::keys_playing`). Drum keys hit pads by name;
// `padNames` holds the kit's pads in order.
export function keysByNotes(layout: KeyLayout, padNames: string[]): Map<string, string> {
  const found = new Map<string, string>();
  const keys: [string, number[]][] = [];
  for (const [key, note] of Object.entries(layout.notes)) {
    keys.push([key, [note]]);
  }
  for (const [key, chord] of Object.entries(layout.chords)) {
    keys.push([key, chord.notes]);
  }
  for (const [key, pad] of Object.entries(layout.drums)) {
    const place = padNames.indexOf(pad);
    if (place >= 0) {
      keys.push([key, [DRUM_FIRST_NOTE + place]]);
    }
  }
  keys.sort((a, b) => byName(a[0], b[0]));
  for (const [key, notes] of keys) {
    const word = notesWord(notes);
    if (!found.has(word)) {
      found.set(word, key);
    }
  }
  return found;
}

// The presses in `notes`: notes that start and end on the same beat were
// one press. Notes still held, with no end yet, are left out.
function pressesOf(notes: LoopNote[]): { notes: number[]; start: number; end: number }[] {
  const byTime = new Map<string, { notes: number[]; start: number; end: number }>();
  for (const note of notes) {
    if (note.end === null) {
      continue;
    }
    const time = `${note.start}|${note.end}`;
    const press = byTime.get(time);
    if (press === undefined) {
      byTime.set(time, { notes: [note.note], start: note.start, end: note.end });
    } else {
      press.notes.push(note.note);
    }
  }
  return [...byTime.values()];
}

// Each of `layers`' presses that a key plays now, by `keys`.
export function layerPresses(layers: LayerView[], keys: Map<string, string>): LayerPresses[] {
  const all: LayerPresses[] = [];
  for (const layer of layers) {
    const presses: KeyPress[] = [];
    for (const press of pressesOf(layer.notes)) {
      const key = keys.get(notesWord(press.notes));
      if (key !== undefined) {
        presses.push({ key, start: press.start, end: press.end });
      }
    }
    let heard = layer.playing;
    if (layer.recording) {
      heard = false;
    }
    all.push({ bars: layer.bars, heard, presses });
  }
  return all;
}

// The keys pressed now, `beats` beats into the loop playing, by each heard
// layer at its own place: a layer shorter than the loop goes round on its
// own. None while the loop is not playing.
export function keysPressedNow(looper: LooperView, layers: LayerPresses[], beats: number): Set<string> {
  const pressed = new Set<string>();
  if (looper.state !== "looping" && looper.state !== "layering") {
    return pressed;
  }
  for (const layer of layers) {
    if (!layer.heard) {
      continue;
    }
    let bars = layer.bars;
    if (bars === 0) {
      bars = Math.max(1, looper.number);
    }
    const place = beats % (bars * BEATS_PER_BAR);
    for (const press of layer.presses) {
      if (press.start <= place && place < press.end) {
        pressed.add(press.key);
      }
    }
  }
  return pressed;
}
