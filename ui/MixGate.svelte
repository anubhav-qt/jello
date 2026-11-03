<!-- The gate's page (docs/design.md#mix): input against output, dropping by
     its range below the threshold, with a square at the threshold and one
     at the range, and a dot lit while it is open. -->
<script lang="ts">
  import {
    curvedScale,
    effectSetting,
    GATE_SILENT_DB,
    linearScale,
    numberOf,
    type MixSender,
    type MixView,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfX, valueOfY, WIDTH, xOf, yOf } from "./mix-drawing";
  import MixListensTo from "./MixListensTo.svelte";
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

  // The drawing reads from LOWEST_DB to 0 dB both ways.
  const LOWEST_DB = -80;
  // It counts as open while it turns down less than this, in dB.
  const OPEN_UNDER_DB = 1;

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

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let threshold = $derived(setting("threshold_db", "Threshold", linearScale(-80, 0, 0.5, "dB")));
  let range = $derived(setting("range_db", "Range", linearScale(GATE_SILENT_DB, 0, 0.5, "dB")));
  let attack = $derived(setting("attack_ms", "Attack", curvedScale(0.1, 50, "ms")));
  let hold = $derived(setting("hold_ms", "Hold", curvedScale(0, 500, "ms")));
  let release = $derived(setting("release_ms", "Release", curvedScale(5, 2000, "ms")));

  let open = $derived(reduction < OPEN_UNDER_DB);

  // Under the threshold the output drops by the range; at a silent range
  // it goes off the bottom.
  let curve = $derived.by(() => {
    const at = value("threshold_db");
    const drop = value("range_db");
    const below: [number, number][] = [];
    for (let input = LOWEST_DB; input <= at; input += 1) {
      below.push([xOf(input, LOWEST_DB, 0), yOf(input + drop, LOWEST_DB, 0)]);
    }
    const above: [number, number][] = [
      [xOf(at, LOWEST_DB, 0), yOf(at, LOWEST_DB, 0)],
      [xOf(0, LOWEST_DB, 0), yOf(0, LOWEST_DB, 0)],
    ];
    return { below: pathOf(below), above: pathOf(above) };
  });

  let thresholdX = $derived(xOf(numberOf(effect.values.threshold_db), LOWEST_DB, 0));
  let thresholdY = $derived(yOf(numberOf(effect.values.threshold_db), LOWEST_DB, 0));
  let rangeY = $derived(
    yOf(numberOf(effect.values.threshold_db) + numberOf(effect.values.range_db), LOWEST_DB, 0),
  );

  function moveThreshold(x: number) {
    const db = valueOfX(x, LOWEST_DB, 0);
    threshold.set(threshold.scale.at(threshold.scale.share(db)));
  }

  function moveRange(_x: number, y: number) {
    const db = valueOfY(y, LOWEST_DB, 0) - value("threshold_db");
    const within = Math.max(GATE_SILENT_DB, Math.min(0, db));
    range.set(range.scale.at(range.scale.share(within)));
  }

  let rangeWords = $derived.by(() => {
    const drop = numberOf(effect.values.range_db);
    if (drop <= GATE_SILENT_DB) {
      return "silent";
    }
    return `${-drop} dB down`;
  });
</script>

<div class="mix-effect">
  <div class="mix-drawing-row">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit-soft is-faint" d={pathOf([[EDGE, HEIGHT - EDGE], [WIDTH - EDGE, EDGE]])} />
        <path class="sound-lit" d={curve.below} />
        <path class="sound-lit" d={curve.above} />
        <text class="sound-axis" x={labelX(thresholdX)} y={thresholdY + 14}>{threshold.scale.words(threshold.now())} dB</text>
        <text class="sound-axis" x={labelX(thresholdX)} y={Math.min(rangeY + 4, HEIGHT - 4)}>{rangeWords}</text>
        <SoundHandle
          x={thresholdX}
          y={thresholdY}
          ringed={isPicked(picked, threshold.id)}
          onGrab={() => onPick(pickedOf(threshold))}
          onMove={moveThreshold}
        />
        <SoundHandle
          x={thresholdX}
          y={rangeY}
          ringed={isPicked(picked, range.id)}
          onGrab={() => onPick(pickedOf(range))}
          onMove={moveRange}
        />
      </svg>
    </div>
    <div class="mix-gate-state" class:is-open={open}>
      <i></i>
      <span>{open ? "open" : "closed"}</span>
    </div>
  </div>
  <div class="mix-fields">
    <SoundValue setting={threshold} dots={9} {picked} {onPick} />
    <SoundValue setting={range} dots={9} {picked} {onPick} />
    <SoundValue setting={attack} dots={9} {picked} {onPick} />
    <SoundValue setting={hold} dots={9} {picked} {onPick} />
    <SoundValue setting={release} dots={9} {picked} {onPick} />
    <MixListensTo {place} {slot} {effect} {view} {send} />
  </div>
</div>
