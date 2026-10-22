<!-- The sound's dev panel: plain sliders for tuning unison
     (docs/adr/0029-unison.md) and the low-pass filter
     (docs/adr/0028-a-low-pass-filter.md) by ear until their screen is
     designed. It opens over Compose's tape, as wide as it. Cutoff and
     resonance move every note, even those already sounding; unison and the
     filter envelope start with the next note. -->
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
    UNISON_MOST_COPIES,
    UNISON_MOST_DETUNE_CENTS,
    type FilterEnvelope,
    type FilterView,
    type Unison,
    type UnisonView,
  } from "./sound";

  interface Props {
    // Where the filter is and unison's copies, as Rust answered.
    filter: FilterView;
    unison: UnisonView;
    // How many key widths wide the keyboard is, so the panel is as wide.
    keysAcross: number;
    // A filter slider moved: `cutoffHz` is middle C's cutoff, and
    // `resonance` is in percent.
    onFilterChange: (cutoffHz: number, resonance: number) => void;
    // A filter envelope slider moved.
    onEnvelopeChange: (envelope: FilterEnvelope) => void;
    // A unison slider moved.
    onUnisonChange: (unison: Unison) => void;
  }

  let { filter, unison, keysAcross, onFilterChange, onEnvelopeChange, onUnisonChange }: Props = $props();

  // Where the sound is as the panel opens, kept on purpose: the sliders
  // start there, then follow the hand, so a drag never jumps back to where
  // Rust rounded it. The readouts beside them show what Rust answered.
  const opening = untrack(() => filter);
  const openingUnison = untrack(() => unison);

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
  let copies = $state(openingUnison.copies);
  let detuneCents = $state(openingUnison.detune_cents);
  let spread = $state(openingUnison.spread);

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

  function sendUnison() {
    onUnisonChange({ copies, detuneCents, spread });
  }

  function moveCopies(count: number) {
    copies = count;
    sendUnison();
  }

  function moveDetune(cents: number) {
    detuneCents = cents;
    sendUnison();
  }

  function moveSpread(percent: number) {
    spread = percent;
    sendUnison();
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
{#snippet slider(
  name: string,
  words: string,
  position: number,
  min: number,
  max: number,
  step: number,
  move: (value: number) => void,
)}
  <label class="sound-field">
    <span class="sound-field-top">
      <span class="sound-field-name">{name}</span>
      <span class="sound-field-value">{words}</span>
    </span>
    <input
      type="range"
      {min}
      {max}
      {step}
      value={position}
      oninput={(event) => move(sliderValue(event))}
      onchange={letGo}
    />
  </label>
{/snippet}

<section class="dev-panel sound-panel" style:--keys-across={keysAcross}>
  <p class="dev-panel-title">Dev panel: sound, not designed yet</p>
  <div class="sound-rows">
    <div class="sound-row has-two-groups">
      <span class="sound-row-name">Unison, next note</span>
      {@render slider("Copies", `${unison.copies}`, copies, 1, UNISON_MOST_COPIES, 1, moveCopies)}
      {@render slider("Detune", `${unison.detune_cents} cents`, detuneCents, 0, UNISON_MOST_DETUNE_CENTS, 1, moveDetune)}
      {@render slider("Spread", `${unison.spread}%`, spread, 0, 100, 1, moveSpread)}

      <span class="sound-row-name">Filter</span>
      {@render slider("Cutoff", cutoffWords(filter.cutoff_hz), cutoff, 0, SLIDER_STEPS, 1, moveCutoff)}
      {@render slider("Resonance", `${filter.resonance}%`, resonance, 0, 100, 1, moveResonance)}
    </div>

    <div class="sound-row">
      <span class="sound-row-name">Envelope, next note</span>
      {@render slider("Amount", `${filter.amount.toFixed(1)} oct`, amount, 0, FILTER_MOST_OCTAVES, 0.1, moveAmount)}
      {@render slider("Attack", `${filter.attack_ms} ms`, attack, 0, SLIDER_STEPS, 1, moveAttack)}
      {@render slider("Decay", `${filter.decay_ms} ms`, decay, 0, SLIDER_STEPS, 1, moveDecay)}
      {@render slider("Sustain", `${filter.sustain}%`, sustain, 0, 100, 1, moveSustain)}
      {@render slider("Release", `${filter.release_ms} ms`, release, 0, SLIDER_STEPS, 1, moveRelease)}
    </div>
  </div>
</section>
