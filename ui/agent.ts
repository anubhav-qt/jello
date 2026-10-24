// What the agent's panel shows (docs/design.md#the-agent-mode). The agent
// host tells the panel everything pi does as it happens, as "agent-host"
// events (src-tauri/src/agent.rs); this file turns them into turns: what
// the player said, then each step the agent took, live.

// The agent's job in each tab.
export type Job = "tutor" | "composer" | "producer";

export interface ModelView {
  provider: string;
  id: string;
  name: string;
}

// A provider the player can sign in to: in the browser (with the button's
// words), with an API key, or both.
export interface ProviderView {
  id: string;
  name: string;
  browser: string | null;
  key: boolean;
}

// One event from the agent host.
export interface HostEvent {
  type: string;
  [field: string]: unknown;
}

// The agent thinking, from when it started to when it ended, if it has.
export interface ThinkingStep {
  kind: "thinking";
  text: string;
  startedAt: number;
  endedAt: number | null;
}

// One of Jello's tools, called with `args`. Once done: whether it failed,
// the lines saying what changed, the card Jello drew, its answer and how
// long it took.
export interface ToolStep {
  kind: "tool";
  id: string;
  tool: string;
  args: Record<string, unknown>;
  done: boolean;
  failed: boolean;
  changes: string[];
  card: Card | null;
  answer: string;
  ms: number | null;
}

// The cards Jello draws in a reply (src-tauri/src/cards.rs). Rust checks
// every field, so the panel can trust them.
export interface KeysCard {
  kind: "keys";
  title: string;
  keys: { key: string; what: string; label: string }[];
}

export interface LaneNote {
  label: string;
  pitches: number[];
  start: number;
  hold: number;
}

export interface LaneCard {
  kind: "lane";
  title: string;
  beats: number;
  tempo: number;
  parts: { name: string; notes: LaneNote[] }[];
}

export type Fit = "in_key" | "can_work" | "outside";

export interface ProgressionCard {
  kind: "progression";
  title: string;
  key: string;
  chords: { name: string; numeral: string; fit: Fit }[];
}

export interface CompareCard {
  kind: "compare";
  title: string;
  items: { name: string; measures: { name: string; value: string }[] }[];
}

export interface ChartCard {
  kind: "chart";
  title: string;
  unit: string;
  points: { label: string; value: number }[];
}

// What a sound's drawing needs.
export interface SoundSketch {
  loudness: { attack_ms: number; decay_ms: number; sustain: number; release_ms: number };
  filter: { type: string; cutoff_hz: number; resonance: number };
  oscillators: { kind: string; level: number }[];
}

export interface SoundCard {
  kind: "sound";
  title: string;
  after: SoundSketch;
  before: SoundSketch | null;
  lines?: string[];
}

export interface PressesCard {
  kind: "presses";
  title: string;
  presses: { label: string; pitch: number | null; at_ms: number; held_ms: number | null }[];
}

export interface Measures {
  loudness_db: number | null;
  peak_db: number | null;
  soft_clip_reached: boolean;
  brightness_hz: number | null;
  lows_percent: number | null;
  mids_percent: number | null;
  highs_percent: number | null;
  width_percent: number | null;
  balance_db: number | null;
}

export interface ListenCard {
  kind: "listen";
  title: string;
  rendered: string;
  seconds: number;
  whole: Measures;
  loudness: (number | null)[];
  silences: { from_beat: number; to_beat: number }[];
}

// The piece jello_practice set, as the Play tab shows it.
export interface PracticeCard {
  kind: "practice";
  title: string;
  view: {
    piece: { name: string; bars: number; parts: { name: string; hand: string; player: boolean }[] };
    settings: { mode: string; hands: string; level: string; tempo: number; wait: boolean; loop: boolean };
    window_ms: number;
  };
}

// One choice an options card offers, as Rust checked it: chords with their
// numerals, a melody's presses, or a sound's changes and drawings.
export interface OptionView {
  name: string;
  chords?: { name: string; hold: number; numeral?: string }[];
  presses?: { label: string; start: number; hold: number }[];
  lines?: string[];
  after?: SoundSketch;
  before?: SoundSketch;
}

export interface OptionsCard {
  kind: "options";
  title: string;
  offer: number;
  choice: "chords" | "melody" | "sound";
  options: OptionView[];
}

export type Card =
  | KeysCard
  | LaneCard
  | ProgressionCard
  | CompareCard
  | ChartCard
  | SoundCard
  | PressesCard
  | ListenCard
  | PracticeCard
  | OptionsCard;

// Text from the agent, as it arrives.
export interface TextStep {
  kind: "text";
  text: string;
}

export type Step = ThinkingStep | ToolStep | TextStep;

// A question a tool asks the player, by number, like whether to save over
// a preset.
export interface Question {
  number: number;
  question: string;
  answer: boolean | null;
}

// What the player said, and the agent's run that answered it.
export interface Turn {
  said: string;
  job: Job;
  // Whether the job changed for this turn, so the panel says "Now Composer".
  jobChanged: boolean;
  // The number Undo takes this turn back with, or null if it can't be.
  reply: number | null;
  steps: Step[];
  questions: Question[];
  running: boolean;
  stopped: boolean;
  undone: boolean;
  errors: string[];
  // Tokens sent and how many of them the cache already had.
  input: number;
  cacheRead: number;
  // Drawn again from a saved chat, so its timings aren't known.
  replayed: boolean;
}

// A saved chat, as the Chats page lists it.
export interface ChatView {
  path: string;
  title: string;
  song: string;
  jobs: Job[];
  // When it last changed, in ms since 1970.
  modified: number;
  // Its text, for searching.
  text: string;
}

// A turn of a saved chat, as the host replays it: what the player said and
// the events of its run.
export interface SavedTurn {
  said: string;
  job: Job | null;
  jobChanged: boolean;
  events: HostEvent[];
}

export function newTurn(said: string, job: Job, jobChanged: boolean, reply: number | null): Turn {
  return {
    said,
    job,
    jobChanged,
    reply,
    steps: [],
    questions: [],
    running: true,
    stopped: false,
    undone: false,
    errors: [],
    input: 0,
    cacheRead: 0,
    replayed: false,
  };
}

// The turn `saved` drew when it ran.
export function replayedTurn(saved: SavedTurn): Turn {
  let job: Job = "tutor";
  if (saved.job !== null) {
    job = saved.job;
  }
  const turn = newTurn(saved.said, job, saved.jobChanged, null);
  turn.replayed = true;
  let stopped = false;
  for (const event of saved.events) {
    if (event.type === "stopped") {
      stopped = true;
    } else {
      addToTurn(turn, event, 0);
    }
  }
  addToTurn(turn, { type: "run_end", stopped }, 0);
  return turn;
}

// The letter each job is marked with on the Chats page.
export function jobLetter(job: Job): string {
  if (job === "composer") {
    return "C";
  }
  if (job === "producer") {
    return "P";
  }
  return "T";
}

// The chats whose title, song or text has `search` in it, in any case.
export function chatsMatching(chats: ChatView[], search: string): ChatView[] {
  const wanted = search.trim().toLowerCase();
  if (wanted === "") {
    return chats;
  }
  const matching: ChatView[] = [];
  for (const chat of chats) {
    const words = `${chat.title}\n${chat.song}\n${chat.text}`.toLowerCase();
    if (words.includes(wanted)) {
      matching.push(chat);
    }
  }
  return matching;
}

// The day `time` was on, as the Chats page heads it: Today, Yesterday, or
// the date, as seen at `now`.
export function chatDay(time: number, now: number): string {
  const day = new Date(time);
  day.setHours(0, 0, 0, 0);
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  const daysAgo = Math.round((today.getTime() - day.getTime()) / 86400000);
  if (daysAgo === 0) {
    return "Today";
  }
  if (daysAgo === 1) {
    return "Yesterday";
  }
  return day.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
}

// `chats`, newest first already, in groups by day.
export function chatsByDay(chats: ChatView[], now: number): { day: string; chats: ChatView[] }[] {
  const groups: { day: string; chats: ChatView[] }[] = [];
  for (const chat of chats) {
    const day = chatDay(chat.modified, now);
    const last = groups[groups.length - 1];
    if (last !== undefined && last.day === day) {
      last.chats.push(chat);
    } else {
      groups.push({ day, chats: [chat] });
    }
  }
  return groups;
}

function lastStep(turn: Turn): Step | undefined {
  return turn.steps[turn.steps.length - 1];
}

function textOf(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  return "";
}

function numberOf(value: unknown): number {
  if (typeof value === "number") {
    return value;
  }
  return 0;
}

// Adds what `event` says to `turn`, at `now`, by performance.now(). Events
// about the panel rather than a run (status, sign-in) are left alone.
export function addToTurn(turn: Turn, event: HostEvent, now: number) {
  if (event.type === "thinking_start") {
    turn.steps.push({ kind: "thinking", text: "", startedAt: now, endedAt: null });
  } else if (event.type === "thinking") {
    const step = lastStep(turn);
    if (step !== undefined && step.kind === "thinking") {
      step.text += textOf(event.text);
    }
  } else if (event.type === "thinking_end") {
    const step = lastStep(turn);
    if (step !== undefined && step.kind === "thinking") {
      step.endedAt = now;
    }
  } else if (event.type === "text") {
    addText(turn, textOf(event.text));
  } else if (event.type === "tool_start") {
    turn.steps.push({
      kind: "tool",
      id: textOf(event.id),
      tool: textOf(event.tool),
      args: argsOf(event.args),
      done: false,
      failed: false,
      changes: [],
      card: null,
      answer: "",
      ms: null,
    });
  } else if (event.type === "tool_end") {
    endTool(turn, event);
  } else if (event.type === "usage") {
    turn.input += numberOf(event.input) + numberOf(event.cacheRead) + numberOf(event.cacheWrite);
    turn.cacheRead += numberOf(event.cacheRead);
  } else if (event.type === "error") {
    turn.errors.push(textOf(event.message));
  } else if (event.type === "retry") {
    turn.errors.push(`Trying again (${numberOf(event.attempt)} of ${numberOf(event.attempts)}): ${textOf(event.why)}`);
  } else if (event.type === "run_end") {
    turn.running = false;
    turn.stopped = event.stopped === true;
    endThinking(turn, now);
  } else if (event.type === "host_ended") {
    turn.running = false;
    turn.errors.push(hostEndedWhy(event));
  }
}

function addText(turn: Turn, text: string) {
  const step = lastStep(turn);
  if (step !== undefined && step.kind === "text") {
    step.text += text;
    return;
  }
  turn.steps.push({ kind: "text", text });
}

function argsOf(value: unknown): Record<string, unknown> {
  if (value !== null && typeof value === "object") {
    return value as Record<string, unknown>;
  }
  return {};
}

function endTool(turn: Turn, event: HostEvent) {
  for (const step of turn.steps) {
    if (step.kind !== "tool" || step.id !== event.id) {
      continue;
    }
    step.done = true;
    step.failed = event.failed === true;
    step.answer = textOf(event.answer);
    if (Array.isArray(event.changes)) {
      step.changes = event.changes.map((line) => String(line));
    }
    if (event.card !== null && typeof event.card === "object") {
      step.card = event.card as Card;
    }
    if (typeof event.ms === "number") {
      step.ms = event.ms;
    }
  }
}

// A run stopped part way can leave thinking without an end.
function endThinking(turn: Turn, now: number) {
  for (const step of turn.steps) {
    if (step.kind === "thinking" && step.endedAt === null) {
      step.endedAt = now;
    }
  }
}

export function hostEndedWhy(event: HostEvent): string {
  const why = textOf(event.why).trim();
  if (why === "") {
    return "The agent stopped running. Send again to restart it.";
  }
  return `The agent stopped running: ${why}`;
}

// Whether the turn changed anything, so it gets an Undo button.
export function changedSomething(turn: Turn): boolean {
  for (const step of turn.steps) {
    if (step.kind === "tool" && step.changes.length > 0) {
      return true;
    }
  }
  return false;
}

// What a tool step's row says, short, like "Changed the sound".
export function toolTitle(step: ToolStep): string {
  if (step.tool === "jello_look") {
    return "Looked at";
  }
  if (step.tool === "jello_sound") {
    return "Changed the sound";
  }
  if (step.tool === "jello_play") {
    if (step.done) {
      return "Played it for you";
    }
    return "Playing it for you";
  }
  if (step.tool === "jello_looper") {
    return "Looper";
  }
  if (step.tool === "jello_song") {
    return "Song";
  }
  if (step.tool === "jello_presets") {
    return "Presets";
  }
  if (step.tool === "jello_listen") {
    if (step.done) {
      return "Listened";
    }
    return "Listening";
  }
  if (step.tool === "jello_show") {
    return "Showed";
  }
  if (step.tool === "jello_options") {
    return "Options";
  }
  if (step.tool === "jello_notes") {
    return "Wrote notes";
  }
  if (step.tool === "jello_theory") {
    return "Worked out";
  }
  if (step.tool === "jello_practice") {
    return "Practice";
  }
  return step.tool.replace("jello_", "");
}

// What the row says after its title, from the call's arguments: the parts
// looked at, how many presses played, the looper's or song's action.
export function toolDetail(step: ToolStep): string {
  const args = step.args;
  if (step.tool === "jello_look") {
    if (Array.isArray(args.parts) && args.parts.length > 0) {
      return args.parts.join(", ");
    }
    return "everything";
  }
  if (step.tool === "jello_play" || step.tool === "jello_listen") {
    if (Array.isArray(args.presses)) {
      return `${args.presses.length} presses`;
    }
    return "";
  }
  if (step.tool === "jello_show") {
    const card = args.card as Record<string, unknown> | undefined;
    if (card !== undefined && typeof card.kind === "string") {
      return card.kind;
    }
    return "";
  }
  if (step.tool === "jello_options" && typeof args.kind === "string") {
    return args.kind;
  }
  if (step.tool === "jello_theory" && typeof args.question === "string") {
    return args.question.split("_").join(" ");
  }
  if (step.tool === "jello_notes" && Array.isArray(args.notes)) {
    return `${args.notes.length} notes`;
  }
  if (typeof args.action === "string") {
    return args.action.split("_").join(" ");
  }
  return "";
}

// The space between one press and the next, as a presses strip marks it:
// "+120 ms gap", "40 ms overlap", or "" when one starts as the other ends.
// A press still held has no end yet.
export function gapLabel(
  press: { at_ms: number; held_ms: number | null },
  next: { at_ms: number },
): string {
  if (press.held_ms === null) {
    return "";
  }
  const gap = next.at_ms - (press.at_ms + press.held_ms);
  if (gap > 0) {
    return `+${gap} ms gap`;
  }
  if (gap < 0) {
    return `${-gap} ms overlap`;
  }
  return "";
}

// The lowest and highest pitch a lane's notes reach.
export function laneRange(card: LaneCard): { low: number; high: number } {
  let low = 127;
  let high = 0;
  for (const part of card.parts) {
    for (const note of part.notes) {
      for (const pitch of note.pitches) {
        low = Math.min(low, pitch);
        high = Math.max(high, pitch);
      }
    }
  }
  if (low > high) {
    return { low: 60, high: 72 };
  }
  return { low, high };
}

// A loudness envelope's outline, as an SVG path `width` by `height`, with
// `longestMs` across: up over the attack, down over the decay to the
// sustain, held a while, then down over the release.
export function envelopePath(
  loudness: SoundSketch["loudness"],
  width: number,
  height: number,
  longestMs: number,
): string {
  const scale = width / longestMs;
  const attack = loudness.attack_ms * scale;
  const decay = attack + loudness.decay_ms * scale;
  const held = decay + envelopeHoldMs(longestMs) * scale;
  const release = held + loudness.release_ms * scale;
  const sustain = height - (loudness.sustain / 100) * height;
  return `M0,${height} L${attack},0 L${decay},${sustain} L${held},${sustain} L${release},${height}`;
}

// How long the drawing holds the sustain: a fixed share of the drawing.
function envelopeHoldMs(longestMs: number): number {
  return longestMs * 0.2;
}

// The ms an envelope drawing needs across to fit every envelope given.
export function envelopeSpanMs(sketches: SoundSketch[]): number {
  let longest = 1;
  for (const sketch of sketches) {
    const loudness = sketch.loudness;
    const total = loudness.attack_ms + loudness.decay_ms + loudness.release_ms;
    longest = Math.max(longest, total);
  }
  // The held part takes a fifth of the drawing.
  return longest / 0.8;
}

// The filter's gain at `hz`, in dB, as a second order filter of its type,
// with the resonance as its peak.
export function filterGainDb(filter: SoundSketch["filter"], hz: number): number {
  const q = 0.707 + (filter.resonance / 100) * 8;
  const r = hz / filter.cutoff_hz;
  const under = Math.sqrt((1 - r * r) ** 2 + (r / q) ** 2);
  let gain = 1 / under;
  if (filter.type === "high-pass") {
    gain = (r * r) / under;
  } else if (filter.type === "band-pass") {
    gain = r / q / under;
  } else if (filter.type === "notch") {
    gain = Math.abs(1 - r * r) / under;
  }
  return 20 * Math.log10(Math.max(gain, 0.00001));
}

// The filter's curve, as an SVG path `width` by `height`, from 20 Hz to
// 20 kHz across and from +12 dB to -36 dB down.
export function filterPath(filter: SoundSketch["filter"], width: number, height: number): string {
  const steps = 60;
  let path = "";
  for (let step = 0; step <= steps; step += 1) {
    const hz = 20 * Math.pow(1000, step / steps);
    const db = Math.min(12, Math.max(-36, filterGainDb(filter, hz)));
    const x = (step / steps) * width;
    const y = ((12 - db) / 48) * height;
    if (step === 0) {
      path += `M${x.toFixed(1)},${y.toFixed(1)}`;
    } else {
      path += ` L${x.toFixed(1)},${y.toFixed(1)}`;
    }
  }
  return path;
}

// Where `value` falls from `low` to `high`, from 0 to 1.
export function shareOf(value: number | null, low: number, high: number): number {
  if (value === null) {
    return 0;
  }
  const share = (value - low) / (high - low);
  return Math.min(1, Math.max(0, share));
}

// How many seconds `step` thought for, or has been thinking at `now`.
export function thoughtSeconds(step: ThinkingStep, now: number): number {
  let end = now;
  if (step.endedAt !== null) {
    end = step.endedAt;
  }
  return Math.max(0, Math.round((end - step.startedAt) / 1000));
}

// The share of the turn's tokens the cache already had, in percent, or
// null before the model has answered.
export function cachedPercent(turn: Turn): number | null {
  if (turn.input === 0) {
    return null;
  }
  return Math.round((turn.cacheRead / turn.input) * 100);
}

export function jobName(job: Job): string {
  if (job === "composer") {
    return "Composer";
  }
  if (job === "producer") {
    return "Producer";
  }
  return "Tutor";
}
