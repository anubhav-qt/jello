<!-- Practice on the Play tab (docs/design.md#the-tutor-to-mastery): the
     piece the Tutor set, its settings as tiles, its lane with a playhead,
     Start and Stop, and every try's results. Rust plays and scores each try
     (src-tauri/src/practice_run.rs) and says how it goes as events. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { colorForNote } from "./note-colors";
  import {
    HANDS,
    headline,
    LEVELS,
    MODE_WORDS,
    MODES,
    nextOf,
    offsetWords,
    SWINGS,
    type PracticeLane,
    type PracticeTry,
    type PracticeView,
    type Tally,
  } from "./practice";

  interface Props {
    view: PracticeView;
    // A try ended: its number, and whether to coach on it.
    onTry: (number: number, coach: boolean) => void;
  }
  let { view, onTry }: Props = $props();

  // The lane's drawing, in its own units.
  const LANE_WIDTH = 1000;
  const LANE_ROW = 6;
  const LANE_LEAST_HEIGHT = 90;

  // The try playing: when it said it started, by performance.now(), and its
  // clock.
  let started: {
    at: number;
    phase: string;
    countInMs: number;
    msPerBeat: number;
    lengthMs: number;
    hidden: boolean;
  } | null = $state(null);
  let now = $state(0);
  let hit: number[] = $state([]);
  let missed: number[] = $state([]);
  let answered = $state(0);
  // What the running try says it is doing, like "Playing".
  let livePhase = $state("");
  let problem = $state("");
  let frame = 0;
  const unlisteners: UnlistenFn[] = [];

  let running = $derived(view.running === true);
  // The settings tiles fold into one line, so the lane and results fit.
  let settingsOpen = $state(false);
  let settings = $derived(view.settings);
  let lane = $derived(view.lane);
  let tries = $derived(view.tries ?? []);
  let lastTry: PracticeTry | null = $derived.by(() => {
    if (tries.length === 0) {
      return null;
    }
    return tries[tries.length - 1];
  });

  onMount(() => {
    listenTo();
    const step = () => {
      now = performance.now();
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
  });

  onDestroy(() => {
    cancelAnimationFrame(frame);
    for (const unlisten of unlisteners) {
      unlisten();
    }
  });

  async function listenTo() {
    unlisteners.push(
      await listen<{ phase: string; count_in_ms: number; ms_per_beat: number; length_ms: number; hidden: boolean }>(
        "practice-started",
        (event) => {
          const said = event.payload;
          started = {
            at: performance.now(),
            phase: said.phase,
            countInMs: said.count_in_ms,
            msPerBeat: said.ms_per_beat,
            lengthMs: said.length_ms,
            hidden: said.hidden,
          };
          if (said.phase !== "answer") {
            hit = [];
            missed = [];
          }
          answered = 0;
        },
      ),
    );
    unlisteners.push(
      await listen<{ hit?: number[]; missed?: number[]; answered?: number }>("practice-progress", (event) => {
        const said = event.payload;
        if (said.hit !== undefined) {
          hit = said.hit;
        }
        if (said.missed !== undefined) {
          missed = said.missed;
        }
        if (said.answered !== undefined) {
          answered = said.answered;
        }
      }),
    );
    unlisteners.push(
      await listen<{ phase: string }>("practice-phase", (event) => {
        livePhase = event.payload.phase;
      }),
    );
    unlisteners.push(
      await listen<{ number: number; coach: boolean }>("practice-try", (event) => {
        onTry(event.payload.number, event.payload.coach);
      }),
    );
  }

  // Where the playhead is, in beats from the lane's start, or null.
  let playhead: number | null = $derived.by(() => {
    if (!running || started === null) {
      return null;
    }
    if (started.phase === "wait" || started.phase === "answer") {
      return null;
    }
    return (now - started.at - started.countInMs) / started.msPerBeat;
  });

  // The beats left to count in, or 0.
  let countingIn: number = $derived.by(() => {
    if (playhead === null || playhead >= 0) {
      return 0;
    }
    return Math.ceil(-playhead);
  });

  let laneHidden = $derived.by(() => {
    if (settings === undefined) {
      return false;
    }
    if (settings.mode === "improvise") {
      return false;
    }
    if (started === null || !running) {
      return false;
    }
    return started.hidden;
  });

  // What the try is doing, in words.
  let phaseWords = $derived.by(() => {
    if (!running) {
      if (tries.length === 0) {
        return "Ready";
      }
      return `${tries.length} ${tries.length === 1 ? "try" : "tries"}`;
    }
    if (countingIn > 0) {
      return `Count in ${countingIn}`;
    }
    if (started !== null && started.phase === "answer") {
      return `Your turn: ${answered} played`;
    }
    if (livePhase !== "") {
      return livePhase;
    }
    return view.phase ?? "";
  });

  function laneRange(drawn: PracticeLane): { low: number; high: number } {
    let low = 127;
    let high = 0;
    for (const part of drawn.parts) {
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
    return { low: low - 1, high: high + 1 };
  }

  function laneHeight(drawn: PracticeLane): number {
    const range = laneRange(drawn);
    return Math.max(LANE_LEAST_HEIGHT, (range.high - range.low + 1) * LANE_ROW);
  }

  function laneY(drawn: PracticeLane, pitch: number): number {
    const range = laneRange(drawn);
    const rows = (range.high - range.low + 1) * LANE_ROW;
    return (range.high - pitch) * LANE_ROW + (laneHeight(drawn) - rows) / 2;
  }

  function laneX(drawn: PracticeLane, beats: number): number {
    return (beats / drawn.beats) * LANE_WIDTH;
  }

  function beatLines(drawn: PracticeLane): number[] {
    const lines: number[] = [];
    for (let beat = 1; beat < drawn.beats; beat += 1) {
      lines.push(beat);
    }
    return lines;
  }

  function octaveWords(moved: number): string {
    if (moved > 0) {
      return `oct +${moved}`;
    }
    return `oct ${moved}`;
  }

  async function change(changes: Record<string, unknown>) {
    problem = "";
    try {
      await invoke("practice_settings", { settings: changes });
    } catch (error) {
      problem = String(error);
    }
  }

  async function start() {
    problem = "";
    try {
      await invoke("start_practice");
    } catch (error) {
      problem = String(error);
    }
  }

  async function stop() {
    await invoke("stop_practice");
  }

  async function close() {
    await invoke("close_practice");
  }

  function sectionAfter(current: string | null): string | null {
    const names: (string | null)[] = [null];
    for (const section of view.piece?.sections ?? []) {
      names.push(section.name);
    }
    return nextOf(names, current);
  }

  function sectionWords(current: string | null): string {
    if (current === null) {
      return "Whole";
    }
    return current;
  }

  function onOff(on: boolean): string {
    if (on) {
      return "On";
    }
    return "Off";
  }

  function capitalized(word: string): string {
    return word.charAt(0).toUpperCase() + word.slice(1);
  }

  function ladderWords(): string {
    if (settings === undefined || settings.ladder === null) {
      return "Off";
    }
    return `+${settings.ladder.step} to ${settings.ladder.top}`;
  }

  // The settings in one line, for when the tiles are folded.
  function settingsWords(): string {
    if (settings === undefined) {
      return "";
    }
    const words = [MODE_WORDS[settings.mode], `${settings.hands} hands`, `${settings.level}, ${view.window_ms} ms`];
    words.push(`count in ${settings.count_in}`);
    words.push(`click ${onOff(settings.click).toLowerCase()}`);
    if (settings.wait) {
      words.push("wait");
    }
    if (settings.swing !== 50) {
      words.push(`swing ${settings.swing}%`);
    }
    if (settings.section !== null) {
      words.push(`section ${settings.section}`);
    }
    if (settings.loop) {
      words.push("loop");
    }
    if (settings.ladder !== null) {
      words.push(`ladder ${ladderWords()}`);
    }
    if (settings.coach) {
      words.push("coach");
    }
    return words.join(" · ");
  }

  function tallyRows(attempt: PracticeTry): { name: string; tally: Tally }[] {
    const rows: { name: string; tally: Tally }[] = [];
    if (attempt.left !== undefined && attempt.left !== null) {
      rows.push({ name: "Left hand", tally: attempt.left });
    }
    if (attempt.right !== undefined && attempt.right !== null) {
      rows.push({ name: "Right hand", tally: attempt.right });
    }
    for (const section of attempt.sections ?? []) {
      rows.push({ name: `Section ${section.name}`, tally: section.tally });
    }
    return rows;
  }

  function msWords(value: number | null | undefined): string {
    if (value === null || value === undefined) {
      return "-";
    }
    return `${Math.round(value)} ms`;
  }

  function percentWords(value: number | null | undefined): string {
    if (value === null || value === undefined) {
      return "-";
    }
    return `${value}%`;
  }
</script>

{#if view.piece !== undefined && settings !== undefined && lane !== undefined}
  <section class="practice">
    <header class="practice-top">
      <div class="practice-title">
        <h2>{view.piece.name}</h2>
        <span class="practice-quiet">
          {view.piece.bars} bars · {view.tempo_now} bpm{#if view.tempo_note}, {view.tempo_note}{/if}
        </span>
      </div>
      <span class="practice-phase" class:is-live={running}>{phaseWords}</span>
      <div class="practice-buttons">
        <button
          type="button"
          class="practice-button"
          class:is-open={settingsOpen}
          disabled={running}
          onclick={() => (settingsOpen = !settingsOpen)}>Settings</button
        >
        {#if running}
          <button type="button" class="practice-button is-live" onclick={stop}>Stop</button>
        {:else}
          <button type="button" class="practice-button" onclick={start}>Start</button>
        {/if}
        <button type="button" class="practice-button" disabled={running} onclick={close}>Close</button>
      </div>
    </header>

    {#if !settingsOpen || running}
      <p class="practice-summary">{settingsWords()}</p>
    {:else}
      <div class="practice-tiles">
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ mode: nextOf(MODES, settings.mode) })}>
          <span class="practice-tile-name">Mode</span>
          <span class="practice-tile-value">{MODE_WORDS[settings.mode]}</span>
        </button>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ hands: nextOf(HANDS, settings.hands) })}>
          <span class="practice-tile-name">Hands</span>
          <span class="practice-tile-value">{capitalized(settings.hands)}</span>
        </button>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ level: nextOf(LEVELS, settings.level as (typeof LEVELS)[number]) })}>
          <span class="practice-tile-name">Level</span>
          <span class="practice-tile-value">{capitalized(settings.level)} {view.window_ms} ms</span>
        </button>
        <div class="practice-tile">
          <span class="practice-tile-name">Tempo</span>
          <span class="practice-tile-value">
            <button type="button" class="practice-step" disabled={running} onclick={() => change({ tempo: settings.tempo - 5 })}>−</button>
            {settings.tempo}
            <button type="button" class="practice-step" disabled={running} onclick={() => change({ tempo: settings.tempo + 5 })}>+</button>
          </span>
        </div>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ count_in: (settings.count_in + 1) % 3 })}>
          <span class="practice-tile-name">Count in</span>
          <span class="practice-tile-value">{settings.count_in} {settings.count_in === 1 ? "bar" : "bars"}</span>
        </button>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ click: !settings.click })}>
          <span class="practice-tile-name">Click</span>
          <span class="practice-tile-value">{onOff(settings.click)}</span>
        </button>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ wait: !settings.wait })}>
          <span class="practice-tile-name">Wait</span>
          <span class="practice-tile-value">{onOff(settings.wait)}</span>
        </button>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ swing: nextOf(SWINGS, settings.swing as (typeof SWINGS)[number]) })}>
          <span class="practice-tile-name">Swing</span>
          <span class="practice-tile-value">{settings.swing}%</span>
        </button>
        {#if view.piece.sections.length > 0}
          <button type="button" class="practice-tile" disabled={running} onclick={() => change({ section: sectionAfter(settings.section) })}>
            <span class="practice-tile-name">Section</span>
            <span class="practice-tile-value">{sectionWords(settings.section)}</span>
          </button>
        {/if}
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ loop: !settings.loop })}>
          <span class="practice-tile-name">Loop</span>
          <span class="practice-tile-value">{onOff(settings.loop)}</span>
        </button>
        <div class="practice-tile">
          <span class="practice-tile-name">Ladder</span>
          <span class="practice-tile-value">{ladderWords()}</span>
        </div>
        <button type="button" class="practice-tile" disabled={running} onclick={() => change({ coach: !settings.coach })}>
          <span class="practice-tile-name">Coach after every try</span>
          <span class="practice-tile-value">{onOff(settings.coach)}</span>
        </button>
      </div>
    {/if}

    <div class="practice-lane-box">
      <div class="practice-parts">
        {#each lane.parts as part (part.name)}
          <span class:is-jello={!part.player}>{part.name}{#if !part.player}, Jello plays{/if}</span>
        {/each}
      </div>
      {#if laneHidden}
        <p class="practice-hidden">From memory: the lane is hidden.</p>
      {:else}
        {@const height = laneHeight(lane)}
        <svg class="practice-lane" viewBox={`0 0 ${LANE_WIDTH} ${height}`} preserveAspectRatio="none" aria-hidden="true">
          {#each beatLines(lane) as beat (beat)}
            <line class:is-bar={beat % 4 === 0} x1={laneX(lane, beat)} x2={laneX(lane, beat)} y1="0" y2={height} />
          {/each}
          {#each lane.parts as part, partPlace (partPlace)}
            {#each part.notes as note, notePlace (notePlace)}
              {#each note.pitches as pitch (pitch)}
                <rect
                  x={laneX(lane, note.start)}
                  y={laneY(lane, pitch)}
                  width={Math.max(2, laneX(lane, note.hold) - 2)}
                  height={LANE_ROW - 1}
                  rx="1"
                  style:fill={`rgb(${colorForNote(pitch).accent})`}
                  class:is-jello={!part.player}
                  class:is-hit={hit.includes(note.index)}
                  class:is-missed={missed.includes(note.index)}
                />
              {/each}
              {#if note.moved !== null && note.moved !== undefined}
                <text class="practice-octave" x={laneX(lane, note.start)} y="9">{octaveWords(note.moved)}</text>
              {/if}
            {/each}
          {/each}
          {#if playhead !== null && playhead >= 0 && playhead <= lane.beats}
            <line class="practice-playhead" x1={laneX(lane, playhead)} x2={laneX(lane, playhead)} y1="0" y2={height} />
          {/if}
        </svg>
        <div class="practice-beats">
          <span>Bar 1</span>
          <span>{lane.beats / 4} bars</span>
        </div>
      {/if}
    </div>

    {#if problem !== ""}
      <p class="practice-problem">{problem}</p>
    {/if}

    {#if lastTry !== null}
      <div class="practice-results">
        <div class="practice-headline">
          <span class="practice-big">{headline(lastTry)}</span>
          <span class="practice-quiet">
            Try {lastTry.try}, {lastTry.tempo} bpm{#if lastTry.clean}, clean{/if}
          </span>
        </div>
        {#if lastTry.whole !== undefined}
          <dl class="practice-measures">
            <div><dt>Timing</dt><dd>{offsetWords(lastTry.whole.mean_offset_ms)}</dd></div>
            <div><dt>Evenness</dt><dd>{msWords(lastTry.whole.spread_ms)}</dd></div>
            <div><dt>Holds</dt><dd>{percentWords(lastTry.whole.hold_percent)}</dd></div>
            <div><dt>Missed</dt><dd>{lastTry.whole.missed}</dd></div>
            <div><dt>Wrong</dt><dd>{lastTry.wrong ?? 0}</dd></div>
            <div><dt>Extra</dt><dd>{lastTry.extras ?? 0}</dd></div>
            {#if lastTry.together_ms !== null && lastTry.together_ms !== undefined}
              <div><dt>Hands apart</dt><dd>{msWords(lastTry.together_ms)}</dd></div>
            {/if}
          </dl>
          {#each tallyRows(lastTry) as row (row.name)}
            <p class="practice-row">
              <span>{row.name}</span>
              <span>{row.tally.accuracy_percent}%</span>
              <span>{offsetWords(row.tally.mean_offset_ms)}</span>
              <span>spread {msWords(row.tally.spread_ms)}</span>
            </p>
          {/each}
        {:else if lastTry.echo !== undefined}
          <dl class="practice-measures">
            <div><dt>Right</dt><dd>{lastTry.echo.right} of {lastTry.echo.steps}</dd></div>
            <div><dt>First wrong</dt><dd>{lastTry.echo.first_wrong ?? "-"}</dd></div>
            <div><dt>Extra</dt><dd>{lastTry.echo.extra_presses}</dd></div>
          </dl>
        {:else if lastTry.improvised !== undefined}
          <dl class="practice-measures">
            <div><dt>In key</dt><dd>{percentWords(lastTry.improvised.in_key_percent)}</dd></div>
            <div><dt>Chord tones on 1 and 3</dt><dd>{percentWords(lastTry.improvised.chord_tones_on_strong_beats_percent)}</dd></div>
            <div><dt>Rhythms</dt><dd>{lastTry.improvised.rhythm_kinds}</dd></div>
            <div><dt>Motif repeats</dt><dd>{lastTry.improvised.motif_repeats}</dd></div>
            <div><dt>Notes</dt><dd>{lastTry.improvised.notes}</dd></div>
          </dl>
        {:else if lastTry.waited !== undefined}
          <dl class="practice-measures">
            <div><dt>Steps</dt><dd>{lastTry.waited.steps}</dd></div>
            <div><dt>Wrong</dt><dd>{lastTry.waited.wrong}</dd></div>
            <div><dt>Time</dt><dd>{lastTry.waited.seconds} s</dd></div>
          </dl>
        {/if}
      </div>
    {/if}

    {#if tries.length > 1}
      <ol class="practice-history">
        {#each tries as attempt (attempt.try)}
          <li class:is-clean={attempt.clean}>
            <span>{attempt.try}</span>
            <span>{headline(attempt)}</span>
            <span>{attempt.tempo} bpm</span>
          </li>
        {/each}
      </ol>
    {/if}
  </section>
{/if}
