// What Compose shows, as Rust's Compose commands answer
// (src-tauri/src/compose.rs).

// How well a chord or a melody note fits the song's key. A note is never
// "can_work".
export type Fit = "in_key" | "can_work" | "outside";

// What guidance says about one chord.
export interface ChordGuide {
  // Its name, spelled for the key, like "Bb" or "F#m".
  name: string;
  fit: Fit;
  // Why it fits, or doesn't, in a few plain words.
  why: string;
}

// A key the player can pick, like Db major.
export interface KeyChoice {
  // The home note, from 0 (C) to 11 (B).
  tonic: number;
  mode: "major" | "minor";
  name: string;
}

// A chord in the song, kept as long as it was held.
export interface ChordInSong {
  // Its name, spelled for the key.
  name: string;
  // Its root as a pitch, from 0 (C) to 11 (B), for its color.
  root: number;
  // How long it was held, in milliseconds. While it still is, how long so
  // far when Rust answered.
  held_ms: number;
  // Whether it is still held, so it is still growing.
  still_held: boolean;
}

// Where the song is playing.
export interface Playhead {
  // The chord playing, by its place in the song, from 0.
  chord: number;
  // How far into it, in milliseconds.
  ms_in: number;
}

export interface ComposeView {
  // Every key there is to pick: the 12 major keys, then the 12 minor keys.
  keys: KeyChoice[];
  // The key picked, or null before one is.
  key: KeyChoice | null;
  // The chords added so far.
  progression: ChordInSong[];
  // The chord being held that isn't in the song yet, or null. The tape
  // draws it as a dashed block growing at the end.
  holding: ChordInSong | null;
  // For each chord key, what guidance says about its chord. Empty until a
  // key is picked.
  guide: Record<string, ChordGuide>;
  // For each melody key, whether its note is in the key or outside it.
  // Empty until a key is picked.
  melody: Record<string, Fit>;
  // Where the song is playing, or null while it isn't. The tape draws the
  // playhead from it.
  playing: Playhead | null;
}

// The fit in a few words, for Trying.
export function fitWords(fit: Fit): string {
  if (fit === "in_key") {
    return "in the key";
  }
  if (fit === "can_work") {
    return "outside the key, but can work";
  }
  return "outside the key";
}

// A length in seconds, like "1.4 s".
export function secondsWords(ms: number): string {
  const seconds = ms / 1000;
  return `${seconds.toFixed(1)} s`;
}

// The song so far, like "4 chords, 9.2 s".
export function songWords(chordCount: number, ms: number): string {
  if (chordCount === 0) {
    return "No chords yet";
  }
  let chords = `${chordCount} chords`;
  if (chordCount === 1) {
    chords = "1 chord";
  }
  return `${chords}, ${secondsWords(ms)}`;
}

// A key's name in the picker, like "Db" for Db major and "C#m" for C# minor.
export function pickerName(choice: KeyChoice): string {
  const tonic = choice.name.split(" ")[0];
  if (choice.mode === "minor") {
    return `${tonic}m`;
  }
  return tonic;
}
