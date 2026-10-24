<!-- The sound panel's Drums page (docs/design.md#sampler): the song's drum
     kit, picked from the instruments, then its sixteen pads as tiles, and
     the open pad's level, tuning, pan and group. A pad's group stops the
     others in it when it is hit, like the closed hat stopping the open hat.
     Right Option (right Ctrl on Windows) switches the letters to drums.
     Changes start with the next hit. -->
<script lang="ts">
  import { isPicked, PERCENT, signedScale, type Picked, type Setting } from "./sound-scales";
  import { DRUM_MOST_GROUP, DRUM_MOST_HALF_STEPS, type PadView, type SoundSender, type SoundView } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundInstruments from "./SoundInstruments.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    onLetGo: () => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, onLetGo, send }: Props = $props();

  const HALF_STEPS = signedScale(DRUM_MOST_HALF_STEPS, "", "");
  const PAN = signedScale(100, "%", "%");

  // The groups to choose from: none, then 1 to DRUM_MOST_GROUP.
  const GROUPS: number[] = [];
  for (let group = 0; group <= DRUM_MOST_GROUP; group += 1) {
    GROUPS.push(group);
  }

  // The pad open, from 0. It opens on the kick.
  let open = $state(0);

  let pad = $derived(sound.drums.pads[open]);

  // Pad `index` as it is now, with `change` made to it.
  function sendPad(index: number, change: Partial<PadView>) {
    const now = sound.drums.pads[index];
    const settings: PadView = {
      name: now.name,
      level: now.level,
      half_steps: now.half_steps,
      pan: now.pan,
      group: now.group,
      ...change,
    };
    send.setDrumPad(index, settings);
  }

  // The open pad's values, each picked by its own id, like "snare level".
  let settings = $derived.by(() => {
    const index = open;
    const name = sound.drums.pads[index].name;
    const level: Setting = {
      id: `${name} level`,
      name: "Level",
      scale: PERCENT,
      now: () => sound.drums.pads[index].level,
      set: (newLevel) => sendPad(index, { level: newLevel }),
    };
    const halfSteps: Setting = {
      id: `${name} half steps`,
      name: "Half steps",
      scale: HALF_STEPS,
      now: () => sound.drums.pads[index].half_steps,
      set: (newHalfSteps) => sendPad(index, { half_steps: newHalfSteps }),
    };
    const pan: Setting = {
      id: `${name} pan`,
      name: "Pan",
      scale: PAN,
      now: () => sound.drums.pads[index].pan,
      set: (newPan) => sendPad(index, { pan: newPan }),
    };
    return { level, halfSteps, pan };
  });

  function openPad(index: number) {
    open = index;
    onLetGo();
  }

  // What group `group` is called on its tile.
  function groupWords(group: number): string {
    if (group === 0) {
      return "None";
    }
    return String(group);
  }

  // Whether any of the open pad's values is picked, so its tile stays lit
  // in the list too.
  let padPicked = $derived.by(() => {
    if (isPicked(picked, settings.level.id)) {
      return true;
    }
    if (isPicked(picked, settings.halfSteps.id)) {
      return true;
    }
    return isPicked(picked, settings.pan.id);
  });
</script>

<div class="sound-modules" style:grid-template-columns="1fr 2fr 1fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Kit</span>
      <span class="sound-hint">{sound.drums.kit.name ?? "none"}</span>
    </div>
    <div class="sound-drawing sound-instruments">
      <SoundInstruments
        chosen={sound.drums.kit}
        onChoose={(name) => send.setDrumKit(name)}
        drums={true}
        {send}
      />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Pads</span>
      <span class="sound-hint">a group stops the others in it</span>
    </div>
    <div class="drum-pads">
      {#each sound.drums.pads as each, index (each.name)}
        <button
          type="button"
          class="sound-tile drum-pad"
          class:is-chosen={open === index}
          class:is-dim={each.level === 0}
          onclick={(event) => clickThenLetGo(event, () => openPad(index))}
        >
          <span class="drum-pad-name">{each.name}</span>
          {#if each.group !== 0}
            <span class="drum-pad-group">group {each.group}</span>
          {/if}
        </button>
      {/each}
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label" class:is-picked={padPicked}>{pad.name}</span>
      <span class="sound-hint">&nbsp;</span>
    </div>
    <div class="sound-fields" style:grid-template-columns="1fr">
      <SoundValue setting={settings.level} dots={11} {picked} {onPick} />
    </div>
    <div class="sound-fields" style:grid-template-columns="1fr 1fr">
      <SoundValue setting={settings.halfSteps} dots={13} {picked} {onPick} bipolar />
      <SoundValue setting={settings.pan} dots={11} {picked} {onPick} bipolar />
    </div>
    <div class="sound-choice drum-groups">
      {#each GROUPS as group (group)}
        <button
          type="button"
          class:is-chosen={pad.group === group}
          onclick={(event) => clickThenLetGo(event, () => sendPad(open, { group }))}
        >
          {groupWords(group)}
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .drum-pads {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    grid-auto-rows: 1fr;
    gap: 5px;
    flex: 1 1 auto;
    min-height: 0;
  }

  .drum-pad {
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 1px;
    height: auto;
  }

  .drum-pad-name::first-letter {
    text-transform: uppercase;
  }

  .drum-pad-group {
    font-size: 10px;
    color: var(--text-3);
  }

  .drum-groups {
    margin-top: auto;
  }

  .is-picked {
    color: rgb(var(--accent));
  }
</style>
