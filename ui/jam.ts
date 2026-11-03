// The jam card (docs/design.md#ai-and-learning-rethought-2026-11-03):
// after a pause, the moments of the jam worth keeping (src-tauri/src/jam.rs).

// How long with no press before the card shows, in ms.
export const JAM_PAUSE_MS = 10000;

export interface JamPress {
  key: string;
  notes: number[];
  startMs: number;
  holdMs: number;
}

export interface Moment {
  // Like "Played 3 times" or "Held ending".
  name: string;
  // Its notes in words, like "C4 D4 E4".
  notes: string;
  presses: JamPress[];
  score: number;
}

export interface JamView {
  moments: Moment[];
  // The press the next jam starts from.
  next: number;
}

// How long a moment lasts, in seconds, like "2.4 s".
export function momentSeconds(moment: Moment): string {
  let end = 0;
  for (const press of moment.presses) {
    end = Math.max(end, press.startMs + press.holdMs);
  }
  return `${(end / 1000).toFixed(1)} s`;
}
