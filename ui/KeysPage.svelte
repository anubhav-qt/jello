<!-- The mapping page (docs/adr/0047-song-key-mappings.md): press a laptop key,
     then click notes on the drawn piano to add or take them off it, each
     sounding as it is clicked. A song has a main mapping, and a section may
     have its own. Locked keys never move when the agent optimizes. Its Guitar
     side edits what the same keys play on a guitar's track
     (docs/adr/0049-the-keyboard-guitar.md): press a left-hand key, or none for
     the open strings, then move each string's note. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { clickThenLetGo } from "./dom";
  import Keyboard from "./Keyboard.svelte";
  import { noteName, type KeyLayout, type Platform } from "./keyboard";
  import {
    PIANO_HIGHEST,
    PIANO_LOWEST,
    STRUM_SLOWEST_MS,
    STRUM_STEP_MS,
    isBlackNote,
    layoutOfGuitar,
    layoutOfMapping,
    withNoteSwitched,
    withStringMoved,
    type GuitarStrings,
    type MappingSender,
    type MappingView,
  } from "./keys";
  import { colorForNote } from "./note-colors";
  import type { PracticeMark } from "./practice";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    send: MappingSender;
    onClose: () => void;
  }
  let { platform, layout, send, onClose }: Props = $props();

  let view: MappingView | null = $state(null);
  let section: number | null = $state(null);
  // The laptop key being edited, like "KeyF".
  let selected: string | null = $state(null);
  // The piano note sounding while it is pressed.
  let sounding: number | null = $state(null);
  // The last key pressed that can't play notes, until a key that can.
  let refusedKey: string | null = $state(null);
  // Whether the page edits the guitar instead of the piano keys, and the
  // left-hand key it edits, or null for the open strings.
  let guitar = $state(false);
  let shape: string | null = $state(null);

  onMount(() => {
    load(null);
    return () => {
      letGoOfNote();
    };
  });

  async function load(next: number | null) {
    refusedKey = null;
    section = next;
    const answer = await send.look(next);
    if (answer !== null) {
      view = answer;
    }
  }

  function show(answer: MappingView | null) {
    if (answer !== null) {
      view = answer;
    }
  }

  // A laptop key went down: main.ts sends every key here while the page is
  // open, so it edits instead of playing.
  export function keyPressed(code: string) {
    if (view === null) {
      return;
    }
    if (guitar) {
      if (view.guitar_shape_keys.includes(code)) {
        refusedKey = null;
        shape = code;
      } else {
        refusedKey = code;
      }
      return;
    }
    if (!view.mappable.includes(code)) {
      refusedKey = code;
      return;
    }
    refusedKey = null;
    selected = code;
  }

  function clickOnKeyboard(event: MouseEvent) {
    if (!(event.target instanceof Element)) {
      return;
    }
    const key = event.target.closest<HTMLElement>("[data-code]");
    if (key === null) {
      return;
    }
    const code = key.dataset.code;
    if (code === undefined || code === "") {
      return;
    }
    keyPressed(code);
  }

  let drawn = $derived.by(() => {
    if (view === null) {
      return layout;
    }
    if (guitar) {
      return layoutOfGuitar(view, layout, shape);
    }
    return layoutOfMapping(view, layout);
  });

  // The strings being edited, thickest first.
  let shownStrings: GuitarStrings = $derived.by(() => {
    if (view === null) {
      return [];
    }
    if (shape !== null && view.guitar.shapes[shape] !== undefined) {
      return view.guitar.shapes[shape].strings;
    }
    return view.guitar.open;
  });

  function noteWords(note: number | null): string {
    if (note === null) {
      return "silent";
    }
    const parts = noteName(note);
    return `${parts.pitch}${parts.octave}`;
  }

  function soundOf(note: number | null) {
    letGoOfNote();
    if (note === null) {
      return;
    }
    sounding = note;
    send.soundNote(note, true);
  }

  async function moveString(string: number, steps: number) {
    if (view === null) {
      return;
    }
    let from = view.guitar.open[string];
    if (from === null) {
      from = 40;
    }
    const strings = withStringMoved(shownStrings, string, steps, from);
    soundOf(strings[string]);
    show(await send.setGuitar(section, shape, strings));
  }

  // J and K strum; each string waits this long after the one before.
  async function stepStrum(steps: number) {
    if (view === null) {
      return;
    }
    let ms = view.guitar.strum_ms + steps * STRUM_STEP_MS;
    if (ms < 0) {
      ms = 0;
    }
    if (ms > STRUM_SLOWEST_MS) {
      ms = STRUM_SLOWEST_MS;
    }
    if (ms === view.guitar.strum_ms) {
      return;
    }
    show(await send.setStrum(section, ms));
  }

  async function silenceString(string: number) {
    const strings = [...shownStrings];
    strings[string] = null;
    letGoOfNote();
    show(await send.setGuitar(section, shape, strings));
  }

  let selectedNotes = $derived.by(() => {
    if (view === null || selected === null) {
      return [];
    }
    const notes = view.keys[selected];
    if (notes === undefined) {
      return [];
    }
    return notes;
  });

  let selectedLocked = $derived.by(() => {
    if (view === null || selected === null) {
      return false;
    }
    return view.locked.includes(selected);
  });

  // The selected key shows as practice's next key, and locked ones are marked.
  let selectionMarks = $derived.by(() => {
    const marks = new Map<string, PracticeMark>();
    let marked = selected;
    if (guitar) {
      marked = shape;
    }
    if (marked !== null) {
      marks.set(marked, { key: marked, label: "", state: "next" });
    }
    return marks;
  });

  let lockMarks = $derived.by(() => {
    const marks = new SvelteMap<string, string>();
    if (view !== null) {
      for (const key of view.locked) {
        marks.set(key, "L");
      }
    }
    return marks;
  });

  const NO_HELD = new Set<string>();

  // The piano's white keys, each with the black key on its right, if any.
  let whiteNotes = $derived.by(() => {
    const whites: { note: number; black: number | null }[] = [];
    for (let note = PIANO_LOWEST; note <= PIANO_HIGHEST; note += 1) {
      if (isBlackNote(note)) {
        continue;
      }
      let black: number | null = null;
      if (note < PIANO_HIGHEST && isBlackNote(note + 1)) {
        black = note + 1;
      }
      whites.push({ note, black });
    }
    return whites;
  });

  function isOn(note: number): boolean {
    return selectedNotes.includes(note);
  }

  // A piano note pressed: it sounds, and goes on or off the selected key.
  async function pressNote(note: number) {
    letGoOfNote();
    refusedKey = null;
    sounding = note;
    send.soundNote(note, true);
    if (selected === null) {
      return;
    }
    const notes = withNoteSwitched(selectedNotes, note);
    show(await send.setNotes(section, selected, notes));
  }

  function letGoOfNote() {
    if (sounding === null) {
      return;
    }
    send.soundNote(sounding, false);
    sounding = null;
  }

  async function clearKey() {
    refusedKey = null;
    if (selected === null) {
      return;
    }
    show(await send.setNotes(section, selected, []));
  }

  async function switchLock() {
    refusedKey = null;
    if (selected === null) {
      return;
    }
    show(await send.setLocked(section, selected, !selectedLocked));
  }

  async function reset() {
    refusedKey = null;
    show(await send.reset(section));
  }

  let resetWords = $derived.by(() => {
    if (section === null) {
      return "Back to the scale's default";
    }
    return "Back to the main mapping";
  });

  function stringWords(string: number): string {
    if (string === 0) {
      return "Thickest";
    }
    if (string === shownStrings.length - 1) {
      return "Thinnest";
    }
    return `String ${string + 1}`;
  }

  let selectedWords = $derived.by(() => {
    if (guitar && refusedKey !== null) {
      return `${refusedKey} can't hold a shape: only the keys from 1QAZ to 5TGB can`;
    }
    if (guitar) {
      if (shape === null) {
        return "The open strings: press a key from 1QAZ to 5TGB for its shape";
      }
      const shown = shape.replace("Key", "").replace("Digit", "");
      return `${shown} holds ${view?.guitar.shapes[shape]?.name ?? "nothing yet"}`;
    }
    if (refusedKey !== null) {
      return `${refusedKey} does something else in Jello, so it can't play notes`;
    }
    if (selected === null) {
      return "Press a key on your keyboard";
    }
    const name = selected.replace("Key", "").replace("Digit", "");
    if (selectedNotes.length === 0) {
      return `${name} plays nothing yet`;
    }
    const names = selectedNotes.map((note) => {
      const parts = noteName(note);
      return `${parts.pitch}${parts.octave}`;
    });
    return `${name} plays ${names.join(" ")}`;
  });
</script>

<!-- Letting go anywhere ends the note. -->
<svelte:window onpointerup={letGoOfNote} onblur={letGoOfNote} />

<section class="keys-page" style:--whites={whiteNotes.length}>
  <header class="keys-top">
    <div class="tape-field">
      <p class="tape-label">Scale</p>
      <p class="tape-value">{view?.scale ?? "None picked: pick one in Compose"}</p>
    </div>
    <div class="keys-scopes">
      <button
        type="button"
        class="keys-button"
        class:is-picked={section === null}
        onclick={(event) => clickThenLetGo(event, () => load(null))}
      >
        Main
      </button>
      {#if view !== null}
        {#each view.sections as found, number}
          <button
            type="button"
            class="keys-button"
            class:is-picked={section === number}
            title={found.own ? "Has its own mapping" : "Plays the main mapping until you change a key here"}
            onclick={(event) => clickThenLetGo(event, () => load(number))}
          >
            {found.name}{found.own ? " *" : ""}
          </button>
        {/each}
      {/if}
    </div>
    <div class="keys-scopes">
      <button
        type="button"
        class="keys-button"
        class:is-picked={!guitar}
        onclick={(event) => clickThenLetGo(event, () => { guitar = false; refusedKey = null; })}
      >
        Piano
      </button>
      <button
        type="button"
        class="keys-button"
        class:is-picked={guitar}
        onclick={(event) => clickThenLetGo(event, () => { guitar = true; refusedKey = null; })}
      >
        Guitar
      </button>
    </div>
    <button type="button" class="keys-button" onclick={(event) => clickThenLetGo(event, onClose)}>
      Done
    </button>
  </header>

  <div class="keys-editing">
    <p class="keys-selected">{selectedWords}</p>
    {#if guitar}
      <button
        type="button"
        class="keys-button"
        disabled={shape === null}
        onclick={(event) => clickThenLetGo(event, () => { shape = null; refusedKey = null; })}
      >
        Open strings
      </button>
      <span class="tape-label">Strum</span>
      <button
        type="button"
        class="keys-button"
        aria-label="Strum faster"
        disabled={view === null || view.guitar.strum_ms === 0}
        onclick={(event) => clickThenLetGo(event, () => stepStrum(-1))}
      >-</button>
      <span class="keys-string-note">{view?.guitar.strum_ms ?? 0} ms</span>
      <button
        type="button"
        class="keys-button"
        aria-label="Strum slower"
        disabled={view === null || view.guitar.strum_ms === STRUM_SLOWEST_MS}
        onclick={(event) => clickThenLetGo(event, () => stepStrum(1))}
      >+</button>
    {:else}
      <button
        type="button"
        class="keys-button"
        disabled={selected === null || selectedNotes.length === 0}
        onclick={(event) => clickThenLetGo(event, clearKey)}
      >
        Clear key
      </button>
      <button
        type="button"
        class="keys-button"
        class:is-picked={selectedLocked}
        disabled={selected === null}
        onclick={(event) => clickThenLetGo(event, switchLock)}
      >
        {selectedLocked ? "Locked" : "Lock key"}
      </button>
    {/if}
    <button
      type="button"
      class="keys-button"
      disabled={view === null || view.scale === null}
      onclick={(event) => clickThenLetGo(event, reset)}
    >
      {resetWords}
    </button>
  </div>

  {#if guitar}
    <!-- Each string's note sounds as it moves. -->
    <div class="keys-strings">
      {#each shownStrings as note, string}
        <div class="keys-string">
          <span class="tape-label">{stringWords(string)}</span>
          <button type="button" class="keys-button" aria-label={`String ${string + 1} down`} disabled={note === 0} onpointerdown={() => moveString(string, -1)}>-</button>
          <span class="keys-string-note" class:is-silent={note === null}>{noteWords(note)}</span>
          <button type="button" class="keys-button" aria-label={`String ${string + 1} up`} disabled={note === 127} onpointerdown={() => moveString(string, 1)}>+</button>
          <button type="button" class="keys-button" disabled={note === null} onclick={(event) => clickThenLetGo(event, () => silenceString(string))}>Silent</button>
        </div>
      {/each}
    </div>
  {:else}
  <!-- Each note sounds while pressed. -->
  <div class="keys-piano">
    {#each whiteNotes as white (white.note)}
      <div class="keys-white-place">
        <button
          type="button"
          class="keys-white"
          class:is-on={isOn(white.note)}
          aria-label={`${noteName(white.note).pitch}${noteName(white.note).octave}`}
          style:--accent={colorForNote(white.note).accent}
          onpointerdown={() => pressNote(white.note)}
        >
          {#if white.note % 12 === 0}
            <span class="keys-octave">C{noteName(white.note).octave}</span>
          {/if}
        </button>
        {#if white.black !== null}
          {@const black = white.black}
          <button
            type="button"
            class="keys-black"
            class:is-on={isOn(black)}
            aria-label={`${noteName(black).pitch}${noteName(black).octave}`}
            style:--accent={colorForNote(black).accent}
            onpointerdown={() => pressNote(black)}
          ></button>
        {/if}
      </div>
    {/each}
  </div>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="keys-keyboard" onclick={clickOnKeyboard}>
    <Keyboard
      {platform}
      layout={drawn}
      agentHeld={NO_HELD}
      loopPressed={NO_HELD}
      agentMarks={lockMarks}
      practiceMarks={selectionMarks}
      held={NO_HELD}
      sound={null}
      looperBeats={0}
      guide={{}}
      liveGuide={{}}
      compatible={{}}
      melodyNext={{}}
      melody={{}}
      chordNotes={[]}
      song={null}
      actions={{}}
      stepping={false}
    />
  </div>
</section>
