<!-- The master's Out: its limiter, always last (docs/design.md#mix). It
     draws the last 4 s of peaks against the ceiling, with what it cut
     faint above them, and a square on the ceiling to drag; then its boost,
     ceiling and release, and the loudness and true peak leaving it. -->
<script lang="ts">
  import { curvedScale, decibelsOf, linearScale, type Limiter, type MixSender } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfY, WIDTH, xOf, yOf } from "./mix-drawing";
  import MixReduction from "./MixReduction.svelte";
  import { isPicked, pickedOf, type Picked, type Setting } from "./sound-scales";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    limiter: Limiter;
    // The master's peaks over the last 4 s, as levels, oldest first, and
    // how far the limiter turned each down, in dB.
    peaks: number[];
    cuts: number[];
    reduction: number;
    lufs: number;
    // The loudest true peak over the last few seconds, as a level.
    truePeak: number;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: MixSender;
  }

  let { limiter, peaks, cuts, reduction, lufs, truePeak, picked, onPick, send }: Props = $props();

  // The drawing reads from LOWEST_DB to 0 dB.
  const LOWEST_DB = -30;

  function setLimiter(change: Partial<Limiter>) {
    send.setLimiter({ ...limiter, ...change });
  }

  let boost: Setting = $derived({
    id: "mix master boost",
    name: "Boost",
    scale: linearScale(0, 24, 0.5, "dB"),
    now: () => limiter.boost_db,
    set: (boostDb) => setLimiter({ boost_db: boostDb }),
  });
  let ceiling: Setting = $derived({
    id: "mix master ceiling",
    name: "Ceiling",
    scale: linearScale(-12, 0, 0.1, "dB"),
    now: () => limiter.ceiling_db,
    set: (ceilingDb) => setLimiter({ ceiling_db: ceilingDb }),
  });
  let release: Setting = $derived({
    id: "mix master release",
    name: "Release",
    scale: curvedScale(1, 1000, "ms"),
    now: () => limiter.release_ms,
    set: (releaseMs) => setLimiter({ release_ms: releaseMs }),
  });

  function lineOf(levels: number[], addDb: number[] | null): string {
    const points: [number, number][] = [];
    const count = Math.max(levels.length, 2);
    for (let place = 0; place < levels.length; place += 1) {
      let db = decibelsOf(levels[place]);
      if (addDb !== null) {
        db += addDb[place] ?? 0;
      }
      points.push([xOf(place, 0, count - 1), yOf(db, LOWEST_DB, 0)]);
    }
    return pathOf(points);
  }

  let peaksLine = $derived(lineOf(peaks, null));
  let cutLine = $derived(lineOf(peaks, cuts));
  let ceilingY = $derived(yOf(limiter.ceiling_db, LOWEST_DB, 0));
  // The ceiling's words sit over its line, or under it near the top.
  let ceilingWordsY = $derived.by(() => {
    if (ceilingY < 24) {
      return ceilingY + 14;
    }
    return ceilingY - 6;
  });

  function moveCeiling(_x: number, y: number) {
    const db = valueOfY(y, LOWEST_DB, 0);
    const within = Math.max(-12, Math.min(0, db));
    ceiling.set(ceiling.scale.at(ceiling.scale.share(within)));
  }

  let lufsWords = $derived.by(() => {
    if (lufs <= -70) {
      return "-inf";
    }
    return lufs.toFixed(1);
  });

  let truePeakWords = $derived.by(() => {
    const db = decibelsOf(truePeak);
    if (db <= -120) {
      return "-inf";
    }
    return db.toFixed(1);
  });
</script>

<div class="mix-effect">
  <div class="mix-drawing-row">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <path class="sound-lit-soft is-faint" d={cutLine} />
        <path class="sound-lit is-fine" d={peaksLine} />
        <path class="sound-lit-soft" d={pathOf([[EDGE, ceilingY], [WIDTH - EDGE, ceilingY]])} />
        <text class="sound-axis" x={EDGE + 4} y={ceilingWordsY}>ceiling {ceiling.scale.words(limiter.ceiling_db)} dB</text>
        <text class="sound-axis" x={EDGE + 4} y={HEIGHT - 12}>last 4 s</text>
        <SoundHandle
          x={WIDTH - EDGE}
          y={ceilingY}
          ringed={isPicked(picked, ceiling.id)}
          onGrab={() => onPick(pickedOf(ceiling))}
          onMove={moveCeiling}
        />
      </svg>
    </div>
    <MixReduction db={reduction} most={12} />
  </div>
  <div class="mix-fields">
    <SoundValue setting={boost} dots={9} {picked} {onPick} />
    <SoundValue setting={ceiling} dots={9} {picked} {onPick} />
    <SoundValue setting={release} dots={9} {picked} {onPick} />
    <div class="mix-readout">
      <div class="sound-value-name">Loudness</div>
      <div class="sound-value-number">{lufsWords} <small>LUFS</small></div>
    </div>
    <div class="mix-readout">
      <div class="sound-value-name">True peak</div>
      <div class="sound-value-number">{truePeakWords} <small>dB</small></div>
    </div>
  </div>
</div>
