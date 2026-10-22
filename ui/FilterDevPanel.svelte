<!-- The low-pass filter's dev panel (docs/adr/0028-a-low-pass-filter.md):
     plain sliders for tuning the filter by ear until its screen is
     designed. It opens over Compose's tape, as wide as it. Cutoff and
     resonance move every note, even those already sounding; the filter
     envelope starts with the next note. -->
<script lang="ts">
  import { untrack } from "svelte";
  import {
    cutoffAt,
    cutoffPosition,
    cutoffWords,
    FILTER_MOST_OCTAVES,
    SLIDER_STEPS,
    stageMsAt,
    stagePosition,
    type FilterEnvelope,
    type FilterView,
  } from "./sound";

  interface Props {
    // Where the filter is, as Rust answered.
    filter: FilterView;
    // How many key widths wide the keyboard is, so the panel is as wide.
    keysAcross: number;
    // A filter slider moved: `cutoffHz` is middle C's cutoff, and
    // `resonance` is in percent.
    onFilterChange: (cutoffHz: number, resonance: number) => void;
    // A filter envelope slider moved.
    onEnvelopeChange: (envelope: FilterEnvelope) => void;
  }

  let { filter, keysAcross, onFilterChange, onEnvelopeChange }: Props = $props();

  // Where the filter is as the panel opens, kept on purpose: the sliders
  // start there, then follow the hand, so a drag never jumps back to where
  // Rust rounded it. The readouts beside them show what Rust answered.
  const opening = untrack(() => filter);

  // What the panel sends. A slider moved sends every value, so each keeps
  // the exact value it had until its own slider moves: a 300 ms decay
  // stays 300, though its slider has no spot for exactly 300.
  let cutoffHz = $state(opening.cutoff_hz);
  let resonance = $state(opening.resonance);
  let amount = $state(opening.amount);
  let attackMs = $state(opening.attack_ms);
  let decayMs = $state(opening.decay_ms);
  let sustain = $state(opening.sustain);
  let releaseMs = $state(opening.release_ms);

  // Where the curved sliders sit. The others sit at their value.
  let cutoff = $state(cutoffPosition(opening.cutoff_hz));
  let attack = $state(stagePosition(opening.attack_ms));
  let decay = $state(stagePosition(opening.decay_ms));
  let release = $state(stagePosition(opening.release_ms));

  function sendFilter() {
    onFilterChange(cutoffHz, resonance);
  }

  function sendEnvelope() {
    onEnvelopeChange({ amount, attackMs, decayMs, sustain, releaseMs });
  }

  function moveCutoff(position: number) {
    cutoff = position;
    cutoffHz = cutoffAt(position);
    sendFilter();
  }

  function moveResonance(percent: number) {
    resonance = percent;
    sendFilter();
  }

  function moveAmount(octaves: number) {
    amount = octaves;
    sendEnvelope();
  }

  function moveAttack(position: number) {
    attack = position;
    attackMs = stageMsAt(position);
    sendEnvelope();
  }

  function moveDecay(position: number) {
    decay = position;
    decayMs = stageMsAt(position);
    sendEnvelope();
  }

  function moveSustain(percent: number) {
    sustain = percent;
    sendEnvelope();
  }

  function moveRelease(position: number) {
    release = position;
    releaseMs = stageMsAt(position);
    sendEnvelope();
  }

  // The number a slider moved to.
  function sliderValue(event: Event): number {
    const slider = event.currentTarget as HTMLInputElement;
    return Number(slider.value);
  }

  // Once the hand lets go, the slider lets go of the focus too, like the
  // buttons do, so keys played later can never move it.
  function letGo(event: Event) {
    const slider = event.currentTarget as HTMLElement;
    slider.blur();
  }
</script>

<!-- One slider: its name and readout over it. -->
{#snippet slider(name: string, words: string, position: number, max: number, step: number, move: (value: number) => void)}
  <label class="filter-field">
    <span class="filter-field-top">
      <span class="filter-field-name">{name}</span>
      <span class="filter-field-value">{words}</span>
    </span>
    <input
      type="range"
      min="0"
      {max}
      {step}
      value={position}
      oninput={(event) => move(sliderValue(event))}
      onchange={letGo}
    />
  </label>
{/snippet}

<section class="dev-panel filter-panel" style:--keys-across={keysAcross}>
  <p class="dev-panel-title">Dev panel: filter, not designed yet</p>
  <div class="filter-rows">
    <span class="filter-row-name">Filter</span>
    {@render slider("Cutoff", cutoffWords(filter.cutoff_hz), cutoff, SLIDER_STEPS, 1, moveCutoff)}
    {@render slider("Resonance", `${filter.resonance}%`, resonance, 100, 1, moveResonance)}

    <span class="filter-row-name">Envelope, next note</span>
    {@render slider("Amount", `${filter.amount.toFixed(1)} oct`, amount, FILTER_MOST_OCTAVES, 0.1, moveAmount)}
    {@render slider("Attack", `${filter.attack_ms} ms`, attack, SLIDER_STEPS, 1, moveAttack)}
    {@render slider("Decay", `${filter.decay_ms} ms`, decay, SLIDER_STEPS, 1, moveDecay)}
    {@render slider("Sustain", `${filter.sustain}%`, sustain, 100, 1, moveSustain)}
    {@render slider("Release", `${filter.release_ms} ms`, release, SLIDER_STEPS, 1, moveRelease)}
  </div>
</section>
