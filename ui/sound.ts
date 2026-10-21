// What Rust says about the sound after each key press (`SoundView` in
// src-tauri/src/commands.rs).
export interface SoundView {
  // The wave new notes play, like "saw".
  wave: string;
  // The beat's tempo, in beats per minute.
  tempo: number;
  // Whether loops snap to the beat: each time around, the loop plays its
  // notes moved to the nearest half beat.
  snap: boolean;
  // What the looper is doing.
  looper: LooperView;
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
