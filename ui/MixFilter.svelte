<!-- The filter's page (docs/design.md#mix): its curve with one square for
     the cutoff (across) and resonance (up), and a faint curve where the
     loudness it follows takes the cutoff; then its type, slope and values,
     and what it follows. -->
<script lang="ts">
  import {
    curvedScale,
    effectSetting,
    linearScale,
    numberOf,
    percentScale,
    type MixSender,
    type MixView,
    type SlotView,
  } from "./mix";
  import { curveHz, EDGE, HEIGHT, hzOfX, pathOf, valueOfY, WIDTH, xOfHz, yOf } from "./mix-drawing";
  import MixChoice from "./MixChoice.svelte";
  import MixListensTo from "./MixListensTo.svelte";
  import { cutoffScale, filterDb, isPicked, pickedOf, type Picked, type Setting } from "./sound-scales";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    place: string;
    slot: number;
    effect: SlotView;
    view: MixView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: MixSender;
  }

  let { place, slot, effect, view, picked, onPick, send }: Props = $props();

  // The drawing reads from LOWEST_DB to HIGHEST_DB.
  const LOWEST_DB = -48;
  const HIGHEST_DB = 24;
  const TYPES = [
    { word: "low-pass", name: "Low-pass" },
    { word: "high-pass", name: "High-pass" },
    { word: "band-pass", name: "Band-pass" },
    { word: "notch", name: "Notch" },
  ];

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  // The cutoff writes "open" at the type's open end, as the sound panel's
  // filter does.
  let cutoff = $derived(setting("cutoff_hz", "Cutoff", cutoffScale(String(effect.values.type))));
  let resonance = $derived(setting("resonance", "Resonance", percentScale(100)));
  let drive = $derived(setting("drive", "Drive", percentScale(100)));
  let follow = $derived(setting("follow_octaves", "Follow", linearScale(-8, 8, 0.1, "oct", true)));
  let attack = $derived(setting("follow_attack_ms", "Attack", curvedScale(0.1, 100, "ms")));
  let release = $derived(setting("follow_release_ms", "Release", curvedScale(5, 2000, "ms")));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));

  let filterType = $derived(String(effect.values.type));

  // The filter's dB at `hz` with its cutoff at `cutoffHz`: twice as steep
  // at 24 dB an octave.
  function responseDb(hz: number, cutoffHz: number): number {
    const once = filterDb(filterType, hz, cutoffHz, value("resonance"));
    if (value("slope") >= 24) {
      return 2 * once;
    }
    return once;
  }

  function curveAt(cutoffHz: number): string {
    const points: [number, number][] = [];
    for (const hz of curveHz(140)) {
      points.push([xOfHz(hz), yOf(responseDb(hz, cutoffHz), LOWEST_DB, HIGHEST_DB)]);
    }
    return pathOf(points);
  }

  let curve = $derived(curveAt(value("cutoff_hz")));

  // Where loudness takes the cutoff, when it follows any.
  let followed = $derived.by(() => {
    const octaves = value("follow_octaves");
    if (octaves === 0) {
      return null;
    }
    const hz = Math.max(20, Math.min(20000, value("cutoff_hz") * Math.pow(2, octaves)));
    return curveAt(hz);
  });

  // The square sits at the cutoff, as high as the resonance.
  let squareX = $derived(xOfHz(value("cutoff_hz")));
  let squareY = $derived(yOf((value("resonance") / 100) * HIGHEST_DB, LOWEST_DB, HIGHEST_DB));

  function moveSquare(x: number, y: number) {
    send.setEffectValue(place, slot, "cutoff_hz", hzOfX(x));
    const db = valueOfY(y, LOWEST_DB, HIGHEST_DB);
    const percent = Math.max(0, Math.min(100, Math.round((db / HIGHEST_DB) * 100)));
    send.setEffectValue(place, slot, "resonance", percent);
  }

  let typeTiles = $derived(
    TYPES.map((each) => ({
      name: each.name,
      chosen: filterType === each.word,
      onClick: () => send.setEffectValue(place, slot, "type", each.word),
    })),
  );

  let slopeTiles = $derived(
    [12, 24].map((each) => ({
      name: `${each} dB`,
      chosen: value("slope") === each,
      onClick: () => send.setEffectValue(place, slot, "slope", each),
    })),
  );

  let followWords = $derived.by(() => {
    const octaves = value("follow_octaves");
    if (octaves === 0) {
      return "";
    }
    let sign = "";
    if (octaves > 0) {
      sign = "+";
    }
    return `${sign}${octaves} oct when loud`;
  });
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        {#if followed !== null}
          <path class="sound-lit-soft is-faint" d={followed} />
        {/if}
        <path class="sound-lit is-fine" d={curve} />
        <text class="sound-axis" x={EDGE + 4} y={EDGE + 10}>{followWords}</text>
        <SoundHandle
          x={squareX}
          y={squareY}
          ringed={isPicked(picked, cutoff.id) || isPicked(picked, resonance.id)}
          onGrab={() => onPick(pickedOf(cutoff))}
          onMove={moveSquare}
        />
      </svg>
    </div>
    <MixChoice tiles={slopeTiles} label="Slope, an octave" />
  </div>
  <div class="mix-fields">
    <div class="mix-wide">
      <MixChoice tiles={typeTiles} />
    </div>
    <SoundValue setting={cutoff} dots={9} {picked} {onPick} />
    <SoundValue setting={resonance} dots={9} {picked} {onPick} />
    <SoundValue setting={drive} dots={9} {picked} {onPick} />
    <SoundValue setting={follow} dots={9} {picked} {onPick} />
    <SoundValue setting={attack} dots={9} {picked} {onPick} dim={value("follow_octaves") === 0} />
    <SoundValue setting={release} dots={9} {picked} {onPick} dim={value("follow_octaves") === 0} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
    <MixListensTo {place} {slot} {effect} {view} {send} label="Follows" />
  </div>
</div>
