<!-- Taste drills in the rail (docs/design.md#ai-and-learning-rethought-2026-11-03),
     while Learn is on: pick Chord, Interval or Sound. Jello plays the
     question from the player's own song or presets (src-tauri/src/drills.rs).
     A chord or interval is answered by playing it back on the keys, checked
     a moment after the last key goes up, or by picking one of four answers.
     The sound drill is matched in the Sound panel, then measured. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  interface DrillView {
    kind: string;
    prompt: string;
    choices: string[];
    state: string;
    said: string;
    wrong: number;
    closeness: number | null;
  }

  const KINDS = [
    { kind: "chord", label: "Chord" },
    { kind: "interval", label: "Interval" },
    { kind: "sound", label: "Sound" },
  ];
  // How long after the last key goes up a play-back is checked, in ms.
  const CHECK_AFTER_MS = 900;
  // How long Jello plays a question, in ms (the longest, the sound drill's,
  // in src-tauri/src/drills.rs). Hear waits for it, so it never doubles.
  const QUESTION_MS = 1500;

  let drill = $state<DrillView | null>(null);
  let problem = $state("");
  // A click is on its way, so a quick second one waits.
  let busy = $state(false);
  let checkTimer: ReturnType<typeof setTimeout> | null = null;
  // The question is playing.
  let hearing = $state(false);
  let hearTimer: ReturnType<typeof setTimeout> | null = null;

  let over = $derived(isOver(drill));

  onMount(() => {
    window.addEventListener("keyup", keyUp);
  });

  onDestroy(() => {
    window.removeEventListener("keyup", keyUp);
    clearCheck();
    if (hearTimer !== null) {
      clearTimeout(hearTimer);
    }
    if (drill !== null) {
      invoke("end_drill");
    }
  });

  function isOver(view: DrillView | null): boolean {
    if (view === null) {
      return false;
    }
    return view.state === "right" || view.state === "told" || view.state === "close";
  }

  function clearCheck() {
    if (checkTimer !== null) {
      clearTimeout(checkTimer);
      checkTimer = null;
    }
  }

  // Keys typed into a text box are words, not a play-back.
  function isTyping(event: KeyboardEvent): boolean {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return false;
    }
    if (target.isContentEditable) {
      return true;
    }
    return target.tagName === "INPUT" || target.tagName === "TEXTAREA";
  }

  function keyUp(event: KeyboardEvent) {
    if (drill === null || over || drill.kind === "sound") {
      return;
    }
    if (isTyping(event)) {
      return;
    }
    clearCheck();
    checkTimer = setTimeout(check, CHECK_AFTER_MS);
  }

  async function run(action: () => Promise<DrillView>) {
    if (busy) {
      return;
    }
    busy = true;
    problem = "";
    try {
      drill = await action();
    } catch (error) {
      problem = String(error);
    }
    busy = false;
  }

  // Hear stays off while the question plays.
  function questionPlays() {
    hearing = true;
    if (hearTimer !== null) {
      clearTimeout(hearTimer);
    }
    hearTimer = setTimeout(() => {
      hearing = false;
      hearTimer = null;
    }, QUESTION_MS);
  }

  async function start(kind: string) {
    clearCheck();
    await run(() => invoke<DrillView>("start_drill", { kind }));
    if (problem === "") {
      questionPlays();
    }
  }

  // Another drill of the same kind.
  function next() {
    if (drill === null) {
      return;
    }
    start(drill.kind);
  }

  function answer(choice: string) {
    run(() => invoke<DrillView>("answer_drill", { choice }));
  }

  function check() {
    checkTimer = null;
    run(() => invoke<DrillView>("check_drill"));
  }

  async function hear() {
    if (hearing) {
      return;
    }
    problem = "";
    questionPlays();
    try {
      await invoke("hear_drill");
    } catch (error) {
      problem = String(error);
    }
  }

  async function stop() {
    clearCheck();
    drill = null;
    problem = "";
    await invoke("end_drill");
  }

  function labelOf(kind: string): string {
    for (const known of KINDS) {
      if (known.kind === kind) {
        return known.label;
      }
    }
    return kind;
  }
</script>

<div class="agent-jam">
  <div class="agent-jam-top">
    <span class="agent-jam-title">Drill</span>
    {#if drill !== null}
      <span class="agent-quiet">{labelOf(drill.kind)}</span>
      <button type="button" class="agent-button-plain" onclick={stop}>Stop</button>
    {/if}
  </div>
  {#if drill === null}
    <span class="agent-quiet">From your own song and presets.</span>
    <span class="agent-drill-buttons">
      {#each KINDS as known (known.kind)}
        <button type="button" class="agent-button-plain" disabled={busy} onclick={() => start(known.kind)}
          >{known.label}</button
        >
      {/each}
    </span>
  {:else}
    <p class="agent-practice-try">{drill.prompt}</p>
    {#if drill.choices.length > 0}
      <span class="agent-drill-choices">
        {#each drill.choices as choice (choice)}
          <button
            type="button"
            class="agent-button-plain"
            disabled={busy || over}
            onclick={() => answer(choice)}>{choice}</button
          >
        {/each}
      </span>
    {/if}
    {#if drill.said !== ""}
      <span class="agent-drill-said" class:is-right={drill.state === "right" || drill.state === "close"}
        >{drill.said}</span
      >
    {/if}
    <span class="agent-drill-buttons">
      <button type="button" class="agent-button-plain" disabled={hearing} onclick={hear}>Hear</button>
      {#if drill.kind === "sound" && !over}
        <button type="button" class="agent-button-plain" disabled={busy} onclick={check}>
          {#if busy}
            Measuring
          {:else}
            Measure
          {/if}
        </button>
      {/if}
      {#if over}
        <button type="button" class="agent-button-plain" disabled={busy} onclick={next}>Next</button>
      {/if}
    </span>
  {/if}
  {#if problem !== ""}
    <p class="agent-error">{problem}</p>
  {/if}
</div>
