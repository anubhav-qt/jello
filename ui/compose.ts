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
  // Whether it is one of the best chords to play next, or null if it isn't
  // (docs/adr/0034-best-next-chords.md).
  next: BestNext | null;
}

// One of the best chords to play next.
export interface BestNext {
  // 1 for the strongest, then 2 and 3.
  rank: number;
  // Why it follows the song's last chord well.
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
  // The chord keys that play the chord playing, like ["KeyC"].
  now_keys: string[];
  // The chord keys that play the chord after it. Empty when the song stops
  // after the chord playing.
  next_keys: string[];
}

// The chord keys the keyboard lights while Compose's song plays.
export interface SongKeys {
  // The keys for the chord playing.
  now: string[];
  // The keys for the chord after it. Empty at the song's end.
  next: string[];
  // How far the song is through the chord playing, from 0 at its start to
  // 1 at its end, so the next keys fill up as their chord comes.
  share: number;
}

export interface PerformanceEvent {
  id: number;
  key: string;
  notes: number[];
  pressed_ms: number;
  held_ms: number | null;
}

export interface ComposeView {
  performance: PerformanceEvent[];
  history: ChordInSong[];
  selected_history: number | null;
  reference: ChordInSong | null;
  live_guide: Record<string, ChordGuide>;
  compatible: Record<string, string>;
  melody_next: Record<string, BestNext>;
  last_note: number | null;
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

// How long `chord` is, in milliseconds, `sinceAnswer` milliseconds after
// Rust said: one still held has grown by that much.
export function lengthSince(chord: ChordInSong, sinceAnswer: number): number {
  if (chord.still_held) {
    return chord.held_ms + sinceAnswer;
  }
  return chord.held_ms;
}

// How far the song is through the chord playing, from 0 at its start to 1
// at its end, `sinceAnswer` milliseconds after Rust said where it was.
export function playingShare(
  playing: Playhead,
  progression: ChordInSong[],
  sinceAnswer: number,
): number {
  const chord = progression[playing.chord];
  const length = lengthSince(chord, sinceAnswer);
  if (length === 0) {
    return 0;
  }
  const msIn = playing.ms_in + sinceAnswer;
  return Math.min(msIn / length, 1);
}

// While the song plays, the chord keys the keyboard lights, `sinceAnswer`
// milliseconds after Rust said where it was. Null while it isn't playing.
export function songKeysOf(
  view: ComposeView,
  sinceAnswer: number,
): SongKeys | null {
  if (view.playing === null) {
    return null;
  }
  return {
    now: view.playing.now_keys,
    next: view.playing.next_keys,
    share: playingShare(view.playing, view.progression, sinceAnswer),
  };
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

// What the sections lane and a section's details show
// (`SectionsView` in src-tauri/src/sections.rs). Sections, tracks, layers
// and places in the order count from 1.
export interface SectionsView {
  // Every section, in the order they were saved.
  sections: SectionView[];
  // The order the song plays them in, as section numbers.
  order: number[];
  // The place in the order playing now, or null.
  playing: number | null;
  // Each track's name and how many layers it has.
  tracks: TrackLayers[];
  // The song's tempo, so a section shows its own only if it differs.
  tempo: number;
}

export interface SectionView {
  name: string;
  bars: number;
  tempo: number;
  // In half steps, from -12 to 12.
  key_shift: number;
  // The key it plays in, with its shift, or null before a key is picked.
  key: string | null;
  // The layers it plays, as [track, layer].
  layers: [number, number][];
  automation: AutomationView[];
}

// A value a section moves from `from` to `to` across it.
export interface AutomationView {
  // null for the tempo.
  track: number | null;
  value: string;
  from: number;
  to: number;
}

export interface TrackLayers {
  name: string;
  layers: number;
}

// One change to the sections (`SectionEdit` in src-tauri/src/sections.rs).
export type SectionEdit =
  | { action: "save" }
  | { action: "remove_last" }
  | { action: "save_again"; section: number }
  | { action: "open"; section: number }
  | { action: "rename"; section: number; name: string }
  | { action: "bars"; section: number; bars: number }
  | { action: "tempo"; section: number; tempo: number }
  | { action: "key_shift"; section: number; key_shift: number }
  | {
      action: "layer";
      section: number;
      track: number;
      layer: number;
      on: boolean;
    }
  | {
      action: "automate";
      section: number;
      track: number | null;
      value: string;
      from: number;
      to: number;
    }
  | {
      action: "stop_automating";
      section: number;
      track: number | null;
      value: string;
    }
  | { action: "move"; place: number; to: number }
  | { action: "use_again"; place: number }
  | { action: "remove_use"; place: number }
  | { action: "play" }
  | { action: "stop" };

// What a section's block says under its name: its bars, and its tempo and
// key shift when they differ from the song's.
export function sectionWords(section: SectionView, songTempo: number): string {
  const words = [barsWords(section.bars)];
  if (section.tempo !== songTempo) {
    words.push(`${section.tempo} bpm`);
  }
  if (section.key_shift > 0) {
    words.push(`+${section.key_shift}`);
  } else if (section.key_shift < 0) {
    words.push(`${section.key_shift}`);
  }
  return words.join(", ");
}

export function barsWords(bars: number): string {
  if (bars === 1) {
    return "1 bar";
  }
  return `${bars} bars`;
}
