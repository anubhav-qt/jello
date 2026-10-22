<!-- The sound panel's Filter page (docs/design.md#sound-panel): the low-pass
     filter's cutoff and resonance, its envelope and key tracking. The
     cutoff and resonance move notes already sounding; the rest start with
     the next key pressed. -->
<script lang="ts">
  import {
    CUTOFF,
    evenScale,
    filterDb,
    isPicked,
    PERCENT,
    pickedOf,
    resonanceForDb,
    TIME,
    type Picked,
    type Setting,
  } from "./sound-scales";
  import {
    FILTER_LOWEST_HZ,
    FILTER_MOST_OCTAVES,
    FILTER_OPEN_HZ,
    type FilterEnvelope,
    type SoundSender,
    type SoundView,
  } from "./sound";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, send }: Props = $props();

  // The filter envelope as it is now, with `change` made to it.
  function sendEnvelope(change: Partial<FilterEnvelope>) {
    const filter = sound.filter;
    const envelope: FilterEnvelope = {
      amount: filter.amount,
      attackMs: filter.attack_ms,
      decayMs: filter.decay_ms,
      sustain: filter.sustain,
      releaseMs: filter.release_ms,
      ...change,
    };
    send.setFilterEnvelope(envelope);
  }

  const cutoff: Setting = {
    id: "cutoff",
    name: "Cutoff",
    scale: CUTOFF,
    now: () => sound.filter.cutoff_hz,
    set: (cutoffHz) => send.setFilter(cutoffHz, sound.filter.resonance),
  };
  const resonance: Setting = {
    id: "resonance",
    name: "Resonance",
    scale: PERCENT,
    now: () => sound.filter.resonance,
    set: (newResonance) => send.setFilter(sound.filter.cutoff_hz, newResonance),
  };
  const amount: Setting = {
    id: "filter amount",
    name: "Amount",
    scale: evenScale(0, FILTER_MOST_OCTAVES, 0.1, "oct", "oct"),
    now: () => sound.filter.amount,
    set: (newAmount) => sendEnvelope({ amount: newAmount }),
  };
  const attack: Setting = {
    id: "filter attack",
    name: "Attack",
    scale: TIME,
    now: () => sound.filter.attack_ms,
    set: (attackMs) => sendEnvelope({ attackMs }),
  };
  const decay: Setting = {
    id: "filter decay",
    name: "Decay",
    scale: TIME,
    now: () => sound.filter.decay_ms,
    set: (decayMs) => sendEnvelope({ decayMs }),
  };
  const sustain: Setting = {
    id: "filter sustain",
    name: "Sustain",
    scale: PERCENT,
    now: () => sound.filter.sustain,
    set: (newSustain) => sendEnvelope({ sustain: newSustain }),
  };
  const release: Setting = {
    id: "filter release",
    name: "Release",
    scale: TIME,
    now: () => sound.filter.release_ms,
    set: (releaseMs) => sendEnvelope({ releaseMs }),
  };
  const keyTracking: Setting = {
    id: "key tracking",
    name: "Key tracking",
    scale: PERCENT,
    now: () => sound.filter.key_tracking,
    set: (newKeyTracking) => send.setKeyTracking(newKeyTracking),
  };

  // The filter drawing, 286 by 140: how loud the filter leaves each
  // pitch, from 20 Hz on the left to 20 kHz on the right, three decades
  // apart. 0 dB is at ZERO_Y, each dB is 2 high, and the deepest cut is
  // drawn at LOWEST_Y.
  const LEFT_X = 10;
  const RIGHT_X = 276;
  const ZERO_Y = 50;
  const LOWEST_Y = 120;
  const DECADES = 3;
  const POINTS = 200;

  function xOfHz(hz: number): number {
    const decades = Math.log10(hz / FILTER_LOWEST_HZ);
    return LEFT_X + (decades / DECADES) * (RIGHT_X - LEFT_X);
  }

  function hzAtX(x: number): number {
    const decades = ((x - LEFT_X) / (RIGHT_X - LEFT_X)) * DECADES;
    return FILTER_LOWEST_HZ * Math.pow(10, decades);
  }

  function yOfDb(db: number): number {
    return Math.min(LOWEST_Y, ZERO_Y - 2 * db);
  }

  // The filter's curve with its cutoff at `cutoffHz`.
  function curve(cutoffHz: number): string {
    const points: string[] = [];
    for (let point = 0; point <= POINTS; point += 1) {
      const x = LEFT_X + ((RIGHT_X - LEFT_X) * point) / POINTS;
      const db = filterDb(hzAtX(x), cutoffHz, sound.filter.resonance);
      points.push(`${x.toFixed(1)} ${yOfDb(db).toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  }

  let cutoffCurve = $derived(curve(sound.filter.cutoff_hz));

  // Where the envelope's peak takes the cutoff, or null when it takes it
  // nowhere: no amount, or the filter is open.
  let peakHz = $derived.by(() => {
    if (sound.filter.amount === 0) {
      return null;
    }
    if (sound.filter.cutoff_hz >= FILTER_OPEN_HZ) {
      return null;
    }
    const hz = sound.filter.cutoff_hz * Math.pow(2, sound.filter.amount);
    return Math.min(hz, FILTER_OPEN_HZ - 1);
  });

  let peakCurve = $derived.by(() => {
    if (peakHz === null) {
      return "";
    }
    return curve(peakHz);
  });

  // ENVELOPE PEAK sits over its curve's bump, kept inside the drawing.
  let peakWordsX = $derived.by(() => {
    if (peakHz === null) {
      return 0;
    }
    const x = xOfHz(peakHz);
    if (x < LEFT_X + 40) {
      return LEFT_X + 40;
    }
    if (x > RIGHT_X - 40) {
      return RIGHT_X - 40;
    }
    return x;
  });

  // The square sits on the curve at the cutoff, on its bump.
  let cutoffX = $derived(xOfHz(sound.filter.cutoff_hz));
  let cutoffY = $derived.by(() => {
    const cutoffHz = sound.filter.cutoff_hz;
    const db = filterDb(cutoffHz, cutoffHz, sound.filter.resonance);
    return yOfDb(db);
  });

  // Sideways it moves the cutoff, and up and down the resonance's bump.
  function moveCutoff(x: number, y: number) {
    // Rounded and kept within its range, as its dots do.
    const share = CUTOFF.share(hzAtX(x));
    const cutoffHz = CUTOFF.at(share);
    const bumpDb = (ZERO_Y - y) / 2;
    send.setFilter(cutoffHz, resonanceForDb(bumpDb));
  }

  // The envelope drawing, 356 by 140, drawn like the loudness envelope,
  // each stage up to STAGE_WIDTH on the time curve.
  const START_X = 14;
  const KEY_UP_X = 220;
  const BOTTOM_Y = 128;
  const TOP_Y = 14;
  const STAGE_WIDTH = 100;

  function stageWidth(ms: number): number {
    return TIME.share(ms) * STAGE_WIDTH;
  }

  function stageMs(width: number): number {
    return TIME.at(width / STAGE_WIDTH);
  }

  function sustainY(percent: number): number {
    return BOTTOM_Y - (percent / 100) * (BOTTOM_Y - TOP_Y);
  }

  function sustainAt(y: number): number {
    const share = (BOTTOM_Y - y) / (BOTTOM_Y - TOP_Y);
    return PERCENT.at(share);
  }

  let peakX = $derived(START_X + stageWidth(sound.filter.attack_ms));
  let decayX = $derived(peakX + stageWidth(sound.filter.decay_ms));
  let holdY = $derived(sustainY(sound.filter.sustain));
  let releaseX = $derived(KEY_UP_X + stageWidth(sound.filter.release_ms));
  let envelopePath = $derived(
    `M${START_X} ${BOTTOM_Y} L ${peakX} ${TOP_Y} L ${decayX} ${holdY} L ${KEY_UP_X} ${holdY} L ${releaseX} ${BOTTOM_Y}`,
  );

  // How many octaves `octaves` is, in words, like "4 OCTAVES" or "1 OCTAVE".
  function octaveWords(octaves: number): string {
    const rounded = Number(octaves.toFixed(1));
    if (rounded === 1) {
      return "1 OCTAVE";
    }
    return `${rounded} OCTAVES`;
  }

  let peakWords = $derived(`+${octaveWords(sound.filter.amount)}`);
  let holdWords = $derived(octaveWords((sound.filter.amount * sound.filter.sustain) / 100));

  function moveDecayCorner(x: number, y: number) {
    sendEnvelope({ decayMs: stageMs(x - peakX), sustain: sustainAt(y) });
  }

  // With no amount, the envelope moves nothing.
  let noAmount = $derived(sound.filter.amount === 0);

  // The key tracking drawing, 213 by 140: low notes on the left, high on
  // the right, and how far each note's cutoff is from middle C's. At 100%
  // it follows the note all the way; at 0% every note has middle C's.
  const TRACK_MIDDLE_Y = 70;
  const TRACK_SWING = 56;

  let trackY = $derived((sound.filter.key_tracking / 100) * TRACK_SWING);

  function moveKeyTracking(_x: number, y: number) {
    const share = (TRACK_MIDDLE_Y - y) / TRACK_SWING;
    keyTracking.set(PERCENT.at(share));
  }
</script>

<div class="sound-modules" style:grid-template-columns="330fr 400fr 257fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Filter</span>
      <span class="sound-hint">takes the top off the sound</span>
    </div>
    <div class="sound-drawing">
      <svg viewBox="0 0 286 140" preserveAspectRatio="xMidYMid meet">
        {#if peakHz !== null}
          <path class="sound-lit-soft" d={peakCurve} />
          <text class="sound-axis" text-anchor="middle" x={peakWordsX} y="24">ENVELOPE PEAK</text>
        {/if}
        <path class="sound-lit" d={cutoffCurve} />
        <SoundHandle
          x={cutoffX}
          y={cutoffY}
          solid
          ringed={isPicked(picked, cutoff.id)}
          onGrab={() => onPick(pickedOf(cutoff))}
          onMove={moveCutoff}
        />
        <text class="sound-axis" x={LEFT_X} y="134">20 HZ</text>
        <text class="sound-axis" text-anchor="end" x={RIGHT_X} y="134">20 KHZ</text>
      </svg>
    </div>
    <div class="sound-fields" style:grid-template-columns="1fr 1fr">
      <SoundValue setting={cutoff} dots={17} {picked} {onPick} />
      <SoundValue setting={resonance} dots={17} {picked} {onPick} />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Filter envelope</span>
      <span class="sound-hint">opens it as each note starts</span>
    </div>
    <div class="sound-drawing">
      <svg viewBox="0 0 356 140" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M{KEY_UP_X} 10 L {KEY_UP_X} 130" />
        <text class="sound-axis" x={KEY_UP_X + 5} y="20">KEY UP</text>
        <text class="sound-axis" x={peakX + 10} y="20">{peakWords}</text>
        <text class="sound-axis" text-anchor="end" x={KEY_UP_X - 6} y={holdY - 6}>{holdWords}</text>
        <path class="sound-lit" d={envelopePath} />
        <SoundHandle
          x={peakX}
          y={TOP_Y}
          solid
          ringed={isPicked(picked, attack.id)}
          onGrab={() => onPick(pickedOf(attack))}
          onMove={(x) => attack.set(stageMs(x - START_X))}
        />
        <SoundHandle
          x={decayX}
          y={holdY}
          ringed={isPicked(picked, decay.id)}
          onGrab={() => onPick(pickedOf(decay))}
          onMove={moveDecayCorner}
        />
        <SoundHandle
          x={KEY_UP_X}
          y={holdY}
          ringed={isPicked(picked, sustain.id)}
          onGrab={() => onPick(pickedOf(sustain))}
          onMove={(_x, y) => sustain.set(sustainAt(y))}
        />
        <SoundHandle
          x={releaseX}
          y={BOTTOM_Y}
          ringed={isPicked(picked, release.id)}
          onGrab={() => onPick(pickedOf(release))}
          onMove={(x) => release.set(stageMs(x - KEY_UP_X))}
        />
      </svg>
    </div>
    <div class="sound-fields" style:grid-template-columns="repeat(5, 1fr)">
      <SoundValue setting={amount} dots={8} {picked} {onPick} />
      <SoundValue setting={attack} dots={8} {picked} {onPick} dim={noAmount} />
      <SoundValue setting={decay} dots={8} {picked} {onPick} dim={noAmount} />
      <SoundValue setting={sustain} dots={8} {picked} {onPick} dim={noAmount} />
      <SoundValue setting={release} dots={8} {picked} {onPick} dim={noAmount} />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Key tracking</span>
      <span class="sound-hint">follows the note</span>
    </div>
    <div class="sound-drawing">
      <svg viewBox="0 0 213 140" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M14 126 L 199 14" />
        <path class="sound-lit-soft" d="M14 70 L 199 70" />
        <text class="sound-axis" text-anchor="end" x="170" y="18">100%</text>
        <text class="sound-axis" text-anchor="end" x="199" y="84">0%</text>
        <path class="sound-lit" d="M14 {TRACK_MIDDLE_Y + trackY} L 199 {TRACK_MIDDLE_Y - trackY}" />
        <circle class="sound-middle-c" cx="106.5" cy={TRACK_MIDDLE_Y} r="3.5" />
        <text class="sound-axis" x="20" y="64">MIDDLE C</text>
        <SoundHandle
          x={199}
          y={TRACK_MIDDLE_Y - trackY}
          solid
          ringed={isPicked(picked, keyTracking.id)}
          onGrab={() => onPick(pickedOf(keyTracking))}
          onMove={moveKeyTracking}
        />
        <text class="sound-axis" x="14" y="134">LOW NOTES</text>
        <text class="sound-axis" text-anchor="end" x="199" y="134">HIGH</text>
      </svg>
    </div>
    <div class="sound-fields" style:grid-template-columns="1fr">
      <SoundValue setting={keyTracking} dots={30} {picked} {onPick} />
    </div>
  </div>
</div>
