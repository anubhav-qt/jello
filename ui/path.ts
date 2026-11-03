// The learning path (docs/design.md#ai-and-learning-rethought-2026-11-03),
// as Rust's path_view and set_learn answer it (src-tauri/src/path.rs).

export interface StageView {
  id: string;
  name: string;
  // "done", "current", "open" or "later" (waiting for vocals).
  state: string;
  // What the song has for it, like "4 chords", shown on hover, or "".
  note: string;
}

export interface TopicView {
  name: string;
  stage: string;
  // "done", "current" or "open".
  state: string;
}

export interface AreaView {
  name: string;
  topics: TopicView[];
}

export interface PathView {
  learn: boolean;
  song: string;
  stages: StageView[];
  current: string | null;
  areas: AreaView[];
}

// How many topics are covered, and how many there are, across every area.
export function coveredCount(path: PathView): { covered: number; all: number } {
  let covered = 0;
  let all = 0;
  for (const area of path.areas) {
    for (const topic of area.topics) {
      all += 1;
      if (topic.state === "done") {
        covered += 1;
      }
    }
  }
  return { covered, all };
}
