<!-- The Mix panel (docs/design.md#mix): the song's mix, set while the
     keyboard keeps playing. It opens in Compose's tape's place, like the
     sound panel. On the left, every track, bus and the master as a thin
     strip; a click picks one, as left Cmd and a number pick a track. In the
     middle, the picked strip's 8 slots and its Out. On the right, the open
     effect, its Out, or the list of effects to add. Clicking empty space
     lets go of the value picked. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, untrack } from "svelte";
  import {
    decibelsOf,
    EFFECT_SLOTS,
    FIRST_BUS_CHAIN,
    kindName,
    MASTER_CHAIN,
    panWords,
    type MetersView,
    type MixSender,
    type MixView,
    type Strip,
  } from "./mix";
  import MixAdd from "./MixAdd.svelte";
  import MixChain from "./MixChain.svelte";
  import MixChorus from "./MixChorus.svelte";
  import MixCompressor from "./MixCompressor.svelte";
  import MixDelay from "./MixDelay.svelte";
  import MixEq from "./MixEq.svelte";
  import MixFilter from "./MixFilter.svelte";
  import MixGate from "./MixGate.svelte";
  import MixLevels from "./MixLevels.svelte";
  import MixLimiter from "./MixLimiter.svelte";
  import MixOut from "./MixOut.svelte";
  import MixReverb from "./MixReverb.svelte";
  import MixSaturation from "./MixSaturation.svelte";
  import MixShaper from "./MixShaper.svelte";
  import { clickThenLetGo } from "./dom";
  import PickHint from "./PickHint.svelte";
  import type { SoundSender, SoundView } from "./sound";
  import type { Picked } from "./sound-scales";

  interface Props {
    sound: SoundView;
    // The mix, as Rust answered last, or null until it has.
    mix: MixView | null;
    keysAcross: number;
    picked: Picked | null;
    adjustModifier: string;
    onPick: (picked: Picked) => void;
    onLetGo: () => void;
    send: MixSender;
    soundSender: SoundSender;
  }

  let { sound, mix, keysAcross, picked, adjustModifier, onPick, onLetGo, send, soundSender }: Props = $props();

  // How often the meters are read, in ms, how fast a meter falls, in dB a
  // read, and how many reads the limiter's drawing keeps: 4 s.
  const METER_EVERY_MS = 50;
  const FALL_DB = 1.5;
  const HISTORY = 80;
  // How long the true peak shown holds its loudest, in reads.
  const TRUE_PEAK_HOLD = 60;

  // The strip picked: null for the picked track, or "bus 2" or "master".
  let pickedOther: string | null = $state(null);
  let place = $derived.by(() => {
    if (pickedOther !== null) {
      return pickedOther;
    }
    return `track ${sound.picked_track}`;
  });

  // What is open on the right: a slot, from 1, its Out, or the list of
  // effects to add to slot `adding`.
  let open: number | "out" | "add" = $state("out");
  let adding: number | null = $state(null);

  // Left Cmd and a number pick a track, which picks its strip here too.
  let lastTrack = untrack(() => sound.picked_track);
  $effect(() => {
    const track = sound.picked_track;
    if (track === lastTrack) {
      return;
    }
    lastTrack = track;
    pickedOther = null;
    openFirst();
  });

  // A bus that went away can't stay picked.
  $effect(() => {
    if (mix === null || pickedOther === null || !pickedOther.startsWith("bus ")) {
      return;
    }
    const bus = Number(pickedOther.split(" ")[1]);
    if (bus > mix.buses.length) {
      pickedOther = null;
      open = "out";
    }
  });

  // A track made or removed elsewhere: ask for the mix again.
  $effect(() => {
    if (mix !== null && mix.tracks.length !== sound.tracks.length) {
      send.refresh();
    }
  });

  let chain = $derived.by(() => {
    if (mix === null) {
      return [];
    }
    if (place === "master") {
      return mix.master.chain;
    }
    const number = Number(place.split(" ")[1]);
    if (place.startsWith("bus ")) {
      return mix.buses[number - 1]?.chain ?? [];
    }
    return mix.tracks[number - 1]?.chain ?? [];
  });

  // The slot open, if it holds an effect.
  let openEffect = $derived.by(() => {
    if (typeof open !== "number") {
      return null;
    }
    return chain[open - 1] ?? null;
  });

  // Opens the first slot that holds an effect, or Out.
  function openFirst() {
    adding = null;
    open = "out";
    for (let slot = 1; slot <= chain.length; slot += 1) {
      if (chain[slot - 1] !== null) {
        open = slot;
        return;
      }
    }
  }

  // Picking the strip already picked, as dragging its fader does, keeps
  // what is open.
  function pickStrip(newPlace: string) {
    if (newPlace === place) {
      return;
    }
    onLetGo();
    if (newPlace.startsWith("track ")) {
      pickedOther = null;
      const track = Number(newPlace.split(" ")[1]);
      if (track !== sound.picked_track) {
        soundSender.pickTrack(track);
        return;
      }
    } else {
      pickedOther = newPlace;
    }
    openFirst();
  }

  function openSlot(slot: number | "out") {
    onLetGo();
    adding = null;
    open = slot;
  }

  function startAdding(slot: number) {
    onLetGo();
    adding = slot;
    open = "add";
  }

  function addEffect(kind: string) {
    if (adding === null) {
      return;
    }
    send.setEffect(place, adding, kind);
    open = adding;
    adding = null;
  }

  function removeEffect() {
    if (typeof open !== "number") {
      return;
    }
    onLetGo();
    send.setEffect(place, open, "none");
    open = "out";
  }

  function moveEffect(from: number, to: number) {
    send.moveEffect(place, from, to);
    if (open === from) {
      open = to;
    }
  }

  // A track's volume comes from its sound; a bus's from the mix.
  function setVolume(stripPlace: string, volume: number) {
    const number = Number(stripPlace.split(" ")[1]);
    if (stripPlace.startsWith("track ")) {
      soundSender.setTrackVolume(number, volume);
    } else if (stripPlace.startsWith("bus ")) {
      send.setBusVolume(number, volume);
    }
  }

  // The meters, falling slowly, in dB, by chain; the master's last 4 s;
  // and the rest the last read said.
  let shownDb: number[] = $state(new Array(MASTER_CHAIN + 1).fill(-120));
  let masterPeaks: number[] = $state([]);
  let masterCuts: number[] = $state([]);
  let meters = $state<MetersView | null>(null);
  let truePeaks: number[] = [];
  let heldTruePeak = $state(0);

  function readMeters(read: MetersView) {
    const next: number[] = [];
    for (let chainNumber = 0; chainNumber <= MASTER_CHAIN; chainNumber += 1) {
      const fallen = shownDb[chainNumber] - FALL_DB;
      next.push(Math.max(decibelsOf(read.peaks[chainNumber] ?? 0), fallen));
    }
    shownDb = next;
    masterPeaks = [...masterPeaks, read.peaks[MASTER_CHAIN] ?? 0].slice(-HISTORY);
    masterCuts = [...masterCuts, read.limiter_reduction_db].slice(-HISTORY);
    truePeaks = [...truePeaks, read.true_peak].slice(-TRUE_PEAK_HOLD);
    heldTruePeak = Math.max(...truePeaks);
    meters = read;
  }

  onMount(() => {
    let asking = false;
    const timer = window.setInterval(async () => {
      if (asking || mix === null) {
        return;
      }
      asking = true;
      try {
        const read = await invoke<MetersView>("mix_meters", { place });
        readMeters(read);
      } catch {
        // The place may have just gone away; the next read asks again.
      }
      asking = false;
    }, METER_EVERY_MS);
    return () => window.clearInterval(timer);
  });

  let strips = $derived.by(() => {
    const all: Strip[] = [];
    if (mix === null) {
      return all;
    }
    for (let track = 1; track <= sound.tracks.length; track += 1) {
      const view = sound.tracks[track - 1];
      all.push({
        place: `track ${track}`,
        label: String(track),
        title: `${track}. ${view.name}`,
        volume: view.volume,
        pan: mix.tracks[track - 1]?.pan ?? 0,
        db: shownDb[track - 1],
        gap: false,
      });
    }
    for (let bus = 1; bus <= mix.buses.length; bus += 1) {
      const view = mix.buses[bus - 1];
      all.push({
        place: `bus ${bus}`,
        label: `B${bus}`,
        title: `Bus ${bus}: ${view.name}`,
        volume: view.volume,
        pan: view.pan,
        db: shownDb[FIRST_BUS_CHAIN + bus - 1],
        gap: bus === 1,
      });
    }
    all.push({
      place: "master",
      label: "M",
      title: "The master",
      volume: null,
      pan: null,
      db: shownDb[MASTER_CHAIN],
      gap: true,
    });
    return all;
  });

  // The picked strip's name, for the tab at the top.
  let title = $derived.by(() => {
    if (place === "master") {
      return "Master";
    }
    const number = Number(place.split(" ")[1]);
    if (place.startsWith("bus ")) {
      return `B${number} ${mix?.buses[number - 1]?.name ?? ""}`;
    }
    return `${number}. ${sound.tracks[number - 1]?.name ?? ""}`;
  });

  // What Out reads: a track's volume, pan and sends, a bus's volume and
  // pan, the master's limiter.
  let outWords = $derived.by(() => {
    if (mix === null) {
      return "";
    }
    if (place === "master") {
      return `limiter ${mix.master.limiter.ceiling_db} dB`;
    }
    const number = Number(place.split(" ")[1]);
    if (place.startsWith("bus ")) {
      const bus = mix.buses[number - 1];
      if (bus === undefined) {
        return "";
      }
      return `${bus.volume} · ${panWords(bus.pan)}`;
    }
    const track = mix.tracks[number - 1];
    const volume = sound.tracks[number - 1]?.volume ?? 0;
    if (track === undefined) {
      return "";
    }
    let sends = 0;
    for (const amount of track.sends) {
      if (amount > 0) {
        sends += 1;
      }
    }
    let sendWords = `${sends} sends`;
    if (sends === 1) {
      sendWords = "1 send";
    }
    return `${volume} · ${panWords(track.pan)} · ${sendWords}`;
  });

  let outVolume = $derived.by(() => {
    const number = Number(place.split(" ")[1]);
    if (place.startsWith("bus ")) {
      return mix?.buses[number - 1]?.volume ?? 0;
    }
    return sound.tracks[number - 1]?.volume ?? 0;
  });

  let reductions = $derived(meters?.reductions ?? new Array(EFFECT_SLOTS).fill(0));

  let lufsWords = $derived.by(() => {
    if (meters === null || meters.short_term_lufs <= -70) {
      return "-inf LUFS";
    }
    return `${meters.short_term_lufs.toFixed(1)} LUFS`;
  });

  // The right module's name and hint.
  let openName = $derived.by(() => {
    if (open === "add") {
      return `${adding}. Add effect`;
    }
    if (open === "out" || openEffect === null) {
      if (place === "master") {
        return "Out: limiter";
      }
      return "Out";
    }
    return `${open}. ${kindName(openEffect.kind)}`;
  });

  let openHint = $derived.by(() => {
    if (open === "add") {
      return "the sound runs through the slots top to bottom";
    }
    if (open === "out" || openEffect === null) {
      if (place === "master") {
        return "always last, can't be removed";
      }
      if (place.startsWith("bus ")) {
        return "volume and pan";
      }
      return "volume, pan, sends";
    }
    return "";
  });

  // Pressing on the panel anywhere but a value, a drawing's square or a
  // button lets go of the value picked.
  function letGoOutside(event: PointerEvent) {
    const target = event.target as Element;
    if (target.closest("button, input, .sound-value, .sound-handle") !== null) {
      return;
    }
    onLetGo();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section class="sound-panel mix-panel" style:--keys-across={keysAcross} onpointerdown={letGoOutside}>
  <div class="sound-panel-top">
    <span class="sound-panel-title">Mix</span>
    <span class="sound-page-tab is-open">{title}</span>
    {#if picked === null}
      <span class="sound-panel-note">Click a strip to pick it · {adjustModifier} + number picks a track</span>
    {:else}
      <PickHint name={picked.name} {adjustModifier} />
    {/if}
  </div>

  <div class="sound-panel-content">
    {#if mix !== null}
      <div class="sound-modules mix-modules">
        <div class="sound-module mix-levels-module">
          <div class="sound-module-top">
            <span class="sound-label">Levels</span>
            <span class="sound-hint">{lufsWords}</span>
          </div>
          <MixLevels {strips} picked={place} onPick={pickStrip} onVolume={setVolume} />
        </div>

        <div class="sound-module mix-chain-module">
          <div class="sound-module-top">
            <span class="sound-label">Chain</span>
            <span class="sound-hint">top to bottom</span>
          </div>
          <MixChain
            {chain}
            {open}
            {adding}
            {reductions}
            {outWords}
            onOpen={openSlot}
            onAdd={startAdding}
            onSwitch={(slot, on) => send.setEffectOn(place, slot, on)}
            onMove={moveEffect}
          />
        </div>

        <div class="sound-module mix-open-module">
          <div class="sound-module-top">
            <span class="sound-label">{openName}</span>
            {#if typeof open === "number" && openEffect !== null}
              <button
                type="button"
                class="sound-hint sound-hint-button"
                title="Empties this slot"
                onclick={(event) => clickThenLetGo(event, removeEffect)}
              >
                Remove
              </button>
            {:else}
              <span class="sound-hint">{openHint}&nbsp;</span>
            {/if}
          </div>
          <div class="mix-open">
            {#if open === "add"}
              <MixAdd onChoose={addEffect} />
            {:else if typeof open === "number" && openEffect !== null}
              {#key `${place} ${open} ${openEffect.kind}`}
                {@const slot = open}
                {@const effect = openEffect}
                {@const reduction = reductions[slot - 1] ?? 0}
                {#if effect.kind === "compressor"}
                  <MixCompressor {place} {slot} {effect} view={mix} {reduction} {picked} {onPick} {send} />
                {:else if effect.kind === "gate"}
                  <MixGate {place} {slot} {effect} view={mix} {reduction} {picked} {onPick} {send} />
                {:else if effect.kind === "eq"}
                  <MixEq {place} {slot} {effect} {picked} {onPick} {send} />
                {:else if effect.kind === "filter"}
                  <MixFilter {place} {slot} {effect} view={mix} {picked} {onPick} {send} />
                {:else if effect.kind === "saturation"}
                  <MixSaturation {place} {slot} {effect} {picked} {onPick} {send} />
                {:else if effect.kind === "chorus"}
                  <MixChorus {place} {slot} {effect} {picked} {onPick} {send} />
                {:else if effect.kind === "delay"}
                  <MixDelay {place} {slot} {effect} {picked} {onPick} {send} />
                {:else if effect.kind === "reverb"}
                  <MixReverb {place} {slot} {effect} {picked} {onPick} {send} />
                {:else if effect.kind === "volume-shaper"}
                  <MixShaper {place} {slot} {effect} {picked} {onPick} {send} />
                {/if}
              {/key}
            {:else if place === "master"}
              <MixLimiter
                limiter={mix.master.limiter}
                peaks={masterPeaks}
                cuts={masterCuts}
                reduction={meters?.limiter_reduction_db ?? 0}
                lufs={meters?.short_term_lufs ?? -70}
                truePeak={heldTruePeak}
                {picked}
                {onPick}
                {send}
              />
            {:else}
              {#key place}
                <MixOut
                  {place}
                  view={mix}
                  volume={outVolume}
                  {picked}
                  {onPick}
                  onVolume={(volume) => setVolume(place, volume)}
                  {send}
                />
              {/key}
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
</section>
