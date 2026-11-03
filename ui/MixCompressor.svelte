<!-- The compressor's page (docs/design.md#mix): input against output, with
     a square at the threshold to drag and a meter of how far it turns the
     sound down now, then its values and what it listens to. -->
<script lang="ts">
  import {
    curvedScale,
    effectSetting,
    hzScale,
    LIMIT_RATIO,
    linearScale,
    numberOf,
    percentScale,
    RATIO,
    type MixSender,
    type MixView,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfX, WIDTH, xOf, yOf } from "./mix-drawing";
  import MixListensTo from "./MixListensTo.svelte";
  import MixReduction from "./MixReduction.svelte";
  import { isPicked, pickedOf, type Picked, type Setting } from "./sound-scales";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    place: string;
    slot: number;
    effect: SlotView;
    view: MixView;
    reduction: number;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: MixSender;
  }

  let { place, slot, effect, view, reduction, picked, onPick, send }: Props = $props();

  const LOWEST_DB = -60;

  // Where a square's words go: right of it, or left near the right edge.
  const LABEL_ROOM = 70;

  function labelX(x: number): number {
    if (x > WIDTH - LABEL_ROOM) {
      return x - LABEL_ROOM + 6;
    }
    return x + 8;
  }

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  // The ratio, with "limit" kept as LIMIT_RATIO.
  function ratioNow(): number {
    if (effect.values.ratio === "limit") {
      return LIMIT_RATIO;
    }
    return value("ratio");
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let threshold = $derived(setting("threshold_db", "Threshold", linearScale(-60, 0, 0.5, "dB")));
  let ratio: Setting = $derived({
    id: `mix ${place} ${slot} ratio`,
    name: "Ratio",
    scale: RATIO,
    now: ratioNow,
    set: (newRatio) => {
      if (newRatio >= LIMIT_RATIO) {
        send.setEffectValue(place, slot, "ratio", "limit");
        return;
      }
      send.setEffectValue(place, slot, "ratio", newRatio);
    },
  });
  let knee = $derived(setting("knee_db", "Knee", linearScale(0, 24, 0.5, "dB")));
  let attack = $derived(setting("attack_ms", "Attack", curvedScale(0.1, 100, "ms")));
  let release = $derived(setting("release_ms", "Release", curvedScale(5, 2000, "ms")));
  let makeup = $derived(setting("makeup_db", "Makeup", linearScale(0, 24, 0.5, "dB")));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));
  let lowCut = $derived(setting("low_cut_hz", "Low cut", hzScale(20, 500)));

  // The output for an input, both in dB, without the knee or makeup.
  function outputDb(input: number): number {
    const at = value("threshold_db");
    if (input <= at) {
      return input;
    }
    const over = ratioNow();
    if (over >= LIMIT_RATIO) {
      return at;
    }
    return at + (input - at) / over;
  }

  let curve = $derived.by(() => {
    const points: [number, number][] = [];
    for (let input = LOWEST_DB; input <= 0; input += 1) {
      points.push([xOf(input, LOWEST_DB, 0), yOf(outputDb(input), LOWEST_DB, 0)]);
    }
    return pathOf(points);
  });

  let squareX = $derived(xOf(numberOf(effect.values.threshold_db), LOWEST_DB, 0));
  let squareY = $derived(yOf(numberOf(effect.values.threshold_db), LOWEST_DB, 0));

  function grabThreshold() {
    onPick(pickedOf(threshold));
  }

  function moveThreshold(x: number) {
    const db = valueOfX(x, LOWEST_DB, 0);
    threshold.set(threshold.scale.at(threshold.scale.share(db)));
  }
</script>

<div class="mix-effect">
  <div class="mix-drawing-row">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit-soft is-faint" d={pathOf([[EDGE, HEIGHT - EDGE], [WIDTH - EDGE, EDGE]])} />
        <path class="sound-lit" d={curve} />
        <text class="sound-axis" x={labelX(squareX)} y={squareY + 16}>{threshold.scale.words(threshold.now())} dB</text>
        <SoundHandle
          x={squareX}
          y={squareY}
          ringed={isPicked(picked, threshold.id)}
          onGrab={grabThreshold}
          onMove={moveThreshold}
        />
      </svg>
    </div>
    <MixReduction db={reduction} />
  </div>
  <div class="mix-fields">
    <SoundValue setting={threshold} dots={9} {picked} {onPick} />
    <SoundValue setting={ratio} dots={9} {picked} {onPick} />
    <SoundValue setting={knee} dots={9} {picked} {onPick} />
    <SoundValue setting={attack} dots={9} {picked} {onPick} />
    <SoundValue setting={release} dots={9} {picked} {onPick} />
    <SoundValue setting={makeup} dots={9} {picked} {onPick} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
    <SoundValue setting={lowCut} dots={9} {picked} {onPick} />
    <MixListensTo {place} {slot} {effect} {view} {send} />
  </div>
</div>
