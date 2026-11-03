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

// How long a moment lasts, in ms.
export function momentMs(moment: Moment): number {
  let end = 0;
  for (const press of moment.presses) {
    end = Math.max(end, press.startMs + press.holdMs);
  }
  return end;
}

// How long a moment lasts, in seconds, like "2.4 s".
export function momentSeconds(moment: Moment): string {
  const seconds = momentMs(moment) / 1000;
  return `${seconds.toFixed(1)} s`;
}

// How many moments the card shows by Jello's own measure.
const SHOWN_MOMENTS = 3;

// A pick from the AI: a candidate by place, and its name.
export interface MomentPick {
  place: number;
  name: string;
}

// A moment on the card: its place among the candidates, the name shown,
// and whether the AI picked and named it.
export interface ShownMoment {
  place: number;
  name: string;
  picked: boolean;
}

// Jello's own best moments, by measure, as the card shows them first.
export function measuredMoments(view: JamView): ShownMoment[] {
  const shown: ShownMoment[] = [];
  const count = Math.min(view.moments.length, SHOWN_MOMENTS);
  for (let place = 0; place < count; place += 1) {
    shown.push({ place, name: view.moments[place].name, picked: false });
  }
  return shown;
}

// The AI's `picks` as the card shows them; picks of no candidate, or of
// one already picked, are left out. With none left, Jello's own.
export function pickedMoments(view: JamView, picks: MomentPick[]): ShownMoment[] {
  const shown: ShownMoment[] = [];
  const taken = new Set<number>();
  for (const pick of picks) {
    const known = pick.place >= 0 && pick.place < view.moments.length;
    if (!known || taken.has(pick.place)) {
      continue;
    }
    taken.add(pick.place);
    shown.push({ place: pick.place, name: pick.name, picked: true });
  }
  if (shown.length === 0) {
    return measuredMoments(view);
  }
  return shown;
}
