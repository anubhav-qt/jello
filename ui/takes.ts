// Takes (docs/design.md#ai-and-learning-rethought-2026-11-03): whatever one
// answer changes is one take on the song, which the player flips to and
// from with right Command, keeps with Return or drops with Esc. Rust keeps
// both versions (src-tauri/src/takes.rs).

export type TakeState = "running" | "open" | "kept" | "dropped";

// "Tape", "Sound" or "Mix".
export type TakePart = string;

export interface TakeView {
  number: number;
  parts: TakePart[];
  state: TakeState;
  // Whether the take is on screen and heard, or the player's own version.
  showingTake: boolean;
}

// What the rail's chip says for a take: "Take 3 · Tape, Sound".
export function takeTitle(take: TakeView): string {
  if (take.parts.length === 0) {
    return `Take ${take.number}`;
  }
  return `Take ${take.number} · ${take.parts.join(", ")}`;
}

// The take's state, in a word, for a take no longer open.
export function takeStateWord(take: TakeView): string {
  if (take.state === "kept") {
    return "Kept";
  }
  if (take.state === "dropped") {
    return "Dropped";
  }
  return "";
}

// Whether the tape draws a draft: an open take that changed the tape and is
// on screen.
export function draftOnTape(take: TakeView | null): boolean {
  if (take === null) {
    return false;
  }
  if (take.state !== "open" || !take.showingTake) {
    return false;
  }
  return take.parts.includes("Tape");
}
