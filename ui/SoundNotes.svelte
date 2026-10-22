<!-- The sound panel's Notes page (docs/design.md#sound-panel): the wave, the
     loudness envelope, unison, the tuning and the vibrato key's times. All
     of them start with the next key pressed. -->
<script lang="ts">
  import {
    evenScale,
    isPicked,
    LOUDNESS_TIME,
    PERCENT,
    pickedOf,
    TIME,
    type Picked,
    type Setting,
  } from "./sound-scales";
  import {
    TUNING_HIGHEST_HZ,
    TUNING_LOWEST_HZ,
    UNISON_MOST_COPIES,
    UNISON_MOST_DETUNE_CENTS,
    type LoudnessEnvelope,
    type SoundSender,
    type SoundView,
    type Unison,
  } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, send }: Props = $props();

  // The four waves, each with a small drawing of it, 44 by 22.
  const WAVES = [
    { name: "sine", glyph: "M2 11 C 7 -3, 12 -3, 17 11 S 27 25, 32 11 S 37 2, 42 4" },
    { name: "triangle", glyph: "M2 11 L 9 2 L 22 20 L 35 2 L 42 11" },
    { name: "square", glyph: "M2 20 L 2 2 L 22 2 L 22 20 L 42 20 L 42 2" },
    { name: "saw", glyph: "M2 20 L 22 2 L 22 20 L 42 2 L 42 20" },
  ];

  // The loudness envelope as it is now, with `change` made to it.
  function sendLoudness(change: Partial<LoudnessEnvelope>) {
    const loudness = sound.loudness;
    const envelope: LoudnessEnvelope = {
      attackMs: loudness.attack_ms,
      decayMs: loudness.decay_ms,
      sustain: loudness.sustain,
      releaseMs: loudness.release_ms,
      ...change,
    };
    send.setLoudnessEnvelope(envelope);
  }

  function sendUnison(change: Partial<Unison>) {
    const unison = sound.unison;
    const newUnison: Unison = {
      copies: unison.copies,
      detuneCents: unison.detune_cents,
      spread: unison.spread,
      ...change,
    };
    send.setUnison(newUnison);
  }

  const attack: Setting = {
    id: "loudness attack",
    name: "Attack",
    scale: LOUDNESS_TIME,
    now: () => sound.loudness.attack_ms,
    set: (attackMs) => sendLoudness({ attackMs }),
  };
  const decay: Setting = {
    id: "loudness decay",
    name: "Decay",
    scale: LOUDNESS_TIME,
    now: () => sound.loudness.decay_ms,
    set: (decayMs) => sendLoudness({ decayMs }),
  };
  const sustain: Setting = {
    id: "loudness sustain",
    name: "Sustain",
    scale: PERCENT,
    now: () => sound.loudness.sustain,
    set: (newSustain) => sendLoudness({ sustain: newSustain }),
  };
  const release: Setting = {
    id: "loudness release",
    name: "Release",
    scale: LOUDNESS_TIME,
    now: () => sound.loudness.release_ms,
    set: (releaseMs) => sendLoudness({ releaseMs }),
  };
  const copies: Setting = {
    id: "copies",
    name: "Copies",
    scale: evenScale(1, UNISON_MOST_COPIES, 1, "", ""),
    now: () => sound.unison.copies,
    set: (newCopies) => sendUnison({ copies: newCopies }),
  };
  const detune: Setting = {
    id: "detune",
    name: "Detune",
    scale: evenScale(0, UNISON_MOST_DETUNE_CENTS, 1, "cent", "cents"),
    now: () => sound.unison.detune_cents,
    set: (detuneCents) => sendUnison({ detuneCents }),
  };
  const spread: Setting = {
    id: "spread",
    name: "Spread",
    scale: PERCENT,
    now: () => sound.unison.spread,
    set: (newSpread) => sendUnison({ spread: newSpread }),
  };
  const tuning: Setting = {
    id: "tuning",
    name: "Tuning",
    scale: evenScale(TUNING_LOWEST_HZ, TUNING_HIGHEST_HZ, 1, "Hz", "Hz"),
    now: () => sound.tuning_hz,
    set: (a4Hz) => send.setTuning(a4Hz),
  };
  const rise: Setting = {
    id: "vibrato rise",
    name: "Rise",
    scale: TIME,
    now: () => sound.vibrato_key.rise_ms,
    set: (riseMs) => send.setVibratoKeyTimes(riseMs, sound.vibrato_key.fall_ms),
  };
  const fall: Setting = {
    id: "vibrato fall",
    name: "Fall",
    scale: TIME,
    now: () => sound.vibrato_key.fall_ms,
    set: (fallMs) => send.setVibratoKeyTimes(sound.vibrato_key.rise_ms, fallMs),
  };

  // The loudness drawing, 303 by 140: the note starts at the bottom left,
  // rises to the top, falls to the sustain and holds it until KEY UP, then
  // fades out. Each stage takes up to STAGE_WIDTH, on the time curve.
  const START_X = 14;
  const KEY_UP_X = 200;
  const BOTTOM_Y = 128;
  const TOP_Y = 14;
  const STAGE_WIDTH = 90;

  function stageWidth(ms: number): number {
    return LOUDNESS_TIME.share(ms) * STAGE_WIDTH;
  }

  // The time a stage takes when it is `width` wide.
  function stageMs(width: number): number {
    return LOUDNESS_TIME.at(width / STAGE_WIDTH);
  }

  function sustainY(percent: number): number {
    return BOTTOM_Y - (percent / 100) * (BOTTOM_Y - TOP_Y);
  }

  // The sustain at `y`, in percent.
  function sustainAt(y: number): number {
    const share = (BOTTOM_Y - y) / (BOTTOM_Y - TOP_Y);
    return PERCENT.at(share);
  }

  let peakX = $derived(START_X + stageWidth(sound.loudness.attack_ms));
  let decayX = $derived(peakX + stageWidth(sound.loudness.decay_ms));
  let holdY = $derived(sustainY(sound.loudness.sustain));
  let releaseX = $derived(KEY_UP_X + stageWidth(sound.loudness.release_ms));
  let envelopePath = $derived(
    `M${START_X} ${BOTTOM_Y} L ${peakX} ${TOP_Y} L ${decayX} ${holdY} L ${KEY_UP_X} ${holdY} L ${releaseX} ${BOTTOM_Y}`,
  );

  // The corner after the peak moves the decay sideways and the sustain up
  // and down, at once.
  function moveDecayCorner(x: number, y: number) {
    sendLoudness({ decayMs: stageMs(x - peakX), sustain: sustainAt(y) });
  }

  // The unison picture, 213 by 140: the copies spread from left to right
  // between the speakers, and detuned from lower to higher.
  const MIDDLE_X = 106.5;
  const MIDDLE_Y = 70;
  const WIDEST = 92;
  const HIGHEST = 58;

  // The middle copy is drawn bigger than the others.
  const COPY_SIZE = 4.5;
  const MIDDLE_COPY_SIZE = 5.5;

  let copyDots = $derived.by(() => {
    const count = sound.unison.copies;
    const dots: { x: number; y: number; size: number; middle: boolean }[] = [];
    for (let copy = 0; copy < count; copy += 1) {
      // From -1, the lowest copy on the left, to 1.
      let along = 0;
      if (count > 1) {
        along = (copy / (count - 1)) * 2 - 1;
      }
      const x = MIDDLE_X + along * WIDEST * (sound.unison.spread / 100);
      const y = MIDDLE_Y - along * (sound.unison.detune_cents / UNISON_MOST_DETUNE_CENTS) * HIGHEST;
      const middle = along === 0;
      let size = COPY_SIZE;
      if (middle) {
        size = MIDDLE_COPY_SIZE;
      }
      dots.push({ x, y, size, middle });
    }
    return dots;
  });

  // With one copy, detune and spread do nothing.
  let oneCopy = $derived(sound.unison.copies === 1);
</script>

<div class="sound-modules" style:grid-template-columns="186fr 341fr 251fr 209fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Wave</span>
      <span class="sound-key-chip">\</span>
    </div>
    <div class="sound-waves">
      {#each WAVES as wave (wave.name)}
        <button
          type="button"
          class="sound-wave"
          class:is-chosen={sound.wave === wave.name}
          onclick={(event) => clickThenLetGo(event, () => send.setWave(wave.name))}
        >
          <svg viewBox="0 0 44 22"><path class="sound-lit" d={wave.glyph} /></svg>
          {wave.name}
        </button>
      {/each}
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Loudness</span>
      <span class="sound-hint">how each note rises and fades</span>
    </div>
    <div class="sound-drawing">
      <svg viewBox="0 0 303 140" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M{KEY_UP_X} 10 L {KEY_UP_X} 130" />
        <text class="sound-axis" x={KEY_UP_X + 5} y="20">KEY UP</text>
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
          solid
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
    <div class="sound-fields" style:grid-template-columns="repeat(4, 1fr)">
      <SoundValue setting={attack} dots={9} {picked} {onPick} />
      <SoundValue setting={decay} dots={9} {picked} {onPick} />
      <SoundValue setting={sustain} dots={9} {picked} {onPick} />
      <SoundValue setting={release} dots={9} {picked} {onPick} />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Unison</span>
      <span class="sound-hint">copies of each note</span>
    </div>
    <div class="sound-drawing">
      <svg viewBox="0 0 213 140" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M106.5 12 L 106.5 128" />
        <path class="sound-lit-soft" d="M14 70 L 199 70" />
        <text class="sound-axis" x="6" y="132">L</text>
        <text class="sound-axis" x="200" y="132">R</text>
        <text class="sound-axis" x="112" y="20">HIGHER</text>
        <text class="sound-axis" x="112" y="128">LOWER</text>
        {#each copyDots as dot}
          <circle class="sound-copy" class:is-middle={dot.middle} cx={dot.x} cy={dot.y} r={dot.size} />
        {/each}
      </svg>
    </div>
    <div class="sound-fields" style:grid-template-columns="repeat(3, 1fr)">
      <SoundValue setting={copies} dots={UNISON_MOST_COPIES} {picked} {onPick} />
      <SoundValue setting={detune} dots={8} {picked} {onPick} dim={oneCopy} />
      <SoundValue setting={spread} dots={8} {picked} {onPick} dim={oneCopy} />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Tuning</span>
      <span class="sound-hint">A4's pitch</span>
    </div>
    <SoundValue setting={tuning} dots={23} {picked} {onPick} big />

    <div class="sound-module-top is-second">
      <span class="sound-label">Vibrato key</span>
      <span class="sound-hint">hold left Shift</span>
    </div>
    <div class="sound-fields is-under-top" style:grid-template-columns="1fr 1fr">
      <SoundValue setting={rise} dots={11} {picked} {onPick} />
      <SoundValue setting={fall} dots={11} {picked} {onPick} />
    </div>
  </div>
</div>
