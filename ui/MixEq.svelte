<!-- The EQ's page (docs/design.md#mix): its curve from 20 Hz to 20 kHz with
     a numbered square per band that is on, tiles 1 to 8 under it to pick a
     band (dim while off), then the picked band's shape, frequency, gain and
     width (a cut's slope), and the output. Picking "Off" switches the band
     off; picking a shape switches it on. -->
<script lang="ts">
  import {
    effectSetting,
    hzScale,
    linearScale,
    numberOf,
    stepsScale,
    type MixSender,
    type SlotView,
  } from "./mix";
  import { curveHz, eqBandDb, EDGE, HEIGHT, hzOfX, pathOf, valueOfY, WIDTH, xOfHz, yOf } from "./mix-drawing";
  import { clickThenLetGo } from "./dom";
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

  const BANDS = 8;
  // The drawing reads from -MOST_DB to MOST_DB.
  const MOST_DB = 24;
  const SHAPES = [
    { word: "low-cut", name: "Low cut" },
    { word: "low-shelf", name: "Low shelf" },
    { word: "bell", name: "Bell" },
    { word: "high-shelf", name: "High shelf" },
    { word: "high-cut", name: "High cut" },
    { word: "notch", name: "Notch" },
  ];

  // The band picked, from 1.
  let band = $state(1);

  function word(of: number, name: string): string {
    return `band_${of}_${name}`;
  }

  function isOn(of: number): boolean {
    return effect.values[word(of, "on")] === true;
  }

  function shapeOf(of: number): string {
    return String(effect.values[word(of, "shape")]);
  }

  function hasGain(shape: string): boolean {
    return shape === "bell" || shape === "low-shelf" || shape === "high-shelf";
  }

  function isCut(shape: string): boolean {
    return shape === "low-cut" || shape === "high-cut";
  }

  function setting(name: string, label: string, scale: Setting["scale"]): Setting {
    const valueWord = word(band, name);
    return effectSetting(place, slot, valueWord, label, scale, () => numberOf(effect.values[valueWord]), send);
  }

  let frequency = $derived(setting("frequency_hz", "Frequency", hzScale(20, 20000)));
  let gain = $derived(setting("gain_db", "Gain", linearScale(-24, 24, 0.5, "dB", true)));
  let width = $derived(setting("width", "Width", linearScale(0.1, 18, 0.1, "Q")));
  let slope = $derived(setting("slope", "Slope", stepsScale([12, 24, 48], String, () => "dB")));
  let output: Setting = $derived(
    effectSetting(
      place,
      slot,
      "output_db",
      "Output",
      linearScale(-24, 24, 0.5, "dB", true),
      () => numberOf(effect.values.output_db),
      send,
    ),
  );

  let bandsOn = $derived.by(() => {
    let count = 0;
    for (let of = 1; of <= BANDS; of += 1) {
      if (isOn(of)) {
        count += 1;
      }
    }
    return count;
  });

  function bandDb(of: number, hz: number): number {
    return eqBandDb(
      shapeOf(of),
      hz,
      numberOf(effect.values[word(of, "frequency_hz")], 1000),
      numberOf(effect.values[word(of, "gain_db")]),
      numberOf(effect.values[word(of, "width")], 1),
      numberOf(effect.values[word(of, "slope")], 24),
    );
  }

  let curve = $derived.by(() => {
    const points: [number, number][] = [];
    const output = numberOf(effect.values.output_db);
    for (const hz of curveHz(120)) {
      let db = output;
      for (let of = 1; of <= BANDS; of += 1) {
        if (isOn(of)) {
          db += bandDb(of, hz);
        }
      }
      points.push([xOfHz(hz), yOf(db, -MOST_DB, MOST_DB)]);
    }
    return pathOf(points);
  });

  // Each band that is on, as a square: at its frequency, and at its gain
  // for a bell or a shelf.
  let squares = $derived.by(() => {
    const all: { band: number; x: number; y: number }[] = [];
    for (let of = 1; of <= BANDS; of += 1) {
      if (!isOn(of)) {
        continue;
      }
      let db = 0;
      if (hasGain(shapeOf(of))) {
        db = numberOf(effect.values[word(of, "gain_db")]);
      }
      const x = xOfHz(numberOf(effect.values[word(of, "frequency_hz")], 1000));
      all.push({ band: of, x, y: yOf(db, -MOST_DB, MOST_DB) });
    }
    return all;
  });

  function pickBand(of: number) {
    band = of;
  }

  function grabBand(of: number) {
    band = of;
    onPick(pickedOf(setting("frequency_hz", "Frequency", hzScale(20, 20000))));
  }

  // Dragging a square moves its band's frequency, and its gain for a bell
  // or a shelf.
  function moveBand(of: number, x: number, y: number) {
    send.setEffectValue(place, slot, word(of, "frequency_hz"), hzOfX(x));
    if (hasGain(shapeOf(of))) {
      const db = valueOfY(y, -MOST_DB, MOST_DB);
      send.setEffectValue(place, slot, word(of, "gain_db"), Math.round(db * 2) / 2);
    }
  }

  function chooseShape(shape: string | null) {
    if (shape === null) {
      send.setEffectValue(place, slot, word(band, "on"), false);
      return;
    }
    send.setEffectValue(place, slot, word(band, "shape"), shape);
    if (!isOn(band)) {
      send.setEffectValue(place, slot, word(band, "on"), true);
    }
  }

  let shapeTiles = $derived.by(() => {
    const on = isOn(band);
    const shape = shapeOf(band);
    const tiles = [{ name: "Off", chosen: !on, onClick: () => chooseShape(null) }];
    for (const each of SHAPES) {
      tiles.push({ name: each.name, chosen: on && shape === each.word, onClick: () => chooseShape(each.word) });
    }
    return tiles;
  });

  const AXIS = [
    { hz: 100, words: "100" },
    { hz: 1000, words: "1k" },
    { hz: 10000, words: "10k" },
  ];
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit-soft is-faint" d={pathOf([[EDGE, HEIGHT / 2], [WIDTH - EDGE, HEIGHT / 2]])} />
        {#each AXIS as mark (mark.hz)}
          <text class="sound-axis" x={xOfHz(mark.hz) - 6} y={HEIGHT - 6}>{mark.words}</text>
        {/each}
        <path class="sound-lit is-fine" d={curve} />
        {#each squares as square (square.band)}
          <SoundHandle
            x={square.x}
            y={square.y}
            solid={square.band === band}
            ringed={square.band === band && isPicked(picked, frequency.id)}
            onGrab={() => grabBand(square.band)}
            onMove={(x, y) => moveBand(square.band, x, y)}
          />
          <text class="sound-axis mix-band-number" x={square.x - 3} y={square.y - 8}>{square.band}</text>
        {/each}
      </svg>
    </div>
    <div class="mix-band-tiles">
      {#each { length: BANDS } as _, place (place)}
        <button
          type="button"
          class="mix-tile"
          class:is-chosen={band === place + 1}
          class:is-dim={!isOn(place + 1)}
          onclick={(event) => clickThenLetGo(event, () => pickBand(place + 1))}
        >
          {place + 1}
        </button>
      {/each}
    </div>
  </div>
  <div class="mix-fields">
    <div class="mix-wide">
      <MixChoice tiles={shapeTiles} label={`Band ${band}, ${bandsOn} of 8 on`} />
    </div>
    <SoundValue setting={frequency} dots={9} {picked} {onPick} dim={!isOn(band)} />
    {#if isCut(shapeOf(band))}
      <SoundValue setting={slope} dots={9} {picked} {onPick} dim={!isOn(band)} />
    {:else}
      <SoundValue setting={gain} dots={9} {picked} {onPick} dim={!isOn(band) || !hasGain(shapeOf(band))} />
      <SoundValue setting={width} dots={9} {picked} {onPick} dim={!isOn(band)} />
    {/if}
    <SoundValue setting={output} dots={9} {picked} {onPick} />
  </div>
</div>
