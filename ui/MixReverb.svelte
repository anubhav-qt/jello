<!-- The reverb's page (docs/design.md#mix): the sound, the gap before the
     room answers (a square to drag), and the tail dying away over its
     decay; Room, Hall and Plate as starting points, and Freeze, which
     holds the tail. Then its values. -->
<script lang="ts">
  import {
    curvedScale,
    effectSetting,
    hzScale,
    numberOf,
    percentScale,
    type MixSender,
    type SlotView,
    type Written,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfX, WIDTH, xOf, yOf } from "./mix-drawing";
  import MixChoice from "./MixChoice.svelte";
  import { isPicked, pickedOf, type Picked, type Setting } from "./sound-scales";
  import SoundHandle from "./SoundHandle.svelte";
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

  // Starting points: each sets these values, and is lit while they all
  // are as it set them.
  const ROOMS: { name: string; values: Record<string, Written> }[] = [
    { name: "Room", values: { size: 30, decay_s: 0.8, pre_delay_ms: 5, damping_hz: 9000, diffusion: 60 } },
    { name: "Hall", values: { size: 80, decay_s: 2.4, pre_delay_ms: 20, damping_hz: 8000, diffusion: 70 } },
    { name: "Plate", values: { size: 50, decay_s: 1.6, pre_delay_ms: 0, damping_hz: 12000, diffusion: 90 } },
  ];
  // The drawing shows the tail down to -60 dB.
  const QUIETEST_DB = -60;

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let size = $derived(setting("size", "Size", percentScale(100)));
  let decay = $derived(setting("decay_s", "Decay", curvedScale(0.1, 30, "s")));
  let preDelay = $derived(setting("pre_delay_ms", "Pre-delay", curvedScale(0, 500, "ms")));
  let damping = $derived(setting("damping_hz", "Damping", hzScale(1000, 20000)));
  let lowCut = $derived(setting("low_cut_hz", "Low cut", hzScale(20, 1000)));
  let diffusion = $derived(setting("diffusion", "Diffusion", percentScale(100)));
  let modulation = $derived(setting("modulation", "Movement", percentScale(100)));
  let width = $derived(setting("width", "Width", percentScale(100)));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));

  let frozen = $derived(effect.values.freeze === true);

  // The drawing spans the pre-delay and the decay, in seconds, with a
  // little room after.
  let span = $derived((value("pre_delay_ms") / 1000 + value("decay_s")) * 1.1);

  let tail = $derived.by(() => {
    const start = value("pre_delay_ms") / 1000;
    const decaySeconds = value("decay_s");
    const points: [number, number][] = [];
    for (let step = 0; step <= 80; step += 1) {
      const after = (step / 80) * decaySeconds;
      let db = (QUIETEST_DB * after) / decaySeconds;
      if (frozen) {
        db = -6;
      }
      points.push([xOf(start + after, 0, span), yOf(db, QUIETEST_DB, 0)]);
    }
    return pathOf(points);
  });

  let gapX = $derived(xOf(value("pre_delay_ms") / 1000, 0, span));

  let grabbedSpan: number | null = null;

  function grabGap() {
    grabbedSpan = span;
    onPick(pickedOf(preDelay));
  }

  function moveGap(x: number) {
    const ms = valueOfX(x, 0, grabbedSpan ?? span) * 1000;
    const within = Math.max(0, Math.min(500, ms));
    preDelay.set(preDelay.scale.at(preDelay.scale.share(within)));
  }

  function isRoom(values: Record<string, Written>): boolean {
    for (const word of Object.keys(values)) {
      if (Math.abs(value(word) - Number(values[word])) > 0.001) {
        return false;
      }
    }
    return true;
  }

  function startFrom(values: Record<string, Written>) {
    for (const word of Object.keys(values)) {
      send.setEffectValue(place, slot, word, values[word]);
    }
  }

  let roomTiles = $derived(
    ROOMS.map((room) => ({ name: room.name, chosen: isRoom(room.values), onClick: () => startFrom(room.values) })),
  );
  let freezeTiles = $derived([
    {
      name: "Freeze",
      chosen: frozen,
      title: "Holds the tail",
      onClick: () => send.setEffectValue(place, slot, "freeze", !frozen),
    },
  ]);

  let decayWords = $derived(`${decay.scale.words(value("decay_s"))} s to silence`);
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit" d={pathOf([[EDGE, HEIGHT - EDGE], [EDGE, EDGE]])} />
        <path class="sound-lit is-fine" d={tail} />
        <text class="sound-axis" x={WIDTH - EDGE - 96} y={EDGE + 10}>{decayWords}</text>
        <text class="sound-axis" x={gapX + 8} y={HEIGHT - EDGE - 2}>{preDelay.scale.words(value("pre_delay_ms"))} ms gap</text>
        <SoundHandle
          x={gapX}
          y={HEIGHT - EDGE}
          ringed={isPicked(picked, preDelay.id)}
          onGrab={grabGap}
          onMove={moveGap}
        />
      </svg>
    </div>
    <div class="mix-tile-rows">
      <MixChoice tiles={roomTiles} />
      <MixChoice tiles={freezeTiles} />
    </div>
  </div>
  <div class="mix-fields">
    <SoundValue setting={size} dots={9} {picked} {onPick} />
    <SoundValue setting={decay} dots={9} {picked} {onPick} />
    <SoundValue setting={preDelay} dots={9} {picked} {onPick} />
    <SoundValue setting={damping} dots={9} {picked} {onPick} />
    <SoundValue setting={lowCut} dots={9} {picked} {onPick} />
    <SoundValue setting={diffusion} dots={9} {picked} {onPick} />
    <SoundValue setting={modulation} dots={9} {picked} {onPick} />
    <SoundValue setting={width} dots={9} {picked} {onPick} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
  </div>
</div>
