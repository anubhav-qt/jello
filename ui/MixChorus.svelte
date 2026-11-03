<!-- The chorus's page (docs/design.md#mix): one wobbling line per copy,
     each its delay swinging, spread apart in time; then its speed in Hz or
     as a cycle in beats, and its values. -->
<script lang="ts">
  import {
    beatScale,
    curvedScale,
    effectSetting,
    linearScale,
    numberOf,
    percentScale,
    type MixSender,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, WIDTH } from "./mix-drawing";
  import MixChoice from "./MixChoice.svelte";
  import type { Picked, Setting } from "./sound-scales";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    place: string;
    slot: number;
    effect: SlotView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: MixSender;
  }

  let { place, slot, effect, picked, onPick, send }: Props = $props();

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let onBeat = $derived(effect.values.on_beat === true);

  let copies = $derived(setting("copies", "Copies", linearScale(1, 4, 1, "")));
  let speed = $derived(setting("speed_hz", "Speed", curvedScale(0.01, 10, "Hz")));
  let cycle = $derived(setting("cycle_beats", "Cycle", beatScale(32)));
  let depth = $derived(setting("depth", "Depth", percentScale(100)));
  let delay = $derived(setting("delay_ms", "Delay", curvedScale(1, 40, "ms")));
  let feedback = $derived(setting("feedback", "Feedback", percentScale(95)));
  let spread = $derived(setting("spread", "Spread", percentScale(100)));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));

  // One lane per copy, each a sine as deep as the depth, the copies
  // spread apart in time. Two cycles show across at any speed.
  let lines = $derived.by(() => {
    const count = Math.round(value("copies"));
    const lane = (HEIGHT - 2 * EDGE - 14) / count;
    const swing = (lane / 2 - 2) * Math.max(0.08, value("depth") / 100);
    const all: string[] = [];
    for (let copy = 0; copy < count; copy += 1) {
      const middle = EDGE + lane * copy + lane / 2;
      const offset = copy / count;
      const points: [number, number][] = [];
      for (let step = 0; step <= 96; step += 1) {
        const along = step / 96;
        const y = middle - swing * Math.sin(2 * Math.PI * (2 * along + offset));
        points.push([EDGE + along * (WIDTH - 2 * EDGE), y]);
      }
      all.push(pathOf(points));
    }
    return all;
  });

  let speedTiles = $derived([
    { name: "Hz", chosen: !onBeat, onClick: () => send.setEffectValue(place, slot, "on_beat", false) },
    { name: "Beats", chosen: onBeat, onClick: () => send.setEffectValue(place, slot, "on_beat", true) },
  ]);
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        {#each lines as line, copy (copy)}
          <path class="sound-lit is-fine" d={line} />
        {/each}
        <text class="sound-axis" x={EDGE + 4} y={HEIGHT - 6}>each line is a copy's delay swinging</text>
      </svg>
    </div>
    <MixChoice tiles={speedTiles} />
  </div>
  <div class="mix-fields">
    <SoundValue setting={copies} dots={4} {picked} {onPick} />
    {#if onBeat}
      <SoundValue setting={cycle} dots={9} {picked} {onPick} />
    {:else}
      <SoundValue setting={speed} dots={9} {picked} {onPick} />
    {/if}
    <SoundValue setting={depth} dots={9} {picked} {onPick} />
    <SoundValue setting={delay} dots={9} {picked} {onPick} />
    <SoundValue setting={feedback} dots={9} {picked} {onPick} />
    <SoundValue setting={spread} dots={9} {picked} {onPick} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
  </div>
</div>
