<!-- The sound panel's Connections page (docs/design.md#sound-panel): the
     song's routing list (docs/adr/0031-lfos-and-a-routing-list.md). Each
     connection has a source move a target by an amount, maybe scaled by
     another source. The list on the left shows those in use and opens one;
     the rest of the page is the open one. Changes move notes already
     sounding. -->
<script lang="ts">
  import { untrack } from "svelte";
  import {
    amountScale,
    isPitch,
    lfoHeight,
    lfoIndexOf,
    movesLive,
    nameWords,
    shapeGlyph,
    speedWords,
    targetUnit,
    TIME,
    type Picked,
    type Setting,
  } from "./sound-scales";
  import { CONNECTION_COUNT, type ConnectionView, type SoundSender, type SoundView } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    onLetGo: () => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, onLetGo, send }: Props = $props();

  // The sources that are not LFOs, and the targets, grouped as on screen,
  // each with the words on its tile and the name Rust knows it by.
  const OTHER_SOURCES = ["vibrato key", "note height", "random", "loudness envelope", "filter envelope"];
  const NOTE_TARGETS = [
    { words: "pitch", target: "pitch" },
    { words: "loudness", target: "loudness" },
    { words: "pan", target: "pan" },
    { words: "attack", target: "loudness attack" },
    { words: "decay", target: "loudness decay" },
    { words: "sustain", target: "loudness sustain" },
    { words: "release", target: "loudness release" },
    { words: "copies", target: "copies" },
    { words: "detune", target: "detune" },
    { words: "spread", target: "spread" },
    { words: "glide time", target: "glide time" },
  ];
  const OSCILLATOR_TARGETS = [
    { words: "level 1", target: "osc 1 level" },
    { words: "level 2", target: "osc 2 level" },
    { words: "level 3", target: "osc 3 level" },
    { words: "pitch 1", target: "osc 1 pitch" },
    { words: "pitch 2", target: "osc 2 pitch" },
    { words: "pitch 3", target: "osc 3 pitch" },
    { words: "width 1", target: "osc 1 width" },
    { words: "width 2", target: "osc 2 width" },
    { words: "width 3", target: "osc 3 width" },
    { words: "kind 1", target: "osc 1 kind" },
    { words: "kind 2", target: "osc 2 kind" },
    { words: "kind 3", target: "osc 3 kind" },
  ];
  const FILTER_TARGETS = [
    { words: "cutoff", target: "cutoff" },
    { words: "resonance", target: "resonance" },
    { words: "amount", target: "filter amount" },
    { words: "attack", target: "filter attack" },
    { words: "decay", target: "filter decay" },
    { words: "sustain", target: "filter sustain" },
    { words: "release", target: "filter release" },
    { words: "key tracking", target: "key tracking" },
    { words: "type", target: "filter type" },
  ];

  // Moves shows one group of targets at a time, picked by its name. The
  // LFOs' speeds are the last group.
  type Group = "notes" | "oscillators" | "filter" | "lfo speed";

  const GROUPS: { group: Group; name: string }[] = [
    { group: "notes", name: "Notes" },
    { group: "oscillators", name: "Oscillators" },
    { group: "filter", name: "Filter" },
    { group: "lfo speed", name: "LFO speed" },
  ];

  function isAmong(targets: { words: string; target: string }[], target: string): boolean {
    for (const each of targets) {
      if (each.target === target) {
        return true;
      }
    }
    return false;
  }

  // The group `target` is in.
  function groupOf(target: string): Group {
    if (isAmong(NOTE_TARGETS, target)) {
      return "notes";
    }
    if (isAmong(OSCILLATOR_TARGETS, target)) {
      return "oscillators";
    }
    if (isAmong(FILTER_TARGETS, target)) {
      return "filter";
    }
    return "lfo speed";
  }

  // Where each connection in use is in the list of all of them, in order.
  function usedPlaces(connections: ConnectionView[]): number[] {
    const places: number[] = [];
    for (let place = 0; place < connections.length; place += 1) {
      if (connections[place].source !== "none") {
        places.push(place);
      }
    }
    return places;
  }

  let used = $derived(usedPlaces(sound.connections));

  // Where the first connection in use is, or 0 when none is.
  function firstUsed(): number {
    const places = usedPlaces(sound.connections);
    if (places.length === 0) {
      return 0;
    }
    return places[0];
  }

  // The connection open now, by its place among all of them. It opens on
  // the first in use. One whose source is "none" is not in use, and the
  // page then shows no connection.
  let open = $state(untrack(firstUsed));
  let connection = $derived(sound.connections[open]);
  let isOpen = $derived(connection.source !== "none");

  // The group Moves shows: the open connection's target's group, until
  // another group's name is clicked, and again once another connection
  // opens.
  let browsed: Group | null = $state(null);
  let targetGroup = $derived(groupOf(connection.target));
  let shownGroup = $derived.by(() => {
    if (browsed !== null) {
      return browsed;
    }
    return targetGroup;
  });

  function showGroup(group: Group) {
    browsed = group;
  }

  function openConnection(place: number) {
    open = place;
    browsed = null;
    onLetGo();
  }

  // Whether the list's row for the connection at `place` is the open one.
  function isOpenRow(place: number): boolean {
    if (!isOpen) {
      return false;
    }
    return place === open;
  }

  function isUsedSource(source: string): boolean {
    for (const each of sound.connections) {
      if (each.source === source) {
        return true;
      }
    }
    return false;
  }

  // The open connection as it is now, with `change` made to it, sent as a
  // plain copy.
  function sendConnection(change: Partial<ConnectionView>) {
    const now = $state.snapshot(sound.connections[open]);
    send.setConnection(open, { ...now, ...change });
  }

  // "+ New connection" fills the first free place with the first LFO no
  // connection uses yet, moving the pitch by 0, so nothing changes until
  // its amount does.
  function addConnection() {
    const free = sound.connections.findIndex((each) => each.source === "none");
    if (free < 0) {
      return;
    }
    // With every LFO already in use, LFO 1 again.
    let source = "lfo 1";
    for (let index = 0; index < sound.lfos.length; index += 1) {
      const name = `lfo ${index + 1}`;
      if (!isUsedSource(name)) {
        source = name;
        break;
      }
    }
    const added: ConnectionView = {
      source,
      target: "pitch",
      amount: 0,
      one_way: false,
      snap: false,
      scaled_by: "none",
      unit: "cents",
    };
    send.setConnection(free, added);
    openConnection(free);
  }

  // The x takes the open connection out of use, and the next one in use
  // opens, or else the one before it.
  function removeConnection() {
    const removed: ConnectionView = {
      source: "none",
      target: "pitch",
      amount: 0,
      one_way: false,
      snap: false,
      scaled_by: "none",
      unit: "cents",
    };
    send.setConnection(open, removed);
    const place = used.indexOf(open);
    let next = open;
    if (place + 1 < used.length) {
      next = used[place + 1];
    } else if (place > 0) {
      next = used[place - 1];
    }
    openConnection(next);
  }

  // A new target keeps the amount's place on its row, in the new unit.
  function setTarget(target: string) {
    const share = amountScale(connection.target).share(connection.amount);
    const amount = amountScale(target).at(share);
    sendConnection({ target, amount });
  }

  const amount: Setting = {
    id: "connection amount",
    name: "Amount",
    // The scale comes with the target, so it is worked out each time.
    get scale() {
      return amountScale(connection.target);
    },
    now: () => connection.amount,
    set: (newAmount) => sendConnection({ amount: newAmount }),
  };

  // A shared LFO's speed is the same for every note, so only another
  // shared LFO can move it.
  function isSharedLfo(source: string): boolean {
    const index = lfoIndexOf(source);
    if (index === null) {
      return false;
    }
    return !sound.lfos[index].per_note;
  }

  function cannotMoveSpeed(index: number): boolean {
    if (sound.lfos[index].per_note) {
      return false;
    }
    return !isSharedLfo(connection.source);
  }

  let unit = $derived(targetUnit(connection.target));
  let unitWords = $derived.by(() => {
    if (unit === "%") {
      return "in percent";
    }
    return `in ${unit}`;
  });

  // The picture of the move, 153 by 64: how far the target is moved over
  // a while, both ways around the middle line or one way up from the
  // bottom one.
  const LEFT = 6;
  const RIGHT = 147;
  const TOP = 8;
  const BOTTOM = 48;
  const MIDDLE = (TOP + BOTTOM) / 2;
  const POINTS = 300;

  // The vibrato key in the picture: pressed, held, then let go, rising
  // and falling between, as shares of the way across.
  const PRESS = 0.18;
  const HELD = 0.34;
  const LET_GO = 0.75;
  const GONE = 0.91;

  function keyAt(along: number): number {
    if (along < PRESS) {
      return 0;
    }
    if (along < HELD) {
      return (along - PRESS) / (HELD - PRESS);
    }
    if (along < LET_GO) {
      return 1;
    }
    if (along < GONE) {
      return 1 - (along - LET_GO) / (GONE - LET_GO);
    }
    return 0;
  }

  // Random in the picture: a new number for each of a few notes.
  const RANDOM_NOTES = [0.4, -0.7, 0.85, -0.2, 0.6, -0.9];

  // An envelope in the picture: one note, its key coming up at KEY_UP, each
  // stage up to STAGE wide on the time curve. From 0 to 1.
  const KEY_UP = 0.7;
  const STAGE = 0.3;

  function envelopeAt(along: number, attackMs: number, decayMs: number, sustain: number, releaseMs: number): number {
    const level = sustain / 100;
    const peak = TIME.share(attackMs) * STAGE;
    const decayEnd = peak + TIME.share(decayMs) * STAGE;
    if (along < peak) {
      return along / peak;
    }
    if (along < decayEnd) {
      return 1 - ((along - peak) / (decayEnd - peak)) * (1 - level);
    }
    if (along < KEY_UP) {
      return level;
    }
    const release = TIME.share(releaseMs) * STAGE;
    if (along < KEY_UP + release) {
      return level * (1 - (along - KEY_UP) / release);
    }
    return 0;
  }

  // How many cycles of an LFO the picture shows: a few, or many while
  // something scales it, so the scaling shows.
  function cyclesShown(index: number): number {
    if (connection.scaled_by !== "none") {
      return 9;
    }
    if (sound.lfos[index].shape === "steps") {
      return 1;
    }
    return 2;
  }

  // Where `source` reads, from -1 to 1, `along` of the way across the
  // picture. Every source is read this way, as the engine reads them
  // (engine/src/voice.cpp).
  function readingAt(source: string, along: number): number {
    const index = lfoIndexOf(source);
    if (index !== null) {
      const lfo = sound.lfos[index];
      return lfoHeight(lfo, lfo.start / 100 + along * cyclesShown(index));
    }
    if (source === "vibrato key") {
      return 2 * keyAt(along) - 1;
    }
    if (source === "note height") {
      return 2 * along - 1;
    }
    if (source === "random") {
      let note = Math.floor(along * RANDOM_NOTES.length);
      if (note > RANDOM_NOTES.length - 1) {
        note = RANDOM_NOTES.length - 1;
      }
      return RANDOM_NOTES[note];
    }
    if (source === "loudness envelope") {
      const loudness = sound.loudness;
      const level = envelopeAt(along, loudness.attack_ms, loudness.decay_ms, loudness.sustain, loudness.release_ms);
      return 2 * level - 1;
    }
    if (source === "filter envelope") {
      const filter = sound.filter;
      const level = envelopeAt(along, filter.attack_ms, filter.decay_ms, filter.sustain, filter.release_ms);
      return 2 * level - 1;
    }
    return 0;
  }

  // The move at `along`, from -1 to 1 both ways or 0 to 1 one way: the
  // source's reading, one way if so, times the scaling source's, read one
  // way (engine/src/routing.cpp).
  function moveAt(along: number): number {
    let move = readingAt(connection.source, along);
    if (connection.one_way) {
      move = (move + 1) / 2;
    }
    if (connection.scaled_by !== "none") {
      const scale = (readingAt(connection.scaled_by, along) + 1) / 2;
      move = move * scale;
    }
    return move;
  }

  let movePath = $derived.by(() => {
    if (!isOpen) {
      return "";
    }
    const points: string[] = [];
    for (let point = 0; point <= POINTS; point += 1) {
      const along = point / POINTS;
      const x = LEFT + (RIGHT - LEFT) * along;
      let y = MIDDLE - moveAt(along) * (MIDDLE - TOP);
      if (connection.one_way) {
        y = BOTTOM - moveAt(along) * (BOTTOM - TOP);
      }
      points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  });

  // What the picture's words say: when the vibrato key is held, how fast
  // an LFO goes, or what the source follows.
  let pictureWords = $derived.by(() => {
    if (!isOpen) {
      return { x: LEFT, words: "" };
    }
    const shiftX = LEFT + PRESS * (RIGHT - LEFT) + 6;
    if (connection.scaled_by === "vibrato key") {
      return { x: shiftX, words: "SHIFT HELD" };
    }
    const index = lfoIndexOf(connection.source);
    if (index !== null) {
      return { x: LEFT, words: speedWords(sound.lfos[index]).toUpperCase() };
    }
    if (connection.source === "vibrato key") {
      return { x: shiftX, words: "SHIFT HELD" };
    }
    if (connection.source === "note height") {
      return { x: LEFT, words: "LOW TO HIGH NOTES" };
    }
    if (connection.source === "random") {
      return { x: LEFT, words: "EACH NOTE" };
    }
    return { x: LEFT, words: "ONE NOTE" };
  });

  let snapsPitch = $derived(isPitch(connection.target));
</script>

<!-- The tiles of every source, for Moved by and Scaled by. -->
{#snippet sourceTiles(chosen: string, choose: (source: string) => void)}
  <div class="sound-tiles is-lfos">
    {#each sound.lfos as lfo, index}
      <button
        type="button"
        class="sound-tile is-lfo"
        class:is-chosen={chosen === `lfo ${index + 1}`}
        onclick={(event) => clickThenLetGo(event, () => choose(`lfo ${index + 1}`))}
      >
        <span class="sound-list-number">{index + 1}</span>
        <svg class="sound-glyph" viewBox="0 0 20 12"><path d={shapeGlyph(lfo.shape)} /></svg>
      </button>
    {/each}
  </div>
  <div class="sound-tiles is-words">
    {#each OTHER_SOURCES as source (source)}
      <button
        type="button"
        class="sound-tile"
        class:is-chosen={chosen === source}
        onclick={(event) => clickThenLetGo(event, () => choose(source))}
      >
        {source}
      </button>
    {/each}
  </div>
{/snippet}

<!-- A group of targets for Moves. -->
{#snippet targetChips(targets: { words: string; target: string }[])}
  <div class="sound-chips">
    {#each targets as each (each.target)}
      <button
        type="button"
        class="sound-tile"
        class:is-chosen={connection.target === each.target}
        onclick={(event) => clickThenLetGo(event, () => setTarget(each.target))}
      >
        {each.words}
      </button>
    {/each}
  </div>
{/snippet}

<div class="sound-modules" style:grid-template-columns="210fr 158fr 262fr 199fr 158fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Connections</span>
      <span class="sound-hint">{used.length} of {CONNECTION_COUNT}</span>
    </div>
    <div class="sound-list">
      {#each used as place, row (place)}
        <div class="sound-list-row is-connection" class:is-open={isOpenRow(place)}>
          <button
            type="button"
            class="sound-list-open"
            onclick={(event) => clickThenLetGo(event, () => openConnection(place))}
          >
            <span class="sound-list-number">{row + 1}</span>
            <span class="sound-list-words">
              {nameWords(sound.connections[place].source)}
              <b>→</b>
              {nameWords(sound.connections[place].target)}
            </span>
          </button>
          {#if isOpenRow(place)}
            <button
              type="button"
              class="sound-remove"
              title="Remove this connection"
              onclick={(event) => clickThenLetGo(event, removeConnection)}
            >
              <svg viewBox="0 0 10 10"><path d="M2 2 L 8 8 M8 2 L 2 8" /></svg>
            </button>
          {/if}
        </div>
      {/each}
      {#if used.length < CONNECTION_COUNT}
        <div class="sound-list-row is-connection is-new">
          <button type="button" class="sound-list-open" onclick={(event) => clickThenLetGo(event, addConnection)}>
            <span class="sound-list-number">+</span>
            <span class="sound-list-words">New connection</span>
          </button>
        </div>
      {/if}
    </div>
  </div>

  {#if isOpen}
    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">Moved by</span>
        <!-- Empty, to keep the name level with the others (sound-panel.css). -->
        <span class="sound-hint">&nbsp;</span>
      </div>
      {@render sourceTiles(connection.source, (source) => sendConnection({ source }))}
    </div>

    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">Moves</span>
        <span class="sound-hint">
          {#if movesLive(connection.target)}
            moves while it sounds
          {:else}
            read as each note starts
          {/if}
        </span>
      </div>
      <!-- The groups' names: the shown one lavender, the target's white. -->
      <div class="sound-group-tabs">
        {#each GROUPS as each (each.group)}
          <button
            type="button"
            class="sound-group-tab"
            class:is-open={shownGroup === each.group}
            class:has-target={targetGroup === each.group}
            onclick={(event) => clickThenLetGo(event, () => showGroup(each.group))}
          >
            {each.name}
          </button>
        {/each}
      </div>
      {#if shownGroup === "notes"}
        {@render targetChips(NOTE_TARGETS)}
      {:else if shownGroup === "oscillators"}
        {@render targetChips(OSCILLATOR_TARGETS)}
      {:else if shownGroup === "filter"}
        {@render targetChips(FILTER_TARGETS)}
      {:else}
        <div class="sound-chips">
          {#each sound.lfos as _, index}
            <button
              type="button"
              class="sound-tile is-number"
              class:is-chosen={connection.target === `lfo ${index + 1} speed`}
              class:is-dim={cannotMoveSpeed(index)}
              onclick={(event) => clickThenLetGo(event, () => setTarget(`lfo ${index + 1} speed`))}
            >
              {index + 1}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">How much</span>
        <span class="sound-hint">{unitWords}</span>
      </div>
      <div class="sound-drawing is-fixed" style:height="64px">
        <svg viewBox="0 0 153 64" preserveAspectRatio="xMidYMid meet">
          <path class="sound-lit-soft" d="M{LEFT} {TOP} L {RIGHT} {TOP}" />
          {#if !connection.one_way}
            <path class="sound-lit-soft is-faint" d="M{LEFT} {MIDDLE} L {RIGHT} {MIDDLE}" />
          {/if}
          <path class="sound-lit-soft" d="M{LEFT} {BOTTOM} L {RIGHT} {BOTTOM}" />
          <path class="sound-lit is-fine" d={movePath} />
          <text class="sound-axis" x={pictureWords.x} y="61">{pictureWords.words}</text>
        </svg>
      </div>
      <div class="sound-fields is-amount">
        <SoundValue setting={amount} dots={19} {picked} {onPick} bipolar />
      </div>
      <div class="sound-choice is-amount">
        <button
          type="button"
          class:is-chosen={!connection.one_way}
          onclick={(event) => clickThenLetGo(event, () => sendConnection({ one_way: false }))}
        >
          Both ways
        </button>
        <button
          type="button"
          class:is-chosen={connection.one_way}
          onclick={(event) => clickThenLetGo(event, () => sendConnection({ one_way: true }))}
        >
          One way
        </button>
      </div>
      <!-- Snap rounds a pitch move to whole half steps, so it plays notes. -->
      <button
        type="button"
        class="lane-snap sound-snap"
        class:is-off={!connection.snap}
        class:is-dim={!snapsPitch}
        onclick={(event) => clickThenLetGo(event, () => sendConnection({ snap: !connection.snap }))}
      >
        <span class="lane-snap-label">Snap to notes</span>
        <span class="lane-snap-value">
          {#if connection.snap}
            on
          {:else}
            off
          {/if}
        </span>
      </button>
    </div>

    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">Scaled by</span>
        <!-- Empty, to keep the name level with the others (sound-panel.css). -->
        <span class="sound-hint">&nbsp;</span>
      </div>
      <div class="sound-tiles is-words is-nothing">
        <button
          type="button"
          class="sound-tile"
          class:is-chosen={connection.scaled_by === "none"}
          onclick={(event) => clickThenLetGo(event, () => sendConnection({ scaled_by: "none" }))}
        >
          nothing
        </button>
      </div>
      {@render sourceTiles(connection.scaled_by, (source) => sendConnection({ scaled_by: source }))}
    </div>
  {:else}
    <div class="sound-module is-empty">
      <p class="sound-empty">No connection open. "+ New connection" adds one, moving the pitch by 0.</p>
    </div>
  {/if}
</div>
