<!-- The start page, "Stage": a plain dark stage, lit by note family, like the
     display on a piece of studio gear. A held key lights up at once and
     fades out slowly when let go, like a lamp cooling down. The page powers
     on from the top down. Its tabs share the
     keyboard: Play shows the notes held, Compose adds its tape, outlines
     the chord keys and guides the melody keys, and Produce is a placeholder
     for now. Compose also has the sound panel. In every tab, right
     Shift and a lane under the keyboard show the looper, messages are
     typed out in the top bar, and the open song's name there opens the
     Songs panel (docs/adr/0038-song-files.md). The agent's panel docks on
     the right in every tab (docs/design.md#the-agent-mode). -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import {
    songKeysOf,
    type ChordGuide,
    type ComposeView,
    type Fit,
    type SongKeys,
  } from "./compose";
  import ComposeTape from "./ComposeTape.svelte";
  import { clickThenLetGo } from "./dom";
  import { colorForNote, NOTE_COLORS } from "./note-colors";
  import Keyboard from "./Keyboard.svelte";
  import LooperLane from "./LooperLane.svelte";
  import LoopTransport from "./LoopTransport.svelte";
  import {
    keysAcross,
    noteName,
    notesOf,
    type KeyLayout,
    type Platform,
  } from "./keyboard";
  import { beatsNow, type SoundSender, type SoundView } from "./sound";
  import SoundPanel from "./SoundPanel.svelte";
  import type { SongSender, SongsView } from "./songs";
  import SongsPanel from "./SongsPanel.svelte";
  import type { Picked } from "./sound-scales";
  import type { Tab } from "./tabs";
  import TopBar from "./TopBar.svelte";
  import AgentPanel from "./AgentPanel.svelte";
  import PracticeView from "./PracticeView.svelte";
  import type { PracticeMark, PracticeView as Practice } from "./practice";
  import type { Job } from "./agent";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    // What the top bar's buttons, the lane's snap switch and its volume
    // knobs do. `layer` counts from 1, and `volume` is in percent.
    onRenderClick: () => void;
    onSnapClick: () => void;
    onRecordClick: () => void;
    onExpressionChange: (key: string, held: boolean) => void;
    onExportLoop: () => void;
    onTempoChange: (tempo: number) => void;
    onLayerBarsChange: (layer: number, bars: number) => void;
    onVolumeChange: (layer: number, volume: number) => void;
    // What the lane's play and pause buttons do, for a layer and for the
    // whole loop.
    onLayerPlayingChange: (layer: number, playing: boolean) => void;
    onLoopPlayingChange: (playing: boolean) => void;
    // What Compose's controls do. `place` is a chord's place in the song.
    onKeyPick: (tonic: number, mode: "major" | "minor") => void;
    onChordClick: (place: number) => void;
    onPlayClick: () => void;
    onComposeEdit: (action: string, place: number) => void;
    // What the sound panel and the Songs panel send to Rust.
    soundSender: SoundSender;
    songSender: SongSender;
    // The agent's Undo may have changed the song: ask Rust for it again.
    onSongChanged: () => void;
  }

  let {
    platform,
    layout,
    onRenderClick,
    onSnapClick,
    onRecordClick,
    onExpressionChange,
    onExportLoop,
    onTempoChange,
    onLayerBarsChange,
    onVolumeChange,
    onLayerPlayingChange,
    onLoopPlayingChange,
    onKeyPick,
    onChordClick,
    onPlayClick,
    onComposeEdit,
    soundSender,
    songSender,
    onSongChanged,
  }: Props = $props();

  // How long the page takes to power on, in milliseconds, and how it moves:
  // slow, then fast, then slow.
  const POWER_ON_TIME = 520;
  const POWER_ON_EASING = "cubic-bezier(0.65, 0, 0.35, 1)";
  // How long a message stays up once it is typed out, and how long each
  // letter takes to type, in milliseconds.
  const STATUS_TIME = 4000;
  const LETTER_TIME = 16;
  // How far under the keyboard's panel the looper's lane sits, in pixels.
  const LANE_GAP = 10;

  // The notes the held keys started, chords included, lowest first, each
  // once.
  function heldNotes(started: Map<string, number[]>): number[] {
    const notes: number[] = [];
    for (const keyNotes of started.values()) {
      for (const note of keyNotes) {
        if (!notes.includes(note)) {
          notes.push(note);
        }
      }
    }
    notes.sort((low, high) => low - high);
    return notes;
  }

  // The keys held down right now, by place, like "KeyQ".
  const held = new SvelteSet<string>();
  // The keys the agent holds while it plays a phrase.
  const agentHeld = new SvelteSet<string>();
  // The keys a keys card marks, each with its label, until the player next
  // writes to the agent.
  const agentMarks = new SvelteMap<string, string>();
  // The keys practice marks while a try plays.
  const practiceMarks = new SvelteMap<string, PracticeMark>();
  // The practice the Tutor set, shown on the Play tab, or null.
  let practice: Practice | null = $state(null);
  // Each held key that plays notes, with the notes it started.
  const started = new SvelteMap<string, number[]>();
  // The sound, the tempo and the looper, or null until Rust has said. and
  // when it said, by performance.now().
  let sound: SoundView | null = $state(null);
  let soundAt = $state(0);
  // The kind new notes' first oscillator plays, shown in the Sound button.
  // F12 steps it.
  let wave = $derived.by(() => {
    if (sound === null) {
      return "";
    }
    return sound.oscillators[0].kind;
  });

  // The page's clock, by performance.now(). It ticks every frame while the
  // looper is busy or Compose's song plays (see clockMoving), so the
  // looper's lights, bar and playhead, and the chord keys the song lights,
  // move smoothly between Rust's answers, which come every 50 ms.
  let clock = $state(performance.now());
  let looperBusy = $derived.by(() => {
    if (sound === null) {
      return false;
    }
    return sound.looper.busy;
  });

  // How many beats into the count-in, the recording or the repeat playing
  // now the looper is, counted on from Rust's last answer to this frame.
  let looperBeats = $derived.by(() => {
    if (sound === null) {
      return 0;
    }
    const msSince = Math.max(0, clock - soundAt);
    return beatsNow(sound, msSince);
  });

  // Where the looper's lane sits, just under the keyboard's panel and only
  // as wide as the keys, or null until measured.
  let lanePlace: { left: number; top: number; width: number } | null =
    $state(null);

  function placeLane() {
    const page = root.getBoundingClientRect();
    const panelBox = panel.getBoundingClientRect();
    const keys = panel.querySelector<HTMLElement>(".keyboard");
    if (keys === null) {
      return;
    }
    const keysBox = keys.getBoundingClientRect();
    lanePlace = {
      left: keysBox.left - page.left,
      top: panelBox.bottom - page.top + LANE_GAP,
      width: keysBox.width,
    };
  }

  // The readout shows the notes held at the last press. While other keys are
  // still held, a let-go note stays there, dimmed, until the next press.
  // Once every key is up, the notes played last fade out together, slowly,
  // so a chord let go can still be read.
  let readoutNotes: number[] = $state([]);
  let notesHeldNow = $derived(heldNotes(started));
  let idle = $derived(notesHeldNow.length === 0);

  // The tab open now.
  let tab: Tab = $state("play");
  // Whether the agent's panel is open, and the panel, for Stop.
  let agentOpen = $state(false);
  let agentPanel: AgentPanel | undefined = $state();
  // The agent's job in the tab open now.
  let job: Job = $derived.by(() => {
    if (tab === "compose") {
      return "composer";
    }
    if (tab === "produce") {
      return "producer";
    }
    return "tutor";
  });
  // What Compose shows, or null until Rust has said, and when it said, by
  // performance.now().
  let compose: ComposeView | null = $state(null);
  let composeAt = $state(0);
  // The chord key pressed last, by place, like "KeyX".
  let tried: string | null = $state(null);
  // Whether the sound panel is open. It opens over Compose's tape.
  let soundPanelOpen = $state(false);
  // The sound panel's value picked for the arrow keys, if any.
  let picked: Picked | null = $state(null);
  let soundModifierHeld = $state(false);

  // The drum mode key's name as printed, for the top bar's DRUMS.
  let drumModeKeyName = $derived.by(() => {
    if (platform === "mac") {
      return "Right ⌥";
    }
    return "Right Ctrl";
  });

  // The songs and the one open, or null until Rust has said, and whether
  // the Songs panel is open. It opens in the tape's place in every tab, and
  // the sound panel closes for it.
  let songs: SongsView | null = $state(null);
  let songsOpen = $state(false);

  let songName = $derived.by(() => {
    if (songs === null) {
      return "";
    }
    return songs.open;
  });

  function switchSongsPanel() {
    songsOpen = !songsOpen;
    if (songsOpen) {
      soundPanelOpen = false;
    }
    letGo();
  }

  function switchSoundPanel() {
    soundPanelOpen = !soundPanelOpen;
    if (soundPanelOpen) {
      songsOpen = false;
    }
    letGo();
  }

  function pick(newPicked: Picked) {
    picked = newPicked;
  }

  function letGo() {
    picked = null;
  }

  // Whether Compose's song is playing.
  let songPlaying = $derived.by(() => {
    if (compose === null) {
      return false;
    }
    return compose.playing !== null;
  });

  let clockMoving = $derived.by(() => {
    if (looperBusy) {
      return true;
    }
    return songPlaying;
  });

  $effect(() => {
    if (!clockMoving) {
      return;
    }
    let frame = 0;
    function nextFrame() {
      clock = performance.now();
      frame = requestAnimationFrame(nextFrame);
    }
    frame = requestAnimationFrame(nextFrame);
    return () => {
      cancelAnimationFrame(frame);
    };
  });

  // While Compose's song plays, the chord keys it lights: the one for the
  // chord playing, and the one for the chord after it, filling up as it
  // comes. Null while it isn't playing.
  let songKeys: SongKeys | null = $derived.by(() => {
    if (tab !== "compose") {
      return null;
    }
    if (compose === null) {
      return null;
    }
    const sinceAnswer = Math.max(0, clock - composeAt);
    return songKeysOf(compose, sinceAnswer);
  });

  // Compose outlines the chord keys by how well they fit the song's key.
  let keyboardGuide = $derived.by(() => {
    const none: Record<string, ChordGuide> = {};
    if (tab !== "compose") {
      return none;
    }
    if (compose === null) {
      return none;
    }
    return compose.guide;
  });

  // Compose dims the melody keys whose notes are outside the song's key.
  let melodyGuide = $derived.by(() => {
    const none: Record<string, Fit> = {};
    if (tab !== "compose") {
      return none;
    }
    if (compose === null) {
      return none;
    }
    return compose.melody;
  });

  // The notes of the chord keys held now, as pitches from 0 (C) to 11 (B),
  // each once. In Compose, once a key is picked, the melody keys that play
  // them fill with their color.
  let chordNotes = $derived.by(() => {
    const pitches: number[] = [];
    if (tab !== "compose") {
      return pitches;
    }
    if (compose === null) {
      return pitches;
    }
    if (compose.key === null) {
      return pitches;
    }
    for (const code of held) {
      if (layout.chords[code] === undefined) {
        continue;
      }
      for (const note of notesOf(code, layout)) {
        const pitch = note % 12;
        if (!pitches.includes(pitch)) {
          pitches.push(pitch);
        }
      }
    }
    return pitches;
  });

  // In Compose, Return adds the chord played last to the song and Delete
  // takes the last one off. main.ts sends them to Rust.
  let keyboardActions = $derived.by(() => {
    const actions: Record<string, string> = {};
    if (tab === "compose") {
      actions.Enter = "add";
      actions.Backspace = "undo";
    }
    return actions;
  });

  function openTab(newTab: Tab) {
    tab = newTab;
    // Another tab shows its own screen, so the Songs panel closes.
    songsOpen = false;
    // The sound panel is Compose's, so what it picked is let go.
    letGo();
    // A tab's screen can move the keyboard, so the lane follows it once
    // the page is drawn.
    tick().then(placeLane);
  }

  // What guidance says about the chord tried last.
  let triedGuide = $derived.by(() => {
    if (tried === null) {
      return null;
    }
    const keyGuide = keyboardGuide[tried];
    if (keyGuide === undefined) {
      return null;
    }
    return keyGuide;
  });

  // The root of the chord tried last, its lowest note, for its color.
  let triedRoot = $derived.by(() => {
    if (tried === null) {
      return null;
    }
    return notesOf(tried, layout)[0];
  });

  function isLetGo(note: number): boolean {
    if (idle) {
      return false;
    }
    return !notesHeldNow.includes(note);
  }

  // Messages are typed out in the top bar, one letter at a time.
  let messageText = $state("");
  let messageShown = $state(false);
  let typingTimer = 0;
  let messageTimer = 0;

  let root: HTMLElement;
  let scanLine: HTMLElement;
  let panel: HTMLElement;

  onMount(() => {
    const resizeWatcher = new ResizeObserver(() => {
      placeLane();
    });
    resizeWatcher.observe(root);
    resizeWatcher.observe(panel);
    placeLane();
    powerOn();
    return () => {
      resizeWatcher.disconnect();
      window.clearInterval(typingTimer);
      window.clearTimeout(messageTimer);
    };
  });

  function powerOn() {
    // The page powers on from the top down...
    root.animate(
      [{ clipPath: "inset(0 0 100% 0)" }, { clipPath: "inset(0 0 0% 0)" }],
      {
        duration: POWER_ON_TIME,
        easing: POWER_ON_EASING,
      },
    );

    // ...behind a line of light that sweeps down along its edge, then
    // goes out.
    const bottom = `translateY(${root.clientHeight}px)`;
    scanLine.animate(
      [
        { transform: "translateY(0px)", opacity: 1 },
        { transform: bottom, opacity: 1 },
      ],
      { duration: POWER_ON_TIME, easing: POWER_ON_EASING },
    );
    scanLine.animate(
      [
        { transform: bottom, opacity: 1 },
        { transform: bottom, opacity: 0 },
      ],
      { duration: 220, delay: POWER_ON_TIME, easing: "ease-out" },
    );

    // Then each key flashes once, from left to right, like gear testing
    // its lights as it starts up.
    const keyElements = Array.from(root.querySelectorAll<HTMLElement>(".key"));
    for (const keyElement of keyElements) {
      keyElement.animate(
        [
          {
            offset: 0,
            backgroundColor: `rgba(${NOTE_COLORS.C.accent}, 0.28)`,
            borderColor: `rgba(${NOTE_COLORS.C.accent}, 0.9)`,
            boxShadow: `0 0 14px rgba(${NOTE_COLORS.C.accent}, 0.4)`,
          },
        ],
        {
          duration: 600,
          delay: 380 + keyElement.offsetLeft * 0.45,
          easing: "ease-out",
        },
      );
    }
  }

  // How ui/main.ts drives the page. `code` is a key's place, like "KeyQ".

  export function showSoundModifier(held: boolean) {
    soundModifierHeld = held;
  }

  export function showLayout(next: KeyLayout) {
    layout = next;
  }

  // Escape: stops the agent while it works, then closes Sound or Songs,
  // then the agent's panel. Only once none is left does it reach the looper.
  export function closePanel(): boolean {
    if (agentPanel !== undefined && agentPanel.isWorking()) {
      agentPanel.stop();
      return true;
    }
    if (practice !== null && practice.running === true) {
      invoke("stop_practice");
      return true;
    }
    if (soundPanelOpen || songsOpen) {
      soundPanelOpen = false;
      songsOpen = false;
      letGo();
      return true;
    }
    if (agentOpen) {
      agentOpen = false;
      return true;
    }
    return false;
  }

  // The agent pressed or let go of `code`, playing a phrase. Its key lights
  // as the agent's, and its notes show in Play's readout.
  export function showAgentKey(code: string, down: boolean) {
    const place = `agent ${code}`;
    if (!down) {
      agentHeld.delete(code);
      started.delete(place);
      return;
    }
    agentHeld.add(code);
    const notes = notesOf(code, layout);
    if (notes.length > 0) {
      started.set(place, notes);
      readoutNotes = heldNotes(started);
    }
  }

  // The agent marked `keys` on the keyboard, with a keys card.
  export function showAgentMarks(keys: { key: string; label: string }[]) {
    agentMarks.clear();
    for (const mark of keys) {
      agentMarks.set(mark.key, mark.label);
    }
  }

  function clearAgentMarks() {
    agentMarks.clear();
  }

  // The Tutor set a piece, or practice changed. A new piece opens the Play
  // tab, where practice shows.
  export function showPractice(view: Practice) {
    const isNew = view.set && (practice === null || !practice.set);
    if (view.set) {
      practice = view;
    } else {
      practice = null;
      practiceMarks.clear();
    }
    if (isNew && tab !== "play") {
      openTab("play");
    }
  }

  export function showPracticeMarks(marks: PracticeMark[]) {
    practiceMarks.clear();
    for (const mark of marks) {
      practiceMarks.set(mark.key, mark);
    }
  }

  // A try ended. With coaching on, the agent hears about it by itself.
  function onPracticeTry(number: number, coach: boolean) {
    if (!coach || agentPanel === undefined) {
      return;
    }
    agentOpen = true;
    agentPanel.coach(number);
  }

  export function keyDown(code: string) {
    held.add(code);
    const notes = notesOf(code, layout);
    if (layout.chords[code] !== undefined) {
      tried = code;
    }
    // A press shows the notes held now, lit, at once.
    if (notes.length > 0) {
      started.set(code, notes);
      readoutNotes = heldNotes(started);
    }
  }

  export function keyUp(code: string) {
    held.delete(code);
    started.delete(code);
  }

  // Every key came up at once: Cmd was pressed, or the window lost focus.
  export function releaseAll() {
    held.clear();
    started.clear();
  }

  // An arrow key went down: it steps the value picked in the sound panel,
  // if any. `steps` is 1 or -1 for right and left, 10 or -10 for up and
  // down.
  export function stepPicked(steps: number) {
    if (picked === null) {
      return;
    }
    picked.step(steps);
  }

  // Whether Compose is open, so main.ts knows what Return and Delete do.
  export function wantsPerformance(): boolean {
    if (tab === "compose") {
      return true;
    }
    return agentOpen;
  }

  export function isComposing(): boolean {
    return tab === "compose";
  }

  export function showCompose(view: ComposeView) {
    compose = view;
    composeAt = performance.now();
  }

  export function showSongs(view: SongsView) {
    songs = view;
  }

  export function showSound(view: SoundView) {
    sound = view;
    soundAt = performance.now();
  }

  // A message for the player, like where a file was saved.
  export function showStatus(message: string) {
    window.clearInterval(typingTimer);
    window.clearTimeout(messageTimer);
    messageText = "";
    messageShown = true;

    let typed = 0;
    typingTimer = window.setInterval(() => {
      typed += 1;
      messageText = message.slice(0, typed);
      if (typed < message.length) {
        return;
      }
      window.clearInterval(typingTimer);
      messageTimer = window.setTimeout(() => {
        messageShown = false;
      }, STATUS_TIME);
    }, LETTER_TIME);
  }
</script>

<div
  class="screen stage"
  class:is-playing={!idle}
  bind:this={root}
>
  <!-- The line of light that sweeps down as the page powers on. -->
  <div class="stage-scan-line" bind:this={scanLine}></div>

  <!-- The button for the sound panel: a small wave, "Sound" and the kind
       oscillator 1 plays, which F12 steps. -->
  {#snippet endButtons()}
    <button
      type="button"
      class="action top-button"
      class:is-open={soundPanelOpen}
      title="The sound panel (oscillator 1's kind: F12 steps it)"
      onclick={(event) => clickThenLetGo(event, switchSoundPanel)}
    >
      <svg
        class="tab-icon"
        viewBox="0 0 16 16"
        width="16"
        height="16"
        aria-hidden="true"
      >
        <path
          d="M1 8c1.5-5 3-5 4.5 0s3 5 4.5 0 3-5 4.5 0"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
        />
      </svg>
      <span class="action-name">Sound</span>
      <span class="action-value">{wave}</span>
    </button>
  {/snippet}

  <TopBar
    {tab}
    {songName}
    {songsOpen}
    onSongClick={switchSongsPanel}
    onTabClick={openTab}
    {onRenderClick}
    message={messageText}
    {messageShown}
    {endButtons}
    {agentOpen}
    onAgentClick={() => {
      agentOpen = !agentOpen;
    }}
    drumMode={layout.drum_mode}
    drumModeKey={drumModeKeyName}
  />

  <main
    class="screen-body"
    class:is-composing={tab === "compose"}
    class:has-agent={agentOpen}
  >
    <div class="screen-left">
      <div class="workspace">
        {#if songsOpen}
          <SongsPanel
            {songs}
            keysAcross={keysAcross(platform)}
            send={songSender}
          />
        {:else if soundPanelOpen && sound !== null}
          <SoundPanel
            {sound}
            keysAcross={keysAcross(platform)}
            adjustModifier={platform === "mac" ? "Left ⌘" : "Left Alt"}
            {picked}
            onPick={pick}
            onLetGo={letGo}
            send={soundSender}
          />
        {:else}
          {#if tab === "play" && practice !== null}
            <PracticeView view={practice} onTry={onPracticeTry} />
          {:else if tab === "play"}
            <!-- The held notes, in big glowing dots. -->
            <div class="stage-readout" class:is-idle={idle}>
              {#each readoutNotes as note (note)}
                <span
                  class="stage-note"
                  class:is-let-go={isLetGo(note)}
                  data-note={note}
                  style:--accent={colorForNote(note).accent}
                >
                  <span class="stage-note-pitch">{noteName(note).pitch}</span>
                  <span class="stage-note-octave">{noteName(note).octave}</span>
                </span>
              {/each}
            </div>
          {/if}

          <!-- Compose shows its tape where Play shows the notes held, with the
           sound panel over it while open, and Produce a dev panel until it is
           designed. -->
          {#if tab === "compose"}
            <ComposeTape
              {layout}
              view={compose}
              answeredAt={composeAt}
              tried={triedGuide}
              {triedRoot}
              keysAcross={keysAcross(platform)}
              {onKeyPick}
              {onChordClick}
              {onPlayClick}
              onEdit={onComposeEdit}
            />
          {/if}
          {#if tab === "produce"}
            <section class="dev-panel">
              <p class="dev-panel-title">Dev panel: Produce, not designed yet</p>
              <p class="dev-row dev-quiet">
                Tracks, the mixer and effects come here later. The keyboard still
                plays.
              </p>
            </section>
          {/if}
        {/if}
      </div>

      <!-- The keyboard, with no box around it. -->
      <div class="stage-panel" bind:this={panel}>
        {#if sound !== null}
          <LoopTransport {sound} onRecord={onRecordClick} onTempo={onTempoChange} onExpression={onExpressionChange} onExport={onExportLoop} />
        {/if}
        <Keyboard
          {platform}
          {layout}
          {agentHeld}
          {agentMarks}
          {practiceMarks}
          {held}
          {sound}
          {looperBeats}
          guide={keyboardGuide}
          liveGuide={tab === "compose" ? (compose?.live_guide ?? {}) : {}}
          compatible={tab === "compose" ? (compose?.compatible ?? {}) : {}}
          melodyNext={tab === "compose" ? (compose?.melody_next ?? {}) : {}}
          melody={melodyGuide}
          {chordNotes}
          song={songKeys}
          actions={keyboardActions}
          stepping={picked !== null && soundModifierHeld}
        />
      </div>
    </div>

    <AgentPanel
      bind:this={agentPanel}
      visible={agentOpen}
      {job}
      {onSongChanged}
      onSent={clearAgentMarks}
    />
  </main>

  <!-- The lane shows only once the looper starts: until then nothing is
       drawn under the keyboard. -->
  {#if sound !== null && sound.looper.state !== "idle"}
    {#if lanePlace !== null}
      <LooperLane
        {sound}
        beats={looperBeats}
        left={lanePlace.left}
        top={lanePlace.top}
        width={lanePlace.width}
        {onSnapClick}
        {onVolumeChange}
        {onLayerPlayingChange}
        {onLoopPlayingChange}
        {onLayerBarsChange}
      />
    {/if}
  {/if}
</div>
