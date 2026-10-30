<!-- Compose's tape (docs/adr/0022-compose-as-a-tape.md and docs/design.md,
     "Compose screen"): the song's key, the song as a row of blocks, each as
     wide as its chord was held, and the chord tried last. Clicking a block
     plays that chord, and the play button plays the whole song. Under the
     progression come the sections, then the chords tried and why the best
     next chord follows. The keys show the rest of the guidance. It sits
     between the tabs and the keyboard, as wide as the keyboard's panel. -->
<script lang="ts">
  import {
    fitWords,
    lengthSince,
    playingShare,
    secondsWords,
    songWords,
    type ChordGuide,
    type ChordInSong,
    type ComposeView,
    type SectionEdit,
    type SectionsView,
  } from "./compose";
  import SectionDetails from "./SectionDetails.svelte";
  import SectionsLane from "./SectionsLane.svelte";
  import { clickThenLetGo } from "./dom";
  import ScalePicker from "./ScalePicker.svelte";
  import { colorForNote } from "./note-colors";

  interface Props {
    // What Compose shows, or null until Rust has said.
    view: ComposeView | null;
    // When Rust answered, by performance.now(). Until the next answer, held
    // chords grow and the playhead moves on by the page's own clock.
    answeredAt: number;
    // What guidance says about the chord tried last, or null.
    tried: ChordGuide | null;
    // The root of the chord tried last, its lowest note, for its color. Null
    // before a chord is tried.
    triedRoot: number | null;
    // How many key widths wide the keyboard is, so the tape is as wide.
    keysAcross: number;
    onScalePick: (tonic: number, kind: string) => void;
    // A block was clicked: `place` is its place in the song, from 0.
    onChordClick: (place: number) => void;
    onPlayClick: () => void;
    onEdit: (action: string, place: number) => void;
    // The sections lane under the progression, and a section's details in
    // the guidance's place while one is picked.
    sections: SectionsView | null;
    // Left Cmd on a Mac, left Alt on Windows.
    modifierName: string;
    onSectionEdit: (edit: SectionEdit) => void;
  }

  let {
    view,
    answeredAt,
    tried,
    triedRoot,
    keysAcross,
    onScalePick,
    onChordClick,
    onPlayClick,
    onEdit,
    sections,
    modifierName,
    onSectionEdit,
  }: Props = $props();

  // The place in the sections' order picked, from 1, or null. Its section's
  // details take the progression's and the tried chords' place.
  let pickedSection = $state<number | null>(null);
  let openSection = $derived.by(() => {
    if (sections === null || pickedSection === null) {
      return null;
    }
    const number = sections.order[pickedSection - 1];
    if (number === undefined) {
      return null;
    }
    return number;
  });

  // Esc closes a section's details. Returns true if one was open.
  export function closeSection(): boolean {
    if (openSection === null) {
      return false;
    }
    pickedSection = null;
    return true;
  }

  // How wide one second of a chord is drawn, in pixels.
  const PIXELS_PER_SECOND = 36;
  let selectedSong = $state<number | null>(null);
  let nextChords = $derived(
    Object.entries(view?.live_guide ?? {})
      .filter(([, guide]) => guide.next !== null)
      .sort((a, b) => a[1].next!.rank - b[1].next!.rank),
  );
  // The best next chord, numbered 1 on the keys: its key's place, like
  // "KeyF", and its guide. Null before a chord is played.
  let bestNext = $derived.by(() => {
    const first = nextChords[0];
    if (first === undefined) {
      return null;
    }
    const [code, guide] = first;
    return { code, guide };
  });
  // The chord picked in the song, or null.
  let selectedChord = $derived.by(() => {
    if (view === null || selectedSong === null) {
      return null;
    }
    const chord = view.progression[selectedSong];
    if (chord === undefined) {
      return null;
    }
    return chord;
  });
  function keyLabel(code: string): string {
    const punctuation: Record<string, string> = {
      Quote: "'",
      BracketLeft: "[",
      BracketRight: "]",
      Backslash: "\\",
      Comma: ",",
      Period: ".",
      Slash: "/",
      Semicolon: ";",
      Backquote: "`",
      ControlLeft: "Ctrl",
      Space: "Space",
    };
    return punctuation[code] ?? code.replace("Key", "").replace("Digit", "");
  }
  function selectSong(place: number) {
    selectedSong = place;
    onChordClick(place);
  }
  function moveSong(direction: "left" | "right") {
    if (selectedSong === null || view === null) {
      return;
    }
    const next = selectedSong + (direction === "left" ? -1 : 1);
    if (next < 0 || next >= view.progression.length) {
      return;
    }
    onEdit(direction, selectedSong);
    selectedSong = next;
  }
  function removeSelected() {
    if (selectedSong === null) {
      return;
    }
    onEdit("remove", selectedSong);
    selectedSong = null;
  }

  // A click on a tried chord makes it what the guidance follows; a click on
  // that chord again goes back to following what is played.
  function pickTried(place: number) {
    if (view !== null && view.selected_history === place) {
      onEdit("follow_live", 0);
    } else {
      onEdit("select_history", place);
    }
  }

  // The page's clock, by performance.now(). It ticks every frame while
  // something on the tape moves.
  let clock = $state(performance.now());

  // How long ago Rust answered, by the page's clock.
  let sinceAnswer = $derived(Math.max(0, clock - answeredAt));

  // Whether something on the tape moves: the playhead while the song plays,
  // and every chord still held, which keeps growing.
  let moving = $derived.by(() => {
    if (view === null) {
      return false;
    }
    if (view.playing !== null) {
      return true;
    }
    if (view.holding !== null) {
      return true;
    }
    for (const chord of view.progression) {
      if (chord.still_held) {
        return true;
      }
    }
    return false;
  });

  $effect(() => {
    if (!moving) {
      return;
    }
    let frame = 0;
    function tick() {
      clock = performance.now();
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
    };
  });

  // How long `chord` has been held, in milliseconds: what Rust said, and
  // while it is still held, the time since then too.
  function lengthNow(chord: ChordInSong): number {
    return lengthSince(chord, sinceAnswer);
  }

  // How wide a chord `ms` milliseconds long is drawn. A block is never
  // narrower than its name and length (stage.css), so a short chord can
  // look a little longer.
  function widthFor(ms: number): string {
    const pixels = (ms / 1000) * PIXELS_PER_SECOND;
    return `${pixels}px`;
  }

  // How far the playhead is through the chord at `place` in the song, from
  // 0 at its start to 1 at its end. Null while that chord isn't playing.
  function playedShare(place: number): number | null {
    if (view === null) {
      return null;
    }
    if (view.playing === null) {
      return null;
    }
    if (view.playing.chord !== place) {
      return null;
    }
    return playingShare(view.playing, view.progression, sinceAnswer);
  }

  // How long the whole song is, in milliseconds.
  let songMs = $derived.by(() => {
    let total = 0;
    if (view !== null) {
      for (const chord of view.progression) {
        total += lengthNow(chord);
      }
    }
    return total;
  });

  // Whether the player opened the key picker again after picking a key.
  let pickerReopened = $state(false);

  // The picker is open until a key is picked, and after that while the
  // player has it open again. It takes the song lane's place.
  let pickerShown = $derived.by(() => {
    if (view === null) {
      return false;
    }
    if (view.key === null) {
      return true;
    }
    return pickerReopened;
  });

  function openOrClosePicker() {
    pickerReopened = !pickerReopened;
  }

  function pick(tonic: number, kind: string) {
    pickerReopened = false;
    onScalePick(tonic, kind);
  }

  // With no chord in the song and none held, the lane says what it is for.
  let songIsEmpty = $derived.by(() => {
    if (view === null) {
      return true;
    }
    if (view.progression.length > 0) {
      return false;
    }
    return view.holding === null;
  });

  // What Trying says about the chord tried last: how it fits the key, and
  // why. A best next chord says why it follows the song well instead.
  let triedWords = $derived.by(() => {
    if (tried === null) {
      return "";
    }
    if (tried.next === null) {
      return `${fitWords(tried.fit)}. ${tried.why}`;
    }
    return `${fitWords(tried.fit)}, best next. ${tried.next.why}`;
  });

  let triedColor = $derived.by(() => {
    if (triedRoot === null) {
      return colorForNote(0).accent;
    }
    return colorForNote(triedRoot).accent;
  });

  // Keeps the end of a row in view: the song's, or the chords tried.
  // Whenever the row gets wider or narrower, as a chord is added, taken off
  // or held, it scrolls to its end. Playing changes no width, so it never
  // scrolls.
  function keepEndInView(track: HTMLElement) {
    const lane = track.parentElement;
    const watcher = new ResizeObserver(() => {
      if (lane !== null) {
        lane.scrollLeft = lane.scrollWidth;
      }
    });
    watcher.observe(track);
    return {
      destroy() {
        watcher.disconnect();
      },
    };
  }
</script>

<section class="tape" style:--keys-across={keysAcross}>
  {#if view !== null}
    <div class="tape-top">
      <div class="tape-field">
        <p class="tape-label">Scale</p>
        {#if view.scale === null}
          <p class="tape-key is-unpicked">Pick a scale</p>
        {:else}
          <button
            type="button"
            class="tape-key"
            style:--accent={colorForNote(view.scale.tonic).accent}
            onclick={(event) => clickThenLetGo(event, openOrClosePicker)}
          >
            {view.scale.name} <span class="tape-key-arrow">v</span>
          </button>
        {/if}
      </div>

      {#if view.key !== null}
        <div class="tape-field">
          <p class="tape-label">Song</p>
          <p class="tape-value">{songWords(view.progression.length, songMs)}</p>
        </div>
        <div class="tape-field is-trying">
          <p class="tape-label">Trying</p>
          {#if tried === null}
            <p class="tape-value is-quiet">Play a chord to see how it fits.</p>
          {:else}
            <p class="tape-value">
              <span class="tape-trying-name" style:--accent={triedColor}
                >{tried.name}</span
              >
              {triedWords}
            </p>
          {/if}
        </div>
      {/if}
    </div>

    {#if openSection === null}
      <div class="tape-row tape-slot">
        {#if pickerShown}
          <ScalePicker
            tonic={view.scale?.tonic ?? null}
            kind={view.scale?.kind ?? null}
            onPick={pick}
          />
        {:else}
          <span class="tape-row-label">Progression</span>
          {#if songIsEmpty}
            <p class="tape-empty">
              Your final progression goes here. Try chords freely, then add
              from the chords tried or press Return.
            </p>
          {:else}
            <div class="tape-lane">
              {#if view.progression.length > 0}
                <button
                  type="button"
                  class="tape-play"
                  aria-label="Play the song"
                  onclick={(event) => clickThenLetGo(event, onPlayClick)}
                >
                  &#9654;
                </button>
              {/if}
              <div class="tape-scroller">
                <div class="tape-track" use:keepEndInView>
                  {#each view.progression as chord, place}
                    {@const played = playedShare(place)}
                    <button
                      type="button"
                      class="tape-block"
                      class:is-playing={played !== null}
                      class:is-selected={selectedSong === place}
                      style:--accent={colorForNote(chord.root).accent}
                      style:width={widthFor(lengthNow(chord))}
                      onclick={(event) =>
                        clickThenLetGo(event, () => selectSong(place))}
                    >
                      <span class="tape-block-name">{chord.name}</span>
                      <span class="tape-block-length"
                        >{secondsWords(lengthNow(chord))}</span
                      >
                      {#if played !== null}
                        <span class="tape-playhead" style:--played={played}
                        ></span>
                      {/if}
                    </button>
                  {/each}
                  {#if view.holding !== null}
                    <div
                      class="tape-block is-holding"
                      style:--accent={colorForNote(view.holding.root).accent}
                      style:width={widthFor(lengthNow(view.holding))}
                    >
                      <span class="tape-block-name">{view.holding.name}</span>
                      <span class="tape-block-length"
                        >{secondsWords(lengthNow(view.holding))}</span
                      >
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          {/if}
        {/if}
      </div>
    {/if}

    <SectionsLane
      {sections}
      {modifierName}
      picked={pickedSection}
      onPick={(place) => (pickedSection = place)}
      onEdit={onSectionEdit}
    />

    {#if sections !== null && openSection !== null}
      <SectionDetails
        {sections}
        number={openSection}
        onEdit={onSectionEdit}
        onClose={() => (pickedSection = null)}
      />
    {:else if selectedChord !== null && selectedSong !== null}
      <!-- A chord picked in the song can be moved or taken off. -->
      <div class="tape-row tape-bottom">
        <span class="tape-row-label">Selected</span>
        <span
          class="tape-chosen"
          style:--accent={colorForNote(selectedChord.root).accent}
          >{selectedChord.name}</span
        >
        <div class="tape-tools">
          <button
            type="button"
            disabled={selectedSong === 0}
            onclick={(event) => clickThenLetGo(event, () => moveSong("left"))}
            >← Move</button
          >
          <button
            type="button"
            disabled={selectedSong === view.progression.length - 1}
            onclick={(event) => clickThenLetGo(event, () => moveSong("right"))}
            >Move →</button
          >
          <button
            type="button"
            onclick={(event) => clickThenLetGo(event, removeSelected)}
            >Remove</button
          >
          <button
            type="button"
            onclick={(event) =>
              clickThenLetGo(event, () => (selectedSong = null))}>Done</button
          >
        </div>
      </div>
    {:else}
      <div class="tape-row tape-bottom">
        <span class="tape-row-label">Tried</span>
        {#if view.history.length === 0}
          <p class="tape-quiet">Each chord you play stays here.</p>
        {:else}
          <div class="tape-tried">
            <div class="tape-tried-track" use:keepEndInView>
              {#each view.history as chord, place}
                <div
                  class="tried-chord"
                  class:is-selected={view.selected_history === place}
                  style:--accent={colorForNote(chord.root).accent}
                >
                  <button
                    type="button"
                    class="tried-pick"
                    title="Guide from this chord. Click again to follow what you play."
                    onclick={(event) =>
                      clickThenLetGo(event, () => pickTried(place))}
                  >
                    <strong>{chord.name}</strong>
                    <span>{secondsWords(lengthNow(chord))}</span>
                  </button>
                  <button
                    type="button"
                    class="tried-add"
                    aria-label={`Add ${chord.name} to the progression`}
                    title={`Add ${chord.name} to the progression`}
                    onclick={(event) =>
                      clickThenLetGo(event, () => onEdit("add_history", place))}
                    >+</button
                  >
                </div>
              {/each}
            </div>
          </div>
        {/if}
        <div class="tape-next">
          <span class="tape-label">Next</span>
          {#if bestNext === null}
            <span class="tape-next-why">Play a chord to see what follows it.</span>
          {:else}
            {#if view.reference !== null}
              <span class="tape-next-why">after {view.reference.name}</span>
            {/if}
            <span class="tape-next-rank">{bestNext.guide.next!.rank}</span>
            <span class="tape-next-name">{bestNext.guide.name}</span>
            <kbd>{keyLabel(bestNext.code)}</kbd>
            <span class="tape-next-why">{bestNext.guide.next!.why}</span>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</section>
