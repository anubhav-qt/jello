<!-- The sound panel's Oscillators page (docs/design.md#sound-panel): the
     three oscillators every note plays, side by side
     (docs/adr/0035-oscillators-with-kinds.md). Each has a drawing of what it
     plays, its five kind tiles, and its level and tuning. All of them start
     with the next key pressed. -->
<script lang="ts">
  import { isPicked, KINDS, PERCENT, pickedOf, signedScale, type Picked, type Setting } from "./sound-scales";
  import {
    OSCILLATOR_MOST_CENTS,
    OSCILLATOR_MOST_HALF_STEPS,
    OSCILLATOR_MOST_OCTAVES,
    type Oscillator,
    type OscillatorView,
    type SoundSender,
    type SoundView,
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

  const OCTAVES = signedScale(OSCILLATOR_MOST_OCTAVES, "octave", "octaves");
  const HALF_STEPS = signedScale(OSCILLATOR_MOST_HALF_STEPS, "half step", "half steps");
  const CENTS = signedScale(OSCILLATOR_MOST_CENTS, "cent", "cents");

  // Oscillator `index` as it is now, with `change` made to it.
  function sendOscillator(index: number, change: Partial<Oscillator>) {
    const now = sound.oscillators[index];
    const oscillator: Oscillator = {
      kind: now.kind,
      level: now.level,
      octaves: now.octaves,
      halfSteps: now.half_steps,
      cents: now.cents,
      ...change,
    };
    send.setOscillator(index, oscillator);
  }

  // The four values of oscillator `index`, each picked by its own id, like
  // "osc 2 level".
  function settingsOf(index: number) {
    const prefix = `osc ${index + 1}`;
    const level: Setting = {
      id: `${prefix} level`,
      name: "Level",
      scale: PERCENT,
      now: () => sound.oscillators[index].level,
      set: (newLevel) => sendOscillator(index, { level: newLevel }),
    };
    const octaves: Setting = {
      id: `${prefix} octaves`,
      name: "Octaves",
      scale: OCTAVES,
      now: () => sound.oscillators[index].octaves,
      set: (newOctaves) => sendOscillator(index, { octaves: newOctaves }),
    };
    const halfSteps: Setting = {
      id: `${prefix} half steps`,
      name: "Half steps",
      scale: HALF_STEPS,
      now: () => sound.oscillators[index].half_steps,
      set: (newHalfSteps) => sendOscillator(index, { halfSteps: newHalfSteps }),
    };
    const cents: Setting = {
      id: `${prefix} cents`,
      name: "Cents",
      scale: CENTS,
      now: () => sound.oscillators[index].cents,
      set: (newCents) => sendOscillator(index, { cents: newCents }),
    };
    return { level, octaves, halfSteps, cents };
  }

  const SETTINGS = [settingsOf(0), settingsOf(1), settingsOf(2)];

  // The drawing, 280 by 40, as wide for its height as the box it fills:
  // the oscillator's wave around a middle line, as tall as its level. Right
  // on the note it shows 2 cycles, and each octave up twice as many, so the
  // tuning shows too. Its square, at the right end, drags the level up and
  // down.
  const WIDTH = 280;
  const HEIGHT = 40;
  const MIDDLE_Y = 20;
  const TALLEST = 14;
  const LEFT = 8;
  const RIGHT = 262;
  const POINTS = 240;
  const CYCLES_ON_THE_NOTE = 2;
  const FEWEST_CYCLES = 0.5;
  const MOST_CYCLES = 16;

  // How high `kind` is at `phase`, from 0 to 1 along a cycle: each wave
  // starts its cycle rising from zero, as the engine plays it.
  function kindHeight(kind: string, phase: number): number {
    if (kind === "sine") {
      return Math.sin(2 * Math.PI * phase);
    }
    if (kind === "triangle") {
      if (phase < 0.25) {
        return 4 * phase;
      }
      if (phase < 0.75) {
        return 2 - 4 * phase;
      }
      return 4 * phase - 4;
    }
    if (kind === "square") {
      if (phase < 0.5) {
        return 1;
      }
      return -1;
    }
    if (phase < 0.5) {
      return 2 * phase;
    }
    return 2 * phase - 2;
  }

  // A height from -1 to 1 for point `point` of the noise: always the same,
  // so the drawing holds still.
  function noiseHeight(point: number): number {
    const big = Math.sin(point * 12.9898 + 78.233) * 43758.5453;
    const share = big - Math.floor(big);
    return 2 * share - 1;
  }

  // How many cycles the drawing shows for `oscillator`.
  function cyclesOf(oscillator: OscillatorView): number {
    const cents = oscillator.octaves * 1200 + oscillator.half_steps * 100 + oscillator.cents;
    const cycles = CYCLES_ON_THE_NOTE * Math.pow(2, cents / 1200);
    return Math.max(FEWEST_CYCLES, Math.min(MOST_CYCLES, cycles));
  }

  function levelY(level: number): number {
    return MIDDLE_Y - (level / 100) * TALLEST;
  }

  // The level at `y`, in percent.
  function levelAt(y: number): number {
    const share = (MIDDLE_Y - y) / TALLEST;
    return PERCENT.at(Math.max(0, Math.min(1, share)));
  }

  function pathOf(oscillator: OscillatorView): string {
    const size = (oscillator.level / 100) * TALLEST;
    const cycles = cyclesOf(oscillator);
    const points: string[] = [];
    for (let point = 0; point <= POINTS; point += 1) {
      const along = point / POINTS;
      const x = LEFT + (RIGHT - LEFT) * along;
      let height = 0;
      if (oscillator.kind === "noise") {
        height = noiseHeight(point);
      } else {
        const phase = (along * cycles) % 1;
        height = kindHeight(oscillator.kind, phase);
      }
      const y = MIDDLE_Y - height * size;
      points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  }

  let paths = $derived(sound.oscillators.map(pathOf));

  // A silent oscillator's tuning does nothing, and nor does noise's.
  function tuningDoesNothing(oscillator: OscillatorView): boolean {
    if (oscillator.level === 0) {
      return true;
    }
    return oscillator.kind === "noise";
  }
</script>

<div class="sound-modules" style:grid-template-columns="1fr 1fr 1fr">
  {#each sound.oscillators as oscillator, index}
    {@const settings = SETTINGS[index]}
    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">Oscillator {index + 1}</span>
        <span class="sound-hint">
          {oscillator.kind}
          {#if index === 0}
            <span class="sound-key-chip">\</span>
          {/if}
        </span>
      </div>
      <div class="sound-drawing" class:is-silent={oscillator.level === 0}>
        <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="xMidYMid meet">
          <path class="sound-lit-soft is-faint" d="M{LEFT} {MIDDLE_Y} L {RIGHT} {MIDDLE_Y}" />
          <path class="sound-lit is-fine" d={paths[index]} />
          <SoundHandle
            x={RIGHT + 10}
            y={levelY(oscillator.level)}
            ringed={isPicked(picked, settings.level.id)}
            onGrab={() => onPick(pickedOf(settings.level))}
            onMove={(_x, y) => settings.level.set(levelAt(y))}
          />
        </svg>
      </div>
      <div class="sound-kinds">
        {#each KINDS as kind (kind.name)}
          <button
            type="button"
            class="sound-shape"
            class:is-chosen={oscillator.kind === kind.name}
            title={kind.name}
            onclick={(event) => clickThenLetGo(event, () => sendOscillator(index, { kind: kind.name }))}
          >
            <svg class="sound-glyph" viewBox="0 0 20 12"><path d={kind.glyph} /></svg>
          </button>
        {/each}
      </div>
      <div class="sound-fields" style:grid-template-columns="1fr 1fr">
        <SoundValue setting={settings.level} dots={11} {picked} {onPick} />
        <SoundValue
          setting={settings.octaves}
          dots={7}
          {picked}
          {onPick}
          bipolar
          dim={tuningDoesNothing(oscillator)}
        />
        <SoundValue
          setting={settings.halfSteps}
          dots={13}
          {picked}
          {onPick}
          bipolar
          dim={tuningDoesNothing(oscillator)}
        />
        <SoundValue setting={settings.cents} dots={17} {picked} {onPick} bipolar dim={tuningDoesNothing(oscillator)} />
      </div>
    </div>
  {/each}
</div>
