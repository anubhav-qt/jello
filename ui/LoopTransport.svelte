<script lang="ts">
  import { untrack } from "svelte";
  import { clickThenLetGo } from "./dom";
  import type { SoundView } from "./sound";

  interface Props {
    sound: SoundView;
    onRecord: () => void;
    onTempo: (tempo: number) => void;
    onExpression: (key: string, held: boolean) => void;
    onExport: () => void;
  }
  let { sound, onRecord, onTempo, onExpression, onExport }: Props = $props();
  let bars = $state(8);
  let vibrato = $state(false);
  let glide = $state(false);
  let looperState = $derived(sound.looper.state);
  let tempo = $derived(sound.tempo);
  let recording = $derived(looperState === "recording" || looperState === "layering" || looperState === "counting in");
  let label = $derived.by(() => {
    if (looperState === "counting in") return "Cancel count-in";
    if (looperState === "recording") return "Close loop";
    if (looperState === "layering") return "Finish layer";
    if (looperState === "idle") return "Record loop";
    return "Record layer";
  });

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
</script>

<svelte:window onblur={() => { vibrato = false; glide = false; }} />
<div class="loop-transport">
  <button class:is-recording={recording} onclick={(event) => clickThenLetGo(event, onRecord)}>{label}</button>
  <label>Record length
    <select aria-label="Recording length in bars" bind:value={bars} disabled={recording}>
      <option value={0}>Free</option>
      {#each [1, 2, 3, 4, 6, 8, 12, 16] as length}
        <option value={length}>{length} bars</option>
      {/each}
    </select>
  </label>
  <label>Tempo
    <input aria-label="Tempo BPM" type="number" min="40" max="240" step="1" value={sound.tempo}
      disabled={sound.looper.busy}
      onchange={(event) => onTempo(event.currentTarget.valueAsNumber)} />
    BPM
  </label>
  <span>{(bars * 240 / sound.tempo).toFixed(1)} s</span>
  <label><input type="checkbox" aria-label="Hold vibrato" bind:checked={vibrato} onchange={() => onExpression("ShiftLeft", vibrato)} />Vibrato</label>
  <label><input type="checkbox" aria-label="Hold glide" bind:checked={glide} onchange={() => onExpression("AltLeft", glide)} />Glide</label>
  <button disabled={sound.looper.layers.length === 0 || recording} onclick={(event) => clickThenLetGo(event, onExport)}>Export loop WAV</button>
</div>

<style>
  .loop-transport { display: flex; justify-content: center; align-items: center; flex-wrap: wrap; gap: 14px; margin-bottom: 10px; color: var(--text-2); font-size: 12px; }
  label { display: flex; align-items: center; gap: 7px; }
  button, select, input { padding: 5px 9px; color: var(--text); background: var(--panel-bg); border: 1px solid rgba(var(--accent), .35); border-radius: 3px; font: inherit; }
  button { color: rgb(var(--accent)); min-width: 110px; }
  button.is-recording { color: rgb(var(--rec)); border-color: rgb(var(--rec)); }
  input { width: 65px; }
  input[type="checkbox"] { width: auto; }
  :disabled { opacity: .5; }
</style>
