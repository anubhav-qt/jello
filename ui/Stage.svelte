<!-- The start page, "Stage": a dark stage of dots, lit by note family, like the
     dot-matrix display on a piece of studio gear. A held key lights up at
     once and fades out slowly when let go, like a lamp cooling down. Every
     note sends a beam of light up the grid of dots from its key
     (ui/beams.ts). The page powers on from the top down. Its tabs share the
     keyboard: Play shows the notes held, Compose adds its tape, outlines
     the chord keys and guides the melody keys, and Produce is a placeholder
     for now. Compose also has the sound's dev panel. In every tab, right
     Shift and a lane under the keyboard show the looper, and messages are
     typed out in the top bar. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { createBeams, type Beams } from "./beams";
  import type { ChordGuide, ComposeView, Fit } from "./compose";
  import ComposeTape from "./ComposeTape.svelte";
  import { clickThenLetGo } from "./dom";
  import { colorForNote, DOT_CELL, NOTE_COLORS } from "./note-colors";
  import Keyboard from "./Keyboard.svelte";
  import LooperLane from "./LooperLane.svelte";
  import { keysAcross, noteName, notesOf, type KeyLayout, type Platform } from "./keyboard";
  import { beatsNow, type FilterEnvelope, type SoundView, type Unison } from "./sound";
  import SoundDevPanel from "./SoundDevPanel.svelte";
  import type { Tab } from "./tabs";
  import TopBar from "./TopBar.svelte";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    // What the top bar's buttons, the lane's snap switch and its volume
    // knobs do. `layer` counts from 1, and `volume` is in percent.
    onWaveClick: () => void;
    onRenderClick: () => void;
    onSnapClick: () => void;
    onVolumeChange: (layer: number, volume: number) => void;
    // What Compose's controls do. `place` is a chord's place in the song.
    onKeyPick: (tonic: number, mode: "major" | "minor") => void;
    onChordClick: (place: number) => void;
    onPlayClick: () => void;
    // What the sound's dev panel does. `cutoffHz` is middle C's cutoff,
    // and `resonance` is in percent.
    onFilterChange: (cutoffHz: number, resonance: number) => void;
    onFilterEnvelopeChange: (envelope: FilterEnvelope) => void;
    onUnisonChange: (unison: Unison) => void;
  }

  let {
    platform,
    layout,
    onWaveClick,
    onRenderClick,
    onSnapClick,
    onVolumeChange,
    onKeyPick,
    onChordClick,
    onPlayClick,
    onFilterChange,
    onFilterEnvelopeChange,
    onUnisonChange,
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
  // Each held key that plays notes, with the notes it started.
  const started = new SvelteMap<string, number[]>();
  // The wave, the tempo and the looper, or null until Rust has said, and
  // when it said, by performance.now().
  let sound: SoundView | null = $state(null);
  let soundAt = $state(0);
  // The wave new notes play, like "saw", or "" until Rust has said.
  let wave = $derived.by(() => {
    if (sound === null) {
      return "";
    }
    return sound.wave;
  });

  // The page's clock for the looper, by performance.now(). It ticks every
  // frame while the looper is busy, so its lights, bar and playhead move
  // smoothly between Rust's answers, which come every 50 ms.
  let looperClock = $state(performance.now());
  let looperBusy = $derived.by(() => {
    if (sound === null) {
      return false;
    }
    return sound.looper.busy;
  });

  $effect(() => {
    if (!looperBusy) {
      return;
    }
    let frame = 0;
    function nextFrame() {
      looperClock = performance.now();
      frame = requestAnimationFrame(nextFrame);
    }
    frame = requestAnimationFrame(nextFrame);
    return () => {
      cancelAnimationFrame(frame);
    };
  });

  // How many beats into the count-in, the recording or the repeat playing
  // now the looper is, counted on from Rust's last answer to this frame.
  let looperBeats = $derived.by(() => {
    if (sound === null) {
      return 0;
    }
    const msSince = Math.max(0, looperClock - soundAt);
    return beatsNow(sound, msSince);
  });

  // Where the looper's lane sits, just under the keyboard's panel and as
  // wide as it, or null until measured.
  let lanePlace: { left: number; top: number; width: number } | null = $state(null);

  function placeLane() {
    const page = root.getBoundingClientRect();
    const panelBox = panel.getBoundingClientRect();
    lanePlace = {
      left: panelBox.left - page.left,
      top: panelBox.bottom - page.top + LANE_GAP,
      width: panelBox.width,
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
  // What Compose shows, or null until Rust has said, and when it said, by
  // performance.now().
  let compose: ComposeView | null = $state(null);
  let composeAt = $state(0);
  // The chord key pressed last, by place, like "KeyX".
  let tried: string | null = $state(null);
  // Whether the sound's dev panel is open. It opens over Compose's tape.
  let soundPanelOpen = $state(false);

  function switchSoundPanel() {
    soundPanelOpen = !soundPanelOpen;
  }

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
  let canvas: HTMLCanvasElement;
  let scanLine: HTMLElement;
  let panel: HTMLElement;
  let beams: Beams | null = null;

  onMount(() => {
    beams = createBeams(canvas);
    const resizeWatcher = new ResizeObserver(() => {
      if (beams !== null) {
        beams.fit(root.clientWidth, root.clientHeight);
      }
      placeLane();
    });
    resizeWatcher.observe(root);
    resizeWatcher.observe(panel);
    placeLane();
    powerOn();
    return () => {
      resizeWatcher.disconnect();
      beams?.destroy();
      window.clearInterval(typingTimer);
      window.clearTimeout(messageTimer);
    };
  });

  function powerOn() {
    // The page powers on from the top down...
    root.animate([{ clipPath: "inset(0 0 100% 0)" }, { clipPath: "inset(0 0 0% 0)" }], {
      duration: POWER_ON_TIME,
      easing: POWER_ON_EASING,
    });

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
        { duration: 600, delay: 380 + keyElement.offsetLeft * 0.45, easing: "ease-out" },
      );
    }
  }

  // A key that plays a note or chord sends a beam up from the panel's top
  // edge, on the column of dots nearest the key's middle, in the color of
  // its lowest note.
  function startBeam(code: string, notes: number[]) {
    if (beams === null) {
      return;
    }
    if (notes.length === 0) {
      return;
    }
    const keyElement = root.querySelector<HTMLElement>(`.key[data-code="${code}"]`);
    if (keyElement === null) {
      return;
    }

    const page = root.getBoundingClientRect();
    const key = keyElement.getBoundingClientRect();
    const keyMiddle = key.left + key.width / 2 - page.left;
    const panelTop = panel.getBoundingClientRect().top - page.top;
    beams.start(code, keyMiddle, panelTop, notes[0]);
  }

  function endBeam(code: string) {
    if (beams !== null) {
      beams.end(code);
    }
  }

  // How ui/main.ts drives the page. `code` is a key's place, like "KeyQ".

  export function keyDown(code: string) {
    held.add(code);
    const notes = notesOf(code, layout);
    startBeam(code, notes);
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
    endBeam(code);
  }

  // Every key came up at once: Cmd was pressed, or the window lost focus.
  export function releaseAll() {
    for (const code of held) {
      endBeam(code);
    }
    held.clear();
    started.clear();
  }

  // Whether Compose is open, so main.ts knows what Return and Delete do.
  export function isComposing(): boolean {
    return tab === "compose";
  }

  export function showCompose(view: ComposeView) {
    compose = view;
    composeAt = performance.now();
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
  style:--dot-cell={`${DOT_CELL}px`}
  bind:this={root}
>
  <canvas class="stage-beams" bind:this={canvas}></canvas>

  <!-- The line of light that sweeps down as the page powers on. -->
  <div class="stage-scan-line" bind:this={scanLine}></div>

  <!-- Compose has a button for the sound's dev panel. -->
  {#snippet devButtons()}
    {#if tab === "compose"}
      <button
        type="button"
        class="dev-button"
        class:is-open={soundPanelOpen}
        onclick={(event) => clickThenLetGo(event, switchSoundPanel)}
      >
        Dev: sound
      </button>
    {/if}
  {/snippet}

  <TopBar {tab} onTabClick={openTab} {wave} {onWaveClick} {onRenderClick} message={messageText} {messageShown} {devButtons} />

  <main class="screen-body" class:is-composing={tab === "compose"}>
    {#if tab === "play"}
      <!-- The held notes, in big glowing dots. -->
      <div class="stage-readout" class:is-idle={idle}>
        {#each readoutNotes as note (note)}
          <span class="stage-note" class:is-let-go={isLetGo(note)} data-note={note} style:--accent={colorForNote(note).accent}>
            <span class="stage-note-pitch">{noteName(note).pitch}</span>
            <span class="stage-note-octave">{noteName(note).octave}</span>
          </span>
        {/each}
      </div>
    {/if}

    <!-- Compose shows its tape where Play shows the notes held, with the
         sound's dev panel over it while open, and Produce a dev panel until
         it is designed. -->
    {#if tab === "compose"}
      <ComposeTape
        view={compose}
        answeredAt={composeAt}
        tried={triedGuide}
        {triedRoot}
        keysAcross={keysAcross(platform)}
        {onKeyPick}
        {onChordClick}
        {onPlayClick}
      />
      {#if soundPanelOpen}
        {#if sound !== null}
          <SoundDevPanel
            filter={sound.filter}
            unison={sound.unison}
            keysAcross={keysAcross(platform)}
            {onFilterChange}
            onEnvelopeChange={onFilterEnvelopeChange}
            {onUnisonChange}
          />
        {/if}
      {/if}
    {/if}
    {#if tab === "produce"}
      <section class="dev-panel">
        <p class="dev-panel-title">Dev panel: Produce, not designed yet</p>
        <p class="dev-row dev-quiet">Tracks, the mixer and effects come here later. The keyboard still plays.</p>
      </section>
    {/if}

    <!-- The keyboard sits on a dark panel, like the top of a synth. -->
    <div class="stage-panel" bind:this={panel}>
      <Keyboard
        {platform}
        {layout}
        {held}
        {sound}
        {looperBeats}
        guide={keyboardGuide}
        melody={melodyGuide}
        {chordNotes}
        actions={keyboardActions}
      />
    </div>
  </main>

  {#if sound !== null}
    {#if lanePlace !== null}
      <LooperLane
        {sound}
        beats={looperBeats}
        left={lanePlace.left}
        top={lanePlace.top}
        width={lanePlace.width}
        {onSnapClick}
        {onVolumeChange}
      />
    {/if}
  {/if}
</div>
