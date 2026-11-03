<!-- Saturation's page (docs/design.md#mix): input against output, bending
     harder as drive goes up, with a square on the bend to drag; then its
     curve, drive, tone and mix, and its output: matched to the input's
     loudness, or set. -->
<script lang="ts">
  import {
    effectSetting,
    linearScale,
    numberOf,
    percentScale,
    SIGNED_SHARE,
    type MixSender,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, saturated, valueOfY, WIDTH, xOf, yOf } from "./mix-drawing";
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

  const CURVES = [
    { word: "tape", name: "Tape" },
    { word: "tube", name: "Tube" },
    { word: "hard", name: "Hard clip" },
    { word: "fold", name: "Fold" },
  ];
  // Where along the input the square sits.
  const SQUARE_INPUT = 0.5;
  const MOST_DRIVE_DB = 36;

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  function setting(word: string, name: string, scale: Setting["scale"]): Setting {
    return effectSetting(place, slot, word, name, scale, () => value(word), send);
  }

  let drive = $derived(setting("drive_db", "Drive", linearScale(0, MOST_DRIVE_DB, 0.5, "dB")));
  let tone = $derived(setting("tone", "Tone", SIGNED_SHARE));
  let output = $derived(setting("output_db", "Set level", linearScale(-24, 24, 0.5, "dB", true)));
  let mix = $derived(setting("mix", "Mix", percentScale(100)));

  let curveWord = $derived(String(effect.values.curve));
  let matched = $derived(effect.values.match === true);

  let curve = $derived.by(() => {
    const points: [number, number][] = [];
    const steps = 120;
    for (let place = 0; place <= steps; place += 1) {
      const input = -1 + (2 * place) / steps;
      const out = saturated(curveWord, value("drive_db"), input);
      points.push([xOf(input, -1, 1), yOf(out, -1, 1)]);
    }
    return pathOf(points);
  });

  let squareX = $derived(xOf(SQUARE_INPUT, -1, 1));
  let squareY = $derived(yOf(saturated(curveWord, value("drive_db"), SQUARE_INPUT), -1, 1));

  // Dragging the square up drives harder: the drive that bends the curve
  // through the pointer, read off the tape curve.
  function moveSquare(_x: number, y: number) {
    const out = Math.max(SQUARE_INPUT, Math.min(0.999, valueOfY(y, -1, 1)));
    const gain = Math.atanh(out) / SQUARE_INPUT;
    const db = Math.max(0, Math.min(MOST_DRIVE_DB, 20 * Math.log10(gain)));
    drive.set(drive.scale.at(drive.scale.share(db)));
  }

  let curveTiles = $derived(
    CURVES.map((each) => ({
      name: each.name,
      chosen: curveWord === each.word,
      onClick: () => send.setEffectValue(place, slot, "curve", each.word),
    })),
  );

  let outputTiles = $derived([
    {
      name: "Matched",
      chosen: matched,
      title: "Keeps the input's loudness",
      onClick: () => send.setEffectValue(place, slot, "match", true),
    },
    {
      name: "Set",
      chosen: !matched,
      title: "Turned by the output below",
      onClick: () => send.setEffectValue(place, slot, "match", false),
    },
  ]);
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit-soft is-faint" d={pathOf([[EDGE, HEIGHT - EDGE], [WIDTH - EDGE, EDGE]])} />
        <path class="sound-lit is-fine" d={curve} />
        <SoundHandle
          x={squareX}
          y={squareY}
          ringed={isPicked(picked, drive.id)}
          onGrab={() => onPick(pickedOf(drive))}
          onMove={moveSquare}
        />
      </svg>
    </div>
    <p class="mix-note">Input across, output up</p>
  </div>
  <div class="mix-fields">
    <div class="mix-wide">
      <MixChoice tiles={curveTiles} />
    </div>
    <SoundValue setting={drive} dots={9} {picked} {onPick} />
    <SoundValue setting={tone} dots={9} {picked} {onPick} />
    <SoundValue setting={mix} dots={9} {picked} {onPick} />
    <MixChoice tiles={outputTiles} label="Output" />
    <SoundValue setting={output} dots={9} {picked} {onPick} dim={matched} />
  </div>
</div>
