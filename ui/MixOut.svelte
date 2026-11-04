<!-- A track's or a bus's Out, open on the Mix panel's right (docs/design.md#mix):
     where the sound leaves. A track's reads its volume, its pan and a send
     to each bus; a bus's its volume and pan, its name and a way to remove
     it. "+ Bus" adds a bus, up to 4. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";
  import { MOST_BUSES, PAN, type MixSender, type MixView } from "./mix";
  import { PERCENT, type Picked, type Setting } from "./sound-scales";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    // "track 2" or "bus 1".
    place: string;
    view: MixView;
    // The strip's volume in percent: a track's comes from the sound.
    volume: number;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    onVolume: (volume: number) => void;
    send: MixSender;
  }

  let { place, view, volume, picked, onPick, onVolume, send }: Props = $props();

  // The track or bus, from 1.
  let number = $derived(Number(place.split(" ")[1]));
  let isBus = $derived(place.startsWith("bus "));

  function panNow(): number {
    if (isBus) {
      return view.buses[number - 1]?.pan ?? 0;
    }
    return view.tracks[number - 1]?.pan ?? 0;
  }

  let volumeSetting: Setting = $derived({
    id: `mix ${place} volume`,
    name: "Volume",
    scale: PERCENT,
    now: () => volume,
    set: (newVolume) => onVolume(newVolume),
  });

  let panSetting: Setting = $derived({
    id: `mix ${place} pan`,
    name: "Pan",
    scale: PAN,
    now: panNow,
    set: (pan) => send.setPan(place, pan),
  });

  // One send for each bus, on a track's Out.
  let sendSettings: Setting[] = $derived.by(() => {
    if (isBus) {
      return [];
    }
    const settings: Setting[] = [];
    for (let bus = 1; bus <= view.buses.length; bus += 1) {
      const track = number;
      settings.push({
        id: `mix ${place} send ${bus}`,
        name: `Send B${bus} ${view.buses[bus - 1].name}`,
        scale: PERCENT,
        now: () => view.tracks[track - 1]?.sends[bus - 1] ?? 0,
        set: (amount) => send.setSend(track, bus, amount),
      });
    }
    return settings;
  });

  // A bus's name, edited in place: Return or leaving the field keeps it,
  // Escape puts it back.
  let newName = $state("");
  let editing = $state(false);

  function startEditing() {
    editing = true;
    newName = view.buses[number - 1]?.name ?? "";
  }

  function finishEditing() {
    if (!editing) {
      return;
    }
    editing = false;
    send.renameBus(number, newName);
  }

  function onNameKey(event: KeyboardEvent) {
    if (event.key === "Enter") {
      finishEditing();
    } else if (event.key === "Escape") {
      // Escape only puts the name back: it must not reach the page and
      // close the panel, nor leave the Mac's full screen.
      event.preventDefault();
      event.stopPropagation();
      editing = false;
    }
  }
</script>

<div class="sound-fields mix-out-fields">
  <SoundValue setting={volumeSetting} dots={13} {picked} {onPick} />
  <SoundValue setting={panSetting} dots={13} {picked} {onPick} bipolar />
  {#if isBus}
    <div class="mix-bus-name">
      <div class="sound-value-name">Name</div>
      {#if editing}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="mix-name-input"
          aria-label={`Bus ${number}'s name`}
          maxlength="32"
          bind:value={newName}
          autofocus
          onkeydown={onNameKey}
          onblur={finishEditing}
        />
      {:else}
        <button
          type="button"
          class="mix-name"
          title="Click to rename"
          onclick={(event) => clickThenLetGo(event, startEditing)}
        >
          {view.buses[number - 1]?.name ?? ""}
        </button>
      {/if}
    </div>
  {/if}
  {#each sendSettings as setting (setting.id)}
    <SoundValue {setting} dots={13} {picked} {onPick} />
  {/each}
  {#if view.buses.length < MOST_BUSES}
    <div class="mix-add-bus">
      <div class="sound-value-name">New bus</div>
      <div class="sound-choice">
        <button type="button" onclick={(event) => clickThenLetGo(event, send.addBus)}>+ Bus</button>
      </div>
    </div>
  {/if}
</div>
{#if isBus}
  <p class="mix-note">
    Tracks send to it from their Out. Removing it removes their sends.
    <button
      type="button"
      class="sound-hint-button mix-remove-bus"
      onclick={(event) => clickThenLetGo(event, () => send.removeBus(number))}
    >
      Remove bus
    </button>
  </p>
{:else}
  <p class="mix-note">Sends leave after the volume and pan, so a quieter track sends less.</p>
{/if}
