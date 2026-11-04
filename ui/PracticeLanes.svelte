<!-- Practicing a section of your own song, in the tape's place
     (docs/design.md#ai-and-learning-rethought-2026-11-03, board A): a head
     row (section, track, the tempo ladder, tries, the loop), a bar ruler,
     then a lane per hand moving past a white now line. Each note is a block
     with its key letter and what it plays; once played it says how many ms
     off it was, or "missed". The hard bar is framed lavender as the loop,
     and the bars after it stay dim until it is clean. Rust plays and scores
     each try (src-tauri/src/practice_run.rs, practice_song.rs). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { colorForNote } from "./note-colors";
  import {
    keyLetter,
    ladderSteps,
    offsetWords,
    type NoteResult,
    type PracticeNote,
    type PracticePart,
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

  // How many beats the lanes show at once, and how far in the now line is.
  const SHOWN_BEATS = 8;
  const BEATS_BEFORE_NOW = 2;
  const BEATS_PER_BAR = 4;

  let started: { at: number; countInMs: number; msPerBeat: number } | null = $state(null);
  let now = $state(0);
  let hit: number[] = $state([]);
  let missed: number[] = $state([]);
  let problem = $state("");
  // A click on Start or Close is on its way, so a quick second one waits.
  let busy = false;
  let frame = 0;
  const unlisteners: UnlistenFn[] = [];

  let running = $derived(view.running === true);
  let song = $derived(view.piece?.song ?? null);
  let lane = $derived(view.whole_lane ?? view.lane ?? null);
  let tries = $derived(view.tries ?? []);
  let lastTry: PracticeTry | null = $derived.by(() => {
    if (tries.length === 0) {
      return null;
    }
    return tries[tries.length - 1];
  });
  let loopBar: number | null = $derived.by(() => {
    const section = view.settings?.section ?? null;
    if (section === null) {
      return null;
    }
    const bar = Number(section.replace("Bar ", ""));
    if (!Number.isFinite(bar)) {
      return null;
    }
    return bar;
  });
  // Where tries start, in beats from the section's start.
  let firstBeat = $derived.by(() => {
    if (loopBar === null) {
      return 0;
    }
    return (loopBar - 1) * BEATS_PER_BAR;
  });
  let bars = $derived(view.piece?.bars ?? 0);
  let steps = $derived(ladderSteps(view.settings?.tempo ?? 0, view.settings?.ladder ?? null));
  let cleanTries = $derived.by(() => {
    let count = 0;
    for (const attempt of tries) {
      if (attempt.clean) {
        count += 1;
      }
    }
    return count;
  });
  // The last try's result of each note, by the note's place.
  let results: Map<number, NoteResult> = $derived.by(() => {
    const found = new Map<number, NoteResult>();
    for (const result of lastTry?.notes ?? []) {
      found.set(result.index, result);
    }
    return found;
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
      await listen<{ phase: string; count_in_ms: number; ms_per_beat: number }>("practice-started", (event) => {
        started = {
          at: performance.now(),
          countInMs: event.payload.count_in_ms,
          msPerBeat: event.payload.ms_per_beat,
        };
        hit = [];
        missed = [];
      }),
    );
    unlisteners.push(
      await listen<{ hit?: number[]; missed?: number[] }>("practice-progress", (event) => {
        if (event.payload.hit !== undefined) {
          hit = event.payload.hit;
        }
        if (event.payload.missed !== undefined) {
          missed = event.payload.missed;
        }
      }),
    );
    unlisteners.push(
      await listen<{ number: number; coach: boolean }>("practice-try", (event) => {
        started = null;
        onTry(event.payload.number, event.payload.coach);
      }),
    );
  }

  // The beat at the now line, from the section's start.
  let nowBeat = $derived.by(() => {
    if (!running || started === null) {
      return firstBeat;
    }
    const played = now - started.at - started.countInMs;
    return firstBeat + played / started.msPerBeat;
  });

  // Where `beat` is across the lanes, in percent.
  function placeOf(beat: number): number {
    const shownFrom = nowBeat - BEATS_BEFORE_NOW;
    return ((beat - shownFrom) / SHOWN_BEATS) * 100;
  }

  function widthOf(beats: number): number {
    return (beats / SHOWN_BEATS) * 100;
  }

  function isShown(note: PracticeNote): boolean {
    const shownFrom = nowBeat - BEATS_BEFORE_NOW;
    const shownTo = shownFrom + SHOWN_BEATS;
    return note.start + note.hold > shownFrom && note.start < shownTo;
  }

  function barOf(beat: number): number {
    return Math.floor(beat / BEATS_PER_BAR) + 1;
  }

  // Bars after the looped one stay dim until it is clean.
  function isDim(bar: number): boolean {
    return loopBar !== null && bar > loopBar;
  }

  function handParts(): PracticePart[] {
    const found: PracticePart[] = [];
    for (const part of lane?.parts ?? []) {
      if (part.hand === "left" || part.hand === "right") {
        found.push(part);
      }
    }
    return found;
  }

  function keyOf(note: PracticeNote): string {
    const key = song?.keys[note.index];
    if (key === undefined) {
      return "";
    }
    return keyLetter(key);
  }

  // What a played note says: how far off it was, or "missed".
  function playedWords(note: PracticeNote): string {
    if (missed.includes(note.index)) {
      return "missed";
    }
    if (running && hit.includes(note.index)) {
      return "hit";
    }
    if (running) {
      return "";
    }
    const result = results.get(note.index);
    if (result === undefined) {
      return "";
    }
    if (!result.hit) {
      return "missed";
    }
    if (result.offset_ms === null) {
      return "";
    }
    const offset = Math.round(result.offset_ms);
    if (offset > 0) {
      return `${offset} ms late`;
    }
    if (offset < 0) {
      return `${-offset} ms early`;
    }
    return "on time";
  }

  function isMissed(note: PracticeNote): boolean {
    return playedWords(note) === "missed";
  }

  function handName(part: PracticePart): string {
    if (part.hand === "left") {
      return "Left";
    }
    return "Right";
  }

  // A hand's last try, like "12/14 · 8 ms late".
  function handWords(part: PracticePart): string {
    let tally: Tally | null | undefined = null;
    if (part.hand === "left") {
      tally = lastTry?.left;
    } else {
      tally = lastTry?.right;
    }
    if (tally === null || tally === undefined) {
      if (part.notes.length === 1) {
        return "1 note";
      }
      return `${part.notes.length} notes`;
    }
    return `${tally.hits}/${tally.notes} · ${offsetWords(tally.mean_offset_ms)}`;
  }

  function loopWords(): string {
    if (loopBar === null) {
      return "Whole section";
    }
    return `Bar ${loopBar}`;
  }

  async function start() {
    if (busy) {
      return;
    }
    busy = true;
    problem = "";
    try {
      await invoke("start_practice");
    } catch (error) {
      problem = String(error);
    }
    busy = false;
  }

  async function stop() {
    await invoke("stop_practice");
  }

  async function close() {
    if (busy) {
      return;
    }
    busy = true;
    await invoke("close_practice");
    busy = false;
  }

  // Back to the whole section, out of the bar's loop.
  async function playWhole() {
    problem = "";
    try {
      await invoke("practice_settings", { settings: { section: null } });
    } catch (error) {
      problem = String(error);
    }
  }
</script>

<section class="lanes">
  <header class="lanes-head">
    <div class="lanes-title">
      <h2>{song?.section ?? view.piece?.name}</h2>
      <span class="practice-quiet">{song?.track}</span>
    </div>
    <div class="lanes-ladder" aria-label="Tempo ladder">
      {#each steps as tempo, place (place)}
        <span class:is-now={tempo === view.tempo_now}>{tempo}</span>
      {/each}
      <span class="practice-quiet">bpm</span>
    </div>
    <span class="lanes-tries">
      Tries {tries.length} · clean {cleanTries}
    </span>
    <span class="lanes-loop" class:is-on={loopBar !== null}>Loop {loopWords()}</span>
    <div class="practice-buttons">
      {#if loopBar !== null}
        <button type="button" class="practice-button" disabled={running} onclick={playWhole}>Whole</button>
      {/if}
      {#if running}
        <button type="button" class="practice-button is-live" onclick={stop}>Stop</button>
      {:else}
        <button type="button" class="practice-button" onclick={start}>Start</button>
      {/if}
      <button type="button" class="practice-button" disabled={running} onclick={close}>Close</button>
    </div>
  </header>

  <div class="lanes-ruler" aria-hidden="true">
    {#each { length: bars } as _, place (place)}
      {@const bar = place + 1}
      {@const from = (bar - 1) * BEATS_PER_BAR}
      {#if placeOf(from + BEATS_PER_BAR) > 0 && placeOf(from) < 100}
        <span
          class="lanes-bar"
          class:is-loop={bar === loopBar}
          class:is-dim={isDim(bar)}
          style:left="{placeOf(from)}%"
          style:width="{widthOf(BEATS_PER_BAR)}%">Bar {bar}</span
        >
      {/if}
    {/each}
  </div>

  {#each handParts() as part (part.hand)}
    <div class="lanes-lane">
      <div class="lanes-label">
        <span class="lanes-hand">{handName(part)}</span>
        <span class="practice-quiet">{handWords(part)}</span>
      </div>
      <div class="lanes-track">
        {#each part.notes as note (note.index)}
          {#if isShown(note)}
            {@const color = colorForNote(note.pitches[0])}
            <div
              class="lanes-note"
              class:is-missed={isMissed(note)}
              class:is-dim={isDim(barOf(note.start))}
              style:left="{placeOf(note.start)}%"
              style:width="{widthOf(note.hold)}%"
              style:--note={color.accent}
            >
              <span class="lanes-key">{keyOf(note)}</span>
              <span class="lanes-plays">{note.label}</span>
              <span class="lanes-played">{playedWords(note)}</span>
            </div>
          {/if}
        {/each}
        <span class="lanes-now" style:left="{(BEATS_BEFORE_NOW / SHOWN_BEATS) * 100}%"></span>
      </div>
    </div>
  {/each}

  {#if problem !== ""}
    <p class="practice-problem">{problem}</p>
  {/if}
</section>
