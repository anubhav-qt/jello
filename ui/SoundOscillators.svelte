<!-- The sound panel's Oscillators page (docs/design.md#sound-panel): the
     three oscillators every note plays, side by side
     (docs/adr/0035-oscillators-with-kinds.md). Each has a drawing of what it
     plays, its five kind tiles, its level and a square's width
     (docs/design.md#pulse-width), and its tuning. All of them start with
     the next key pressed. -->
<script lang="ts">
  import {
    evenScale,
    isPicked,
    KINDS,
    PERCENT,
    pickedOf,
    signedScale,
    type Picked,
    type Setting,
  } from "./sound-scales";
  import {
    OSCILLATOR_MOST_CENTS,
    OSCILLATOR_MOST_HALF_STEPS,
    OSCILLATOR_MOST_OCTAVES,
    PULSE_NARROWEST_PERCENT,
    PULSE_WIDEST_PERCENT,
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

  // The tuning's three fields share one row with no unit after the number,
  // since each field's name says it: "-12 half steps -100 cents" would not
  // fit side by side.
  const OCTAVES = signedScale(OSCILLATOR_MOST_OCTAVES, "", "");
  const HALF_STEPS = signedScale(OSCILLATOR_MOST_HALF_STEPS, "", "");
  const CENTS = signedScale(OSCILLATOR_MOST_CENTS, "", "");
  const WIDTH_SCALE = evenScale(PULSE_NARROWEST_PERCENT, PULSE_WIDEST_PERCENT, 1, "%", "%");

  // Oscillator `index` as it is now, with `change` made to it.
  function sendOscillator(index: number, change: Partial<Oscillator>) {
    const now = sound.oscillators[index];
    const oscillator: Oscillator = {
      kind: now.kind,
      level: now.level,
      octaves: now.octaves,
      halfSteps: now.half_steps,
      cents: now.cents,
      width: now.width,
      ...change,
    };
    send.setOscillator(index, oscillator);
  }

  // The five values of oscillator `index`, each picked by its own id, like
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
    const width: Setting = {
      id: `${prefix} width`,
      name: "Width",
      scale: WIDTH_SCALE,
      now: () => sound.oscillators[index].width,
      set: (newWidth) => sendOscillator(index, { width: newWidth }),
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
    return { level, width, octaves, halfSteps, cents };
  }

  const SETTINGS = [settingsOf(0), settingsOf(1), settingsOf(2)];

  // The drawing, 280 by 40, as wide for its height as the box it fills:
  // the oscillator's wave around a middle line, as tall as its level. Right
  // on the note it shows 2 cycles, and each octave up twice as many, so the
  // tuning shows too. Its square, at the right end, drags the level up and
  // down. A square's pulse has a second square on its first fall, which
  // drags its width sideways.
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
  // starts its cycle rising from zero, as the engine plays it. A square is
  // up for the first `width` percent of its cycle. The engine also lifts a
  // thin pulse to balance it on the middle line, which the drawing leaves
  // out, so the pulse always fills the same height.
  function kindHeight(kind: string, phase: number, width: number): number {
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
      if (phase < width / 100) {
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
        height = kindHeight(oscillator.kind, phase, oscillator.width);
      }
      const y = MIDDLE_Y - height * size;
      points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  }

  let paths = $derived(sound.oscillators.map(pathOf));

  // How wide one cycle of `oscillator` is drawn.
  function cycleWidth(oscillator: OscillatorView): number {
    return (RIGHT - LEFT) / cyclesOf(oscillator);
  }

  // Where a square's first fall is drawn, kept inside the drawing when a
  // low oscillator's cycle is wider than it.
  function fallX(oscillator: OscillatorView): number {
    const x = LEFT + (cycleWidth(oscillator) * oscillator.width) / 100;
    return Math.min(x, RIGHT);
  }

  // The width that puts the first fall at `x`, in percent, rounded and kept
  // within its range, as its dots do.
  function widthAt(oscillator: OscillatorView, x: number): number {
    const percent = ((x - LEFT) / cycleWidth(oscillator)) * 100;
    return WIDTH_SCALE.at(WIDTH_SCALE.share(percent));
  }

  // Only a square has a width.
  function widthDoesNothing(oscillator: OscillatorView): boolean {
    if (oscillator.level === 0) {
      return true;
    }
    return oscillator.kind !== "square";
  }

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
          {#if oscillator.kind === "square"}
            <SoundHandle
              x={fallX(oscillator)}
              y={MIDDLE_Y}
              ringed={isPicked(picked, settings.width.id)}
              onGrab={() => onPick(pickedOf(settings.width))}
              onMove={(x) => settings.width.set(widthAt(oscillator, x))}
            />
          {/if}
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
        <SoundValue setting={settings.width} dots={11} {picked} {onPick} dim={widthDoesNothing(oscillator)} />
      </div>
      <div class="sound-fields" style:grid-template-columns="1fr 1fr 1fr">
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
        <SoundValue setting={settings.cents} dots={11} {picked} {onPick} bipolar dim={tuningDoesNothing(oscillator)} />
      </div>
    </div>
  {/each}
</div>
