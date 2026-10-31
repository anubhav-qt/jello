<!-- The looper's transport: one dark panel under the track row, split into
     segments. Record, the recording length and the tempo, each stepped with
     ‹ and ›, the tempo typed too, the loop's length in seconds, then the
     vibrato and glide lamps, lit while held. The free room before the lamps
     is kept for more of them later. -->
<script lang="ts">
  import { untrack } from "svelte";
  import DigitField from "./DigitField.svelte";
  import { clickThenLetGo } from "./dom";
  import type { SoundView } from "./sound";

  interface Props {
    sound: SoundView;
    onRecord: () => void;
    onTempo: (tempo: number) => void;
    onExpression: (key: string, held: boolean) => void;
    // In drum mode Space hits the kick, so the pedal's lamp can't hold it.
    drumMode: boolean;
  }
  let { sound, onRecord, onTempo, onExpression, drumMode }: Props = $props();

  // The recording lengths to pick from, in bars; 0 is free.
  const LENGTHS = [0, 1, 2, 3, 4, 6, 8, 12, 16];
  const SLOWEST = 40;
  const FASTEST = 240;

  let bars = $state(8);
  let vibrato = $state(false);
  let glide = $state(false);
  let pedal = $state(false);
  let looperState = $derived(sound.looper.state);
  let tempo = $derived(sound.tempo);
  let recording = $derived(
    looperState === "recording" ||
      looperState === "layering" ||
      looperState === "counting in",
  );
  let label = $derived.by(() => {
    if (looperState === "counting in") {
      return "Cancel count-in";
    }
    if (looperState === "recording") {
      return "Close loop";
    }
    if (looperState === "layering") {
      return "Finish layer";
    }
    if (looperState === "idle") {
      return "Record loop";
    }
    return "Record layer";
  });
  let lengthPlace = $derived(LENGTHS.indexOf(bars));
  let barsWord = $derived.by(() => {
    if (bars === 1) {
      return "bar";
    }
    return "bars";
  });
  let seconds = $derived(((bars * 240) / tempo).toFixed(1));

  // The first recording closes at the nearest bar; overdubs stop after the
  // selected duration. A free recording still closes with the same button.
  $effect(() => {
    const current = looperState;
    const length = bars;
    const bpm = tempo;
    if (length === 0) return;
    let beats = length * 4;
    if (current === "counting in") {
      beats += 4 - untrack(() => sound.looper.beats_in);
    } else if (current === "recording") {
      beats -= untrack(() => sound.looper.beats_in);
    } else if (current !== "layering") {
      return;
    }
    const timer = window.setTimeout(onRecord, Math.max(0, beats * 60000 / bpm - 60));
    return () => window.clearTimeout(timer);
  });

  // Moves the recording length one step along LENGTHS.
  function stepLength(steps: number) {
    const place = lengthPlace + steps;
    if (place < 0 || place >= LENGTHS.length) {
      return;
    }
    bars = LENGTHS[place];
  }

  function stepTempo(steps: number) {
    const newTempo = tempo + steps;
    if (newTempo < SLOWEST || newTempo > FASTEST) {
      return;
    }
    onTempo(newTempo);
  }

  function switchVibrato() {
    vibrato = !vibrato;
    onExpression("ShiftLeft", vibrato);
  }

  function switchGlide() {
    glide = !glide;
    onExpression("AltLeft", glide);
  }

  function switchPedal() {
    pedal = !pedal;
    onExpression("Space", pedal);
  }
</script>

<svelte:window onblur={() => { vibrato = false; glide = false; pedal = false; }} />
<div class="loop-transport">
  <button
    type="button"
    class="transport-record"
    class:is-recording={recording}
    onclick={(event) => clickThenLetGo(event, onRecord)}
  >
    <span class="transport-record-dot"></span>{label}
  </button>

  <div class="transport-segment" class:is-off={recording}>
    <span class="transport-label">Length</span>
    <button
      type="button"
      class="transport-step"
      aria-label="Shorter recording"
      disabled={recording || lengthPlace <= 0}
      onclick={(event) => clickThenLetGo(event, () => stepLength(-1))}
    >‹</button>
    {#if bars === 0}
      <span class="transport-value">Free</span>
    {:else}
      <span class="transport-value">{bars}</span>
      <span class="transport-unit">{barsWord}</span>
    {/if}
    <button
      type="button"
      class="transport-step"
      aria-label="Longer recording"
      disabled={recording || lengthPlace >= LENGTHS.length - 1}
      onclick={(event) => clickThenLetGo(event, () => stepLength(1))}
    >›</button>
  </div>

  <div class="transport-segment" class:is-off={sound.looper.busy}>
    <span class="transport-label">Tempo</span>
    <button
      type="button"
      class="transport-step"
      aria-label="Slower"
      disabled={sound.looper.busy || tempo <= SLOWEST}
      onclick={(event) => clickThenLetGo(event, () => stepTempo(-1))}
    >‹</button>
    <DigitField
      value={tempo}
      lowest={SLOWEST}
      highest={FASTEST}
      label="Tempo BPM"
      disabled={sound.looper.busy}
      onSet={onTempo}
    />
    <span class="transport-unit">bpm</span>
    <button
      type="button"
      class="transport-step"
      aria-label="Faster"
      disabled={sound.looper.busy || tempo >= FASTEST}
      onclick={(event) => clickThenLetGo(event, () => stepTempo(1))}
    >›</button>
  </div>

  <div class="transport-segment" title="The recording's length in seconds">
    {#if bars === 0}
      <span class="transport-unit">any length</span>
    {:else}
      <span class="transport-value">{seconds}</span>
      <span class="transport-unit">s</span>
    {/if}
  </div>

  <span class="transport-gap"></span>

  <div class="transport-segment transport-lamps">
    <button
      type="button"
      class="transport-lamp"
      class:is-on={vibrato}
      aria-pressed={vibrato}
      aria-label="Hold vibrato"
      onclick={(event) => clickThenLetGo(event, switchVibrato)}
    >
      <span class="transport-lamp-light"></span>Vibrato
    </button>
    <button
      type="button"
      class="transport-lamp"
      class:is-on={glide}
      aria-pressed={glide}
      aria-label="Hold glide"
      onclick={(event) => clickThenLetGo(event, switchGlide)}
    >
      <span class="transport-lamp-light"></span>Glide
    </button>
    <button
      type="button"
      class="transport-lamp"
      class:is-on={pedal}
      aria-pressed={pedal}
      aria-label="Hold the pedal"
      disabled={drumMode && !pedal}
      onclick={(event) => clickThenLetGo(event, switchPedal)}
    >
      <span class="transport-lamp-light"></span>Pedal
    </button>
  </div>
</div>

<style>
  /* Sizes are in em, so the panel scales with the keys (.stage-controls). */
  .loop-transport {
    display: flex;
    align-items: stretch;
    height: 3.38em;
    margin-top: 0.77em;
    background: var(--panel-bg);
    border: 1px solid rgba(var(--ink), 0.09);
    border-radius: 5px;
    white-space: nowrap;
  }
  /* Every segment is split from the next by a thin line. */
  .loop-transport > * {
    display: flex;
    flex: none;
    align-items: center;
    gap: 0.46em;
    padding: 0 0.7em;
    border: 0;
    border-right: 1px solid rgba(var(--ink), 0.09);
  }
  .loop-transport > :last-child {
    border-right: 0;
  }
  /* The free room between the settings and the lamps, with no line. */
  .loop-transport > .transport-gap {
    flex: 1 1 0;
    padding: 0;
    border-right: 0;
  }
  button {
    color: inherit;
    background: none;
    font: inherit;
    cursor: pointer;
  }
  button:disabled {
    cursor: default;
  }
  .transport-record {
    min-width: 9.8em;
    gap: 0.69em;
    color: rgba(var(--ink), 0.9);
    font-weight: 600;
    border-radius: 5px 0 0 5px;
  }
  .transport-record-dot {
    width: 0.62em;
    height: 0.62em;
    border-radius: 50%;
    background: rgb(var(--rec));
    box-shadow: 0 0 8px rgba(var(--rec), 0.7);
  }
  .transport-record.is-recording {
    color: rgb(var(--rec));
    background: rgba(var(--rec), 0.08);
  }
  .transport-record:hover {
    background: rgba(var(--ink), 0.03);
  }
  .transport-label {
    margin-right: 0.15em;
    font-size: 0.81em;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .transport-value {
    color: rgba(var(--ink), 0.92);
    text-shadow: 0 0 10px rgba(var(--ink), 0.25);
    font-family: "Doto", monospace;
    font-weight: 800;
    font-size: 1.31em;
  }
  .transport-unit {
    color: var(--text-3);
    font-size: 0.92em;
  }
  .transport-segment.is-off > :not(.transport-label) {
    opacity: 0.5;
  }
  /* Its own font is a little bigger, so its em here is that size. */
  .transport-step {
    padding: 0.13em 0.2em;
    color: var(--text-3);
    border: 0;
    font-size: 1.15em;
  }
  .transport-step:hover:not(:disabled) {
    color: rgb(var(--accent));
  }
  .transport-step:disabled {
    opacity: 0.35;
  }
  .transport-lamps {
    gap: 1em;
  }
  /* Its own font is smaller, so its em here is that size. */
  .transport-lamp {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0;
    border: 0;
    gap: 0.67em;
    color: var(--text-2);
    font-size: 0.81em;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }
  .transport-lamp-light {
    width: 0.57em;
    height: 0.57em;
    border-radius: 50%;
    background: rgba(var(--ink), 0.14);
  }
  .transport-lamp.is-on {
    color: rgb(var(--accent));
  }
  .transport-lamp.is-on .transport-lamp-light {
    background: rgb(var(--accent));
    box-shadow: 0 0 8px rgb(var(--accent));
  }
  .transport-lamp:hover {
    color: rgb(var(--accent));
  }
</style>
