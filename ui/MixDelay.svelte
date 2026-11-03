<!-- The delay's page (docs/design.md#mix): its echoes as bars, left on top
     and right underneath, each quieter, bouncing between the sides with
     ping-pong; a square on the first echo drags its time. Then its times
     in ms or beats, linked or apart, and its values. -->
<script lang="ts">
  import {
    beatScale,
    curvedScale,
    effectSetting,
    hzScale,
    numberOf,
    percentScale,
    type MixSender,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfX, WIDTH, xOf } from "./mix-drawing";
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

  // How many echoes are drawn at most, and the quietest drawn.
  const MOST_ECHOES = 12;
  const QUIETEST = 0.04;

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let onBeat = $derived(effect.values.on_beat === true);
  let linked = $derived(effect.values.linked === true);
  let pingPong = $derived(effect.values.ping_pong === true);

  let leftWord = $derived(onBeat ? "left_beats" : "left_ms");
  let rightWord = $derived(onBeat ? "right_beats" : "right_ms");
  let timeScale = $derived(onBeat ? beatScale(16) : curvedScale(1, 8000, "ms"));

  let left = $derived(setting(leftWord, "Left", timeScale));
  let right = $derived(setting(rightWord, "Right", timeScale));
  let feedback = $derived(setting("feedback", "Feedback", percentScale(100)));
  let lowCut = $derived(setting("low_cut_hz", "Low cut", hzScale(20, 20000)));
  let highCut = $derived(setting("high_cut_hz", "High cut", hzScale(20, 20000)));
  let wobble = $derived(setting("wobble", "Wobble", percentScale(100)));
  let wobbleSpeed = $derived(setting("wobble_hz", "Wobble speed", curvedScale(0.1, 10, "Hz")));
  let grit = $derived(setting("grit", "Grit", percentScale(100)));
  let duck = $derived(setting("duck", "Duck", percentScale(100)));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));

  // The right side's time: the left's while linked.
  function rightTime(): number {
    if (linked) {
      return value(leftWord);
    }
    return value(rightWord);
  }

  // Each echo: when it comes, which side, and how loud. Without ping-pong
  // each side repeats on its own time; with it they take turns.
  let echoes = $derived.by(() => {
    const all: { time: number; top: boolean; level: number }[] = [];
    const share = value("feedback") / 100;
    const leftTime = value(leftWord);
    const rightOwn = rightTime();
    let level = 1;
    let time = 0;
    for (let echo = 0; echo < MOST_ECHOES && level >= QUIETEST; echo += 1) {
      if (pingPong) {
        const top = echo % 2 === 0;
        if (top) {
          time += leftTime;
        } else {
          time += rightOwn;
        }
        all.push({ time, top, level });
      } else {
        all.push({ time: leftTime * (echo + 1), top: true, level });
        all.push({ time: rightOwn * (echo + 1), top: false, level });
      }
      level *= share;
    }
    return all;
  });

  let span = $derived.by(() => {
    let latest = 0;
    for (const echo of echoes) {
      latest = Math.max(latest, echo.time);
    }
    return Math.max(latest * 1.08, 0.0001);
  });

  // Where an echo's bar is: left on top, right underneath, from the
  // middle out.
  const MIDDLE = HEIGHT / 2;
  const TALLEST = HEIGHT / 2 - EDGE - 4;

  function bar(echo: { time: number; top: boolean; level: number }): string {
    const x = xOf(echo.time, 0, span);
    const reach = TALLEST * echo.level;
    let end = MIDDLE + 4 + reach;
    let start = MIDDLE + 4;
    if (echo.top) {
      start = MIDDLE - 4;
      end = MIDDLE - 4 - reach;
    }
    return pathOf([[x, start], [x, end]]);
  }

  let squareX = $derived(xOf(value(leftWord), 0, span));
  let squareY = $derived(MIDDLE - 4 - TALLEST);

  // Dragging the first echo moves the left time, keeping the drawing's
  // span as it was when grabbed so the square follows the pointer.
  let grabbedSpan: number | null = null;

  function grabFirst() {
    grabbedSpan = span;
    onPick(pickedOf(left));
  }

  function moveFirst(x: number) {
    const along = valueOfX(x, 0, grabbedSpan ?? span);
    left.set(left.scale.at(left.scale.share(Math.max(along, 0.0001))));
  }

  function switchTile(word: string, name: string, on: boolean) {
    return { name, chosen: on, onClick: () => send.setEffectValue(place, slot, word, !on) };
  }

  let unitTiles = $derived([
    { name: "ms", chosen: !onBeat, onClick: () => send.setEffectValue(place, slot, "on_beat", false) },
    { name: "Beats", chosen: onBeat, onClick: () => send.setEffectValue(place, slot, "on_beat", true) },
  ]);
  let switchTiles = $derived([switchTile("linked", "Linked", linked), switchTile("ping_pong", "Ping-pong", pingPong)]);
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit" d={pathOf([[EDGE, MIDDLE - 4 - TALLEST], [EDGE, MIDDLE + 4 + TALLEST]])} />
        {#each echoes as echo, place (place)}
          <path class="sound-lit" d={bar(echo)} />
        {/each}
        <text class="sound-axis" x={WIDTH - EDGE - 8} y={EDGE + 8}>L</text>
        <text class="sound-axis" x={WIDTH - EDGE - 8} y={HEIGHT - EDGE}>R</text>
        <SoundHandle
          x={squareX}
          y={squareY}
          ringed={isPicked(picked, left.id)}
          onGrab={grabFirst}
          onMove={moveFirst}
        />
      </svg>
    </div>
    <div class="mix-tile-rows">
      <MixChoice tiles={unitTiles} />
      <MixChoice tiles={switchTiles} />
    </div>
  </div>
  <div class="mix-fields">
    <SoundValue setting={left} dots={9} {picked} {onPick} />
    <SoundValue setting={right} dots={9} {picked} {onPick} dim={linked} />
    <SoundValue setting={feedback} dots={9} {picked} {onPick} />
    <SoundValue setting={lowCut} dots={9} {picked} {onPick} />
    <SoundValue setting={highCut} dots={9} {picked} {onPick} />
    <SoundValue setting={wobble} dots={9} {picked} {onPick} />
    <SoundValue setting={wobbleSpeed} dots={9} {picked} {onPick} dim={value("wobble") === 0} />
    <SoundValue setting={grit} dots={9} {picked} {onPick} />
    <SoundValue setting={duck} dots={9} {picked} {onPick} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
  </div>
</div>
