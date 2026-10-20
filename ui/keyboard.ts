// The laptop keyboards Jello can draw: where each key sits, what is printed
// on it, and the note it plays. Keys are named by their place on the
// keyboard, the way `event.code` names them ("KeyQ", "Slash"), just like
// Rust's key map (src-tauri/src/key_map.rs) names them.

import { makeElement } from "./dom";

export type Platform = "mac" | "windows";

// What each key does, as Rust's `key_layout` command answers.
export interface KeyLayout {
  // Each key that plays a note, with its note: "KeyM" plays 60.
  notes: Record<string, number>;
  // The keys that move new notes on to the next wave.
  wave_keys: string[];
}

// One key, as drawn.
interface KeySpec {
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
interface KeyRow {
  keys: KeySpec[];
  // The function keys' row is drawn shorter than the others.
  short?: boolean;
}

// What a key is for, which decides how it is drawn.
export type KeyKind = "note" | "wave" | "plain" | "blank";

// A drawn keyboard: its element, its rows, and each key's element by its place.
export interface DrawnKeyboard {
  element: HTMLElement;
  rows: HTMLElement[];
  keys: Map<string, HTMLElement>;
}

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

// A color for each of the 12 pitches, as a hue on the color wheel (0 to 360).
// Pitches are placed round the wheel in the order of the circle of fifths
// (C, G, D, A, E...), where neighbours share most of their notes and sound
// good together. So a chord's notes get colors that sit close together.
export function pitchHue(note: number): number {
  const pitch = note % 12;
  const placeOnCircle = (pitch * 7) % 12;
  return (placeOnCircle * 30 + 200) % 360;
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

function kindOf(code: string, layout: KeyLayout): KeyKind {
  if (code === "") {
    return "blank";
  }
  if (layout.notes[code] !== undefined) {
    return "note";
  }
  if (layout.wave_keys.includes(code)) {
    return "wave";
  }
  return "plain";
}

// A note's name as two parts, so the octave can be drawn smaller.
function drawNoteName(note: number): HTMLElement {
  const name = noteName(note);
  const element = makeElement("span", "key-note");
  element.append(makeElement("span", "key-pitch", name.pitch));
  element.append(makeElement("span", "key-octave", String(name.octave)));
  return element;
}

function drawKey(spec: KeySpec, layout: KeyLayout): HTMLElement {
  const element = makeElement("div", "key");
  element.style.setProperty("--w", String(spec.width));
  element.dataset.code = spec.code;

  const kind = kindOf(spec.code, layout);
  element.dataset.kind = kind;
  if (spec.half === true) {
    element.classList.add("is-half");
  }

  if (spec.symbol !== undefined) {
    element.append(makeElement("span", "key-symbol", spec.symbol));
  }
  const label = makeElement("span", "key-label", spec.label);
  // Words like "shift" are printed smaller than single letters.
  if (spec.label.length > 1) {
    label.classList.add("is-word");
  }
  element.append(label);

  if (kind === "note") {
    const note = layout.notes[spec.code];
    element.dataset.note = String(note);
    element.style.setProperty("--pitch-hue", String(pitchHue(note)));
    element.append(drawNoteName(note));
  }
  if (kind === "wave") {
    // The wave's name goes here once it is known (showWaveOnKeys).
    element.append(makeElement("span", "key-note", "wave"));
  }
  return element;
}

// Draws the keyboard for `platform`, with each key's note from `layout`.
export function drawKeyboard(platform: Platform, layout: KeyLayout): DrawnKeyboard {
  let rowSpecs = windowsRows();
  if (platform === "mac") {
    rowSpecs = macRows();
  }

  const element = makeElement("div", "keyboard");
  element.dataset.platform = platform;
  const rows: HTMLElement[] = [];
  const keys = new Map<string, HTMLElement>();

  for (const rowSpec of rowSpecs) {
    const row = makeElement("div", "keyboard-row");
    if (rowSpec.short === true) {
      row.classList.add("is-short");
    }

    for (const spec of rowSpec.keys) {
      if (spec.gap === true) {
        const room = makeElement("div", "key-gap");
        room.style.setProperty("--w", String(spec.width));
        row.append(room);
        continue;
      }

      const keyElement = drawKey(spec, layout);
      if (spec.code !== "") {
        keys.set(spec.code, keyElement);
      }

      // A key with another stacked under it shares its spot in the row.
      if (spec.below === undefined) {
        row.append(keyElement);
        continue;
      }
      const stack = makeElement("div", "key-stack");
      stack.style.setProperty("--w", String(spec.width));
      const lower = drawKey(spec.below, layout);
      keys.set(spec.below.code, lower);
      stack.append(keyElement, lower);
      row.append(stack);
    }

    element.append(row);
    rows.push(row);
  }

  return { element, rows, keys };
}

// Writes the wave's name on the keys that switch waves.
export function showWaveOnKeys(keyboard: DrawnKeyboard, wave: string) {
  for (const keyElement of keyboard.keys.values()) {
    if (keyElement.dataset.kind !== "wave") {
      continue;
    }
    const name = keyElement.querySelector(".key-note");
    if (name !== null) {
      name.textContent = wave;
    }
  }
}
