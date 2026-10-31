<!-- The sound panel's Oscillators page (docs/design.md#sound-panel): the
     three oscillators every note plays, side by side
     (docs/adr/0035-oscillators-with-kinds.md). Each has a drawing of what it
     plays, its six kind tiles, its level and a square's width
     (docs/design.md#pulse-width), and its tuning. A sample shows its
     instrument in the drawing's place (docs/design.md#sampler): clicking
     it lists the instruments over the oscillator until one is picked. Its
     touch, how hard its keys are struck, takes the width's place. A string
     (docs/design.md#guitar) has its decay there, and a row of its own for its
     tone, where it is plucked, its pickup and its body. All of them start with
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
    STRING_FURTHEST,
    STRING_LONGEST_DECAY_MS,
    STRING_NEAREST,
    STRING_SHORTEST_DECAY_MS,
    TOUCH_HARDEST,
    TOUCH_SOFTEST,
    type Oscillator,
    type OscillatorView,
    type SoundSender,
    type SoundView,
    type StringSound,
  } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundInstruments from "./SoundInstruments.svelte";
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
  const TOUCH_SCALE = evenScale(TOUCH_SOFTEST, TOUCH_HARDEST, 1, "", "");
  const DECAY_SCALE = evenScale(STRING_SHORTEST_DECAY_MS, STRING_LONGEST_DECAY_MS, 100, " ms", " ms");
  const PICK_SCALE = evenScale(STRING_NEAREST, STRING_FURTHEST, 1, "%", "%");
  const PICKUP_SCALE = evenScale(0, STRING_FURTHEST, 1, "%", "%");

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
      touch: now.touch,
      string: now.string,
      ...change,
    };
    send.setOscillator(index, oscillator);
  }

  function sendString(index: number, change: Partial<StringSound>) {
    sendOscillator(index, { string: { ...sound.oscillators[index].string, ...change } });
  }

  // One value of a string, picked by its own id, like "osc 2 tone".
  function stringSetting(index: number, name: string, field: keyof StringSound, scale: Setting["scale"]): Setting {
    return {
      id: `osc ${index + 1} ${field}`,
      name,
      scale,
      now: () => sound.oscillators[index].string[field],
      set: (value) => sendString(index, { [field]: value }),
    };
  }

  // The six values of oscillator `index`, each picked by its own id, like
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
    const touch: Setting = {
      id: `${prefix} touch`,
      name: "Touch",
      scale: TOUCH_SCALE,
      now: () => sound.oscillators[index].touch,
      set: (newTouch) => sendOscillator(index, { touch: newTouch }),
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
    const decay = stringSetting(index, "Decay", "decay_ms", DECAY_SCALE);
    const tone = stringSetting(index, "Tone", "tone", PERCENT);
    const pick = stringSetting(index, "Pick", "pick", PICK_SCALE);
    const pickup = stringSetting(index, "Pickup", "pickup", PICKUP_SCALE);
    const pickupTone = stringSetting(index, "Coil", "pickup_tone", PERCENT);
    const body = stringSetting(index, "Body", "body", PERCENT);
    return { level, width, touch, octaves, halfSteps, cents, decay, tone, pick, pickup, pickupTone, body };
  }

  const SETTINGS = [settingsOf(0), settingsOf(1), settingsOf(2)];

  // The oscillator whose instruments are listed, from 0, or null.
  let choosing: number | null = $state(null);

  function chooseInstrument(index: number, name: string | null) {
    send.setInstrument(index, name);
    choosing = null;
  }

  // What a sample's button says: its instrument, and whether it is ready.
  function instrumentWords(oscillator: OscillatorView): string {
    const instrument = oscillator.instrument;
    if (instrument.name === null) {
      return "No instrument";
    }
    if (instrument.state === "loading") {
      return `${instrument.name}, loading`;
    }
    if (instrument.state === "failed") {
      return `${instrument.name}, can't be read`;
    }
    return instrument.name;
  }

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
      } else if (oscillator.kind === "string") {
        // Plucked, then ringing away.
        height = Math.sin(2 * Math.PI * along * cycles * 3) * Math.exp(-3 * along);
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
        {#if choosing === index}
          <button
            type="button"
            class="sound-hint sound-hint-button"
            onclick={(event) => clickThenLetGo(event, () => (choosing = null))}
          >
            Close the list
          </button>
        {:else}
          <span class="sound-hint">
            {oscillator.kind}
            {#if index === 0}
              <span class="sound-key-chip">F12</span>
            {/if}
          </span>
        {/if}
      </div>
      {#if choosing === index}
        <div class="sound-instrument-list">
          <SoundInstruments
            chosen={oscillator.instrument}
            onChoose={(name) => chooseInstrument(index, name)}
            drums={false}
            {send}
          />
        </div>
      {:else}
        {#if oscillator.kind === "sample"}
          <button
            type="button"
            class="sound-drawing sound-instrument-pick"
            class:is-silent={oscillator.level === 0}
            onclick={(event) => clickThenLetGo(event, () => (choosing = index))}
          >
            <span class="sound-instrument-name">{instrumentWords(oscillator)}</span>
            <span class="sound-instrument-more">pick</span>
          </button>
        {:else}
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
        {/if}
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
          {#if oscillator.kind === "sample"}
            <SoundValue setting={settings.touch} dots={11} {picked} {onPick} dim={oscillator.level === 0} />
          {:else if oscillator.kind === "string"}
            <SoundValue setting={settings.decay} dots={11} {picked} {onPick} dim={oscillator.level === 0} />
          {:else}
            <SoundValue setting={settings.width} dots={11} {picked} {onPick} dim={widthDoesNothing(oscillator)} />
          {/if}
        </div>
        {#if oscillator.kind === "string"}
          <div class="sound-fields" style:grid-template-columns="1fr 1fr 1fr 1fr 1fr">
            <SoundValue setting={settings.tone} dots={7} {picked} {onPick} dim={oscillator.level === 0} />
            <SoundValue setting={settings.pick} dots={7} {picked} {onPick} dim={oscillator.level === 0} />
            <SoundValue setting={settings.pickup} dots={7} {picked} {onPick} dim={oscillator.level === 0} />
            <SoundValue setting={settings.pickupTone} dots={7} {picked} {onPick} dim={oscillator.level === 0 || oscillator.string.pickup === 0} />
            <SoundValue setting={settings.body} dots={7} {picked} {onPick} dim={oscillator.level === 0} />
          </div>
        {/if}
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
      {/if}
    </div>
  {/each}
</div>
