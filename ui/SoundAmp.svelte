<!-- The sound panel's Amp page (docs/design.md#guitar): the guitar amp every
     note plays through, off until switched on. Its knobs move notes already
     sounding. -->
<script lang="ts">
  import { PERCENT, type Picked, type Setting } from "./sound-scales";
  import type { Amp, SoundSender, SoundView } from "./sound";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, send }: Props = $props();

  type Knob = Exclude<keyof Amp, "on">;

  function knob(name: string, field: Knob): Setting {
    return {
      id: `amp ${field}`,
      name,
      scale: PERCENT,
      now: () => sound.amp[field],
      set: (value) => send.setAmp({ ...sound.amp, [field]: value }),
    };
  }

  const knobs = [
    knob("Gain", "gain"),
    knob("Bass", "bass"),
    knob("Middle", "middle"),
    knob("Treble", "treble"),
    knob("Presence", "presence"),
    knob("Speaker", "speaker"),
    knob("Volume", "volume"),
  ];

  function switchAmp() {
    send.setAmp({ ...sound.amp, on: !sound.amp.on });
  }
</script>

<div class="sound-fields" style:grid-template-columns="repeat(8, 1fr)">
  <button type="button" class="keys-button amp-switch" class:is-picked={sound.amp.on} onclick={switchAmp}>
    {sound.amp.on ? "Amp on" : "Amp off"}
  </button>
  {#each knobs as setting (setting.id)}
    <SoundValue {setting} dots={11} {picked} {onPick} dim={!sound.amp.on} />
  {/each}
</div>

<style>
  .amp-switch {
    align-self: start;
  }
</style>
