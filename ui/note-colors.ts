// The Stage lights: one color per note letter, shared across octaves.
// CSS and the canvas use the same RGB triples so keys and beams match.
export interface NoteColor {
  accent: string;
  glow: string;
  core: string;
}

// The background grid and canvas beams use the same spacing in page pixels.
export const DOT_CELL = 12;

export const NOTE_COLORS: Record<string, NoteColor> = {
  C: { accent: "191, 165, 255", glow: "219, 203, 255", core: "245, 239, 255" },
  D: { accent: "255, 188, 154", glow: "255, 214, 192", core: "255, 241, 232" },
  E: { accent: "139, 230, 194", glow: "183, 246, 220", core: "231, 255, 246" },
  F: { accent: "245, 221, 140", glow: "251, 235, 185", core: "255, 249, 230" },
  G: { accent: "255, 145, 196", glow: "255, 191, 221", core: "255, 235, 245" },
  A: { accent: "169, 216, 245", glow: "208, 235, 251", core: "239, 249, 255" },
  B: { accent: "234, 228, 219", glow: "245, 240, 233", core: "255, 252, 247" },
};

// Sharps share their note letter's color. Chord keys use their root note;
// the readout gives each note its own letter's color.
const NOTE_FAMILIES = ["C", "C", "D", "D", "E", "F", "F", "G", "G", "A", "A", "B"];

export function colorForNote(note: number): NoteColor {
  const family = NOTE_FAMILIES[note % 12];
  return NOTE_COLORS[family];
}
