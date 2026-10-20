<!-- The start page, "Stage": a dark stage of dots, lit in amber, like the
     dot-matrix display on a piece of studio gear. A held key lights up at
     once and fades out slowly when let go, like a lamp cooling down. Every
     note sends a beam of light up the grid of dots from its key
     (ui/beams.ts). The page powers on from the top down. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { createBeams, type Beams } from "./beams";
  import Keyboard from "./Keyboard.svelte";
  import { noteName, notesOf, type KeyLayout, type Platform } from "./keyboard";
  import TopBar from "./TopBar.svelte";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    // What the top bar's buttons do.
    onWaveClick: () => void;
    onRenderClick: () => void;
  }

  let { platform, layout, onWaveClick, onRenderClick }: Props = $props();

  // How long the page takes to power on, in milliseconds, and how it moves:
  // slow, then fast, then slow.
  const POWER_ON_TIME = 520;
  const POWER_ON_EASING = "cubic-bezier(0.65, 0, 0.35, 1)";
  // How long a message stays up once it is typed out, and how long each
  // letter takes to type, in milliseconds.
  const STATUS_TIME = 4000;
  const LETTER_TIME = 16;

  // The notes the `held` keys play, chords included, lowest first, each once.
  function heldNotes(held: Set<string>): number[] {
    const notes: number[] = [];
    for (const code of held) {
      for (const note of notesOf(code, layout)) {
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
  // The wave new notes play, like "saw", or "" until Rust has said.
  let wave = $state("");

  // The readout shows the notes held at the last press. While other keys are
  // still held, a let-go note stays there, dimmed, until the next press.
  // Once every key is up, the notes played last fade out together, slowly,
  // so a chord let go can still be read.
  let readoutNotes: number[] = $state([]);
  let notesHeldNow = $derived(heldNotes(held));
  let idle = $derived(notesHeldNow.length === 0);

  function isLetGo(note: number): boolean {
    if (idle) {
      return false;
    }
    return !notesHeldNow.includes(note);
  }

  // Messages are typed out at the bottom left, one letter at a time.
  let tickerText = $state("");
  let tickerShown = $state(false);
  let typingTimer = 0;
  let tickerTimer = 0;

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
    });
    resizeWatcher.observe(root);
    powerOn();
    return () => resizeWatcher.disconnect();
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
            backgroundColor: "rgba(255, 176, 72, 0.28)",
            borderColor: "rgba(255, 176, 72, 0.9)",
            boxShadow: "0 0 14px rgba(255, 176, 72, 0.4)",
          },
        ],
        { duration: 600, delay: 380 + keyElement.offsetLeft * 0.45, easing: "ease-out" },
      );
    }
  }

  // A key that plays a note or chord sends a beam up from the panel's top
  // edge, on the column of dots nearest the key's middle.
  function startBeam(code: string) {
    if (beams === null) {
      return;
    }
    if (notesOf(code, layout).length === 0) {
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
    beams.start(code, keyMiddle, panelTop);
  }

  function endBeam(code: string) {
    if (beams !== null) {
      beams.end(code);
    }
  }

  // How ui/main.ts drives the page. `code` is a key's place, like "KeyQ".

  export function keyDown(code: string) {
    held.add(code);
    startBeam(code);
    // A press shows the notes held now, lit, at once.
    const notes = heldNotes(held);
    if (notes.length > 0) {
      readoutNotes = notes;
    }
  }

  export function keyUp(code: string) {
    held.delete(code);
    endBeam(code);
  }

  // Every key came up at once: Cmd was pressed, or the window lost focus.
  export function releaseAll() {
    for (const code of held) {
      endBeam(code);
    }
    held.clear();
  }

  export function showWave(newWave: string) {
    wave = newWave;
  }

  // A message for the player, like where a file was saved.
  export function showStatus(message: string) {
    window.clearInterval(typingTimer);
    window.clearTimeout(tickerTimer);
    tickerText = "";
    tickerShown = true;

    let typed = 0;
    typingTimer = window.setInterval(() => {
      typed += 1;
      tickerText = message.slice(0, typed);
      if (typed < message.length) {
        return;
      }
      window.clearInterval(typingTimer);
      tickerTimer = window.setTimeout(() => {
        tickerShown = false;
      }, STATUS_TIME);
    }, LETTER_TIME);
  }
</script>

<div class="screen stage" class:is-playing={!idle} bind:this={root}>
  <canvas class="stage-beams" bind:this={canvas}></canvas>

  <!-- The line of light that sweeps down as the page powers on. -->
  <div class="stage-scan-line" bind:this={scanLine}></div>

  <TopBar {wave} {onWaveClick} {onRenderClick} />

  <main class="screen-body">
    <!-- The held notes, in big glowing dots. -->
    <div class="stage-readout" class:is-idle={idle}>
      {#each readoutNotes as note (note)}
        <span class="stage-note" class:is-let-go={isLetGo(note)} data-note={note}>
          <span class="stage-note-pitch">{noteName(note).pitch}</span>
          <span class="stage-note-octave">{noteName(note).octave}</span>
        </span>
      {/each}
    </div>

    <!-- The keyboard sits on a dark panel, like the top of a synth. -->
    <div class="stage-panel" bind:this={panel}>
      <Keyboard {platform} {layout} {held} {wave} />
    </div>
  </main>

  <!-- The text and cursor sit on one line, so no space comes between them. -->
  <p class="stage-ticker" class:is-shown={tickerShown}><span class="stage-ticker-text">{tickerText}</span><span class="stage-ticker-cursor"></span></p>
</div>
