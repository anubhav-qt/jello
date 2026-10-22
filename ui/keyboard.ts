// The laptop keyboards Jello can draw: where each key sits, what is printed
// on it, and the note it plays. Keys are named by their place on the
// keyboard, the way `event.code` names them ("KeyQ", "Slash"), just like
// Rust's key map (src-tauri/src/key_map.rs) names them.

export type Platform = "mac" | "windows";

// A chord as Rust names it, like "Bb" or "F#m", with its notes, lowest first.
export interface LayoutChord {
  name: string;
  notes: number[];
}

// What each key does, as Rust's `key_layout` command answers.
export interface KeyLayout {
  // Each key that plays one note, with its note: "KeyM" plays 60.
  notes: Record<string, number>;
  // Each key that plays a chord, with its chord: "KeyX" plays Cm.
  chords: Record<string, LayoutChord>;
  // The keys that move new notes on to the next wave.
  wave_keys: string[];
  // The keys held for vibrato: left Shift.
  vibrato_keys: string[];
  // The keys that run the beat, each with what it does: right Shift runs
  // the looper, Esc clears it, - slows the beat down and = speeds it up.
  beat_keys: Record<string, BeatKey>;
}

// What a key that runs the beat does.
export type BeatKey = "looper" | "clear" | "slower" | "faster";

// One key, as drawn.
export interface KeySpec {
  // The key's place, like "KeyQ". Empty for keys the webview never hears
  // about, like fn and Touch ID.
  code: string;
  // What is printed on it, like "Q" or "delete".
  label: string;
  // How wide it is, in key widths.
  width: number;
  // A symbol printed above the label, like the ⌘ on command.
  symbol?: string;
  // A half-height key, like the arrow keys on a Mac.
  half?: boolean;
  // A key stacked under this one, in the same spot: down under up.
  below?: KeySpec;
  // Not a key, just room between keys, like the gaps in a PC's top row.
  gap?: boolean;
}

// One row of keys, left to right.
export interface KeyRow {
  keys: KeySpec[];
  // The function keys' row is drawn shorter than the others.
  short?: boolean;
}

// What a key is for, which decides how it is drawn. A "vibrato" key is held
// for vibrato, a "beat" key runs the beat, and an "action" key does
// something on the open tab, like Compose's Return.
export type KeyKind = "note" | "chord" | "wave" | "vibrato" | "beat" | "action" | "plain" | "blank";

// The keyboard to draw: a Mac's, or a PC's on Windows.
export function detectPlatform(): Platform {
  if (navigator.userAgent.includes("Mac")) {
    return "mac";
  }
  return "windows";
}

const PITCH_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

// The name of MIDI note `note`, in two parts: its pitch, like "C#", and its
// octave. Middle C (60) is C4.
export function noteName(note: number): { pitch: string; octave: number } {
  const pitch = PITCH_NAMES[note % 12];
  const octave = Math.floor(note / 12) - 1;
  return { pitch, octave };
}

function key(code: string, label: string, width = 1): KeySpec {
  return { code, label, width };
}

// A modifier key, like command: a symbol above a word.
function modifier(code: string, symbol: string, label: string, width = 1): KeySpec {
  return { code, label, width, symbol };
}

function halfKey(code: string, label: string): KeySpec {
  return { code, label, width: 1, half: true };
}

function gap(width: number): KeySpec {
  return { code: "", label: "", width, gap: true };
}

// One key per letter, like "QWERTYUIOP".
function letters(text: string): KeySpec[] {
  const keys: KeySpec[] = [];
  for (const letter of text) {
    keys.push(key(`Key${letter}`, letter));
  }
  return keys;
}

// The number keys, 1 to 9 and then 0.
function digits(): KeySpec[] {
  const keys: KeySpec[] = [];
  for (const digit of "1234567890") {
    keys.push(key(`Digit${digit}`, digit));
  }
  return keys;
}

// A MacBook's keyboard (US). Every row is 14.5 keys wide.
function macRows(): KeyRow[] {
  const functionKeys = [key("Escape", "esc", 1.5)];
  for (let number = 1; number <= 12; number += 1) {
    functionKeys.push(key(`F${number}`, `F${number}`));
  }
  // Touch ID.
  functionKeys.push(key("", ""));

  const up = halfKey("ArrowUp", "▲");
  up.below = halfKey("ArrowDown", "▼");

  return [
    { keys: functionKeys, short: true },
    {
      keys: [
        key("Backquote", "`"),
        ...digits(),
        key("Minus", "-"),
        key("Equal", "="),
        key("Backspace", "delete", 1.5),
      ],
    },
    {
      keys: [
        key("Tab", "tab", 1.5),
        ...letters("QWERTYUIOP"),
        key("BracketLeft", "["),
        key("BracketRight", "]"),
        key("Backslash", "\\"),
      ],
    },
    {
      keys: [
        key("CapsLock", "caps lock", 1.75),
        ...letters("ASDFGHJKL"),
        key("Semicolon", ";"),
        key("Quote", "'"),
        key("Enter", "return", 1.75),
      ],
    },
    {
      keys: [
        key("ShiftLeft", "shift", 2.25),
        ...letters("ZXCVBNM"),
        key("Comma", ","),
        key("Period", "."),
        key("Slash", "/"),
        key("ShiftRight", "shift", 2.25),
      ],
    },
    {
      keys: [
        key("", "fn"),
        modifier("ControlLeft", "⌃", "control"),
        modifier("AltLeft", "⌥", "option"),
        modifier("MetaLeft", "⌘", "command", 1.25),
        key("Space", "", 5),
        modifier("MetaRight", "⌘", "command", 1.25),
        modifier("AltRight", "⌥", "option"),
        halfKey("ArrowLeft", "◀"),
        up,
        halfKey("ArrowRight", "▶"),
      ],
    },
  ];
}

// A PC keyboard's main block (US), as on Windows. Every row is 15 keys wide.
function windowsRows(): KeyRow[] {
  const functionKeys = [key("Escape", "Esc"), gap(1)];
  for (let number = 1; number <= 12; number += 1) {
    functionKeys.push(key(`F${number}`, `F${number}`));
    // F1 to F12 come in groups of four.
    if (number === 4 || number === 8) {
      functionKeys.push(gap(0.5));
    }
  }

  return [
    { keys: functionKeys, short: true },
    {
      keys: [
        key("Backquote", "`"),
        ...digits(),
        key("Minus", "-"),
        key("Equal", "="),
        key("Backspace", "Backspace", 2),
      ],
    },
    {
      keys: [
        key("Tab", "Tab", 1.5),
        ...letters("QWERTYUIOP"),
        key("BracketLeft", "["),
        key("BracketRight", "]"),
        key("Backslash", "\\", 1.5),
      ],
    },
    {
      keys: [
        key("CapsLock", "Caps Lock", 1.75),
        ...letters("ASDFGHJKL"),
        key("Semicolon", ";"),
        key("Quote", "'"),
        key("Enter", "Enter", 2.25),
      ],
    },
    {
      keys: [
        key("ShiftLeft", "Shift", 2.25),
        ...letters("ZXCVBNM"),
        key("Comma", ","),
        key("Period", "."),
        key("Slash", "/"),
        key("ShiftRight", "Shift", 2.75),
      ],
    },
    {
      keys: [
        key("ControlLeft", "Ctrl", 1.25),
        key("MetaLeft", "Win", 1.25),
        key("AltLeft", "Alt", 1.25),
        key("Space", "", 6.25),
        key("AltRight", "Alt", 1.25),
        key("MetaRight", "Win", 1.25),
        key("ContextMenu", "Menu", 1.25),
        key("ControlRight", "Ctrl", 1.25),
      ],
    },
  ];
}

export function kindOf(code: string, layout: KeyLayout): KeyKind {
  if (code === "") {
    return "blank";
  }
  if (layout.notes[code] !== undefined) {
    return "note";
  }
  if (layout.chords[code] !== undefined) {
    return "chord";
  }
  if (layout.wave_keys.includes(code)) {
    return "wave";
  }
  if (layout.vibrato_keys.includes(code)) {
    return "vibrato";
  }
  if (layout.beat_keys[code] !== undefined) {
    return "beat";
  }
  return "plain";
}

// The notes key `code` plays, lowest first: one for a note key, three for a
// chord key, and none for any other key.
export function notesOf(code: string, layout: KeyLayout): number[] {
  const note = layout.notes[code];
  if (note !== undefined) {
    return [note];
  }
  const chord = layout.chords[code];
  if (chord !== undefined) {
    return chord.notes;
  }
  return [];
}

// The rows of keys to draw for `platform`.
export function keyRows(platform: Platform): KeyRow[] {
  if (platform === "mac") {
    return macRows();
  }
  return windowsRows();
}

// How many key widths wide the keyboard is on `platform`: 14.5 on a Mac and
// 15 on Windows. Every row is as wide, so it adds up the number row.
export function keysAcross(platform: Platform): number {
  const numberRow = keyRows(platform)[1];
  let width = 0;
  for (const spec of numberRow.keys) {
    width += spec.width;
  }
  return width;
}
