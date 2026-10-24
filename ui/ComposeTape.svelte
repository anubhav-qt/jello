<!-- Compose's tape (docs/adr/0022-compose-as-a-tape.md and docs/design.md,
     "Compose screen"): the song's key, the song as a row of blocks, each as
     wide as its chord was held, and the chord tried last. Clicking a block
     plays that chord, and the play button plays the whole song. It sits
     between the tabs and the keyboard, as wide as the keyboard's panel. -->
<script lang="ts">
  import {
    fitWords,
    lengthSince,
    pickerName,
    playingShare,
    secondsWords,
    songWords,
    type ChordGuide,
    type ChordInSong,
    type ComposeView,
    type KeyChoice,
  } from "./compose";
  import { noteName, type KeyLayout } from "./keyboard";
  import { clickThenLetGo } from "./dom";
  import { colorForNote } from "./note-colors";

  interface Props {
    // What Compose shows, or null until Rust has said.
    view: ComposeView | null;
    layout: KeyLayout;
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
    onKeyPick: (tonic: number, mode: "major" | "minor") => void;
    // A block was clicked: `place` is its place in the song, from 0.
    onChordClick: (place: number) => void;
    onPlayClick: () => void;
    onEdit: (action: string, place: number) => void;
  }

  let {
    view,
    layout,
    answeredAt,
    tried,
    triedRoot,
    keysAcross,
    onKeyPick,
    onChordClick,
    onPlayClick,
    onEdit,
  }: Props = $props();

  // How wide one second of a chord is drawn, in pixels.
  const PIXELS_PER_SECOND = 36;
  let selectedSong = $state<number | null>(null);
  let nextChords = $derived(
    Object.entries(view?.live_guide ?? {})
      .filter(([, guide]) => guide.next !== null)
      .sort((a, b) => a[1].next!.rank - b[1].next!.rank),
  );
  let companions = $derived(
    Object.entries(view?.compatible ?? {}).sort((a, b) =>
      (view?.live_guide[a[0]].name ?? "").localeCompare(
        view?.live_guide[b[0]].name ?? "",
      ),
    ),
  );
  let nextNotes = $derived(
    Object.entries(view?.melody_next ?? {}).sort(
      (a, b) => a[1].rank - b[1].rank,
    ),
  );
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
  function melodyName(code: string): string {
    const name = noteName(layout.notes[code]);
    return `${name.pitch}${name.octave}`;
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

  // The keys to pick from, the major ones above the minor ones.
  let majorKeys = $derived(keysIn("major"));
  let minorKeys = $derived(keysIn("minor"));

  function keysIn(mode: "major" | "minor"): KeyChoice[] {
    const keys: KeyChoice[] = [];
    if (view !== null) {
      for (const choice of view.keys) {
        if (choice.mode === mode) {
          keys.push(choice);
        }
      }
    }
    return keys;
  }

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

  function pick(choice: KeyChoice) {
    pickerReopened = false;
    onKeyPick(choice.tonic, choice.mode);
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

  // What the hint says the chord keys show: while the song plays, its
  // chords; otherwise how each one fits.
  let keysWords = $derived.by(() => {
    const fits =
      "Outlines show key fit. Corner squares follow the saved song. Numbers follow the live reference. Dots mark chords that work together.";
    if (view === null) {
      return fits;
    }
    if (view.playing === null) {
      return fits;
    }
    return "Keys: glowing plays now, filling plays next.";
  });

  let triedColor = $derived.by(() => {
    if (triedRoot === null) {
      return colorForNote(0).accent;
    }
    return colorForNote(triedRoot).accent;
  });

  // Keeps the end of the song in view. Whenever the row of blocks gets
  // wider or narrower, as a chord is added, taken off or held, the lane
  // scrolls to its end. Playing changes no width, so it never scrolls.
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
        <p class="tape-label">Key</p>
        {#if view.key === null}
          <p class="tape-key is-unpicked">Pick a key</p>
        {:else}
          <button
            type="button"
            class="tape-key"
            style:--accent={colorForNote(view.key.tonic).accent}
            onclick={(event) => clickThenLetGo(event, openOrClosePicker)}
          >
            {view.key.name} <span class="tape-key-arrow">v</span>
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

    <div class="tape-slot">
      {#if pickerShown}
        <div class="tape-picker">
          <span class="tape-label">Major</span>
          {#each majorKeys as choice}
            <button
              type="button"
              class="tape-cell"
              style:--accent={colorForNote(choice.tonic).accent}
              onclick={(event) => clickThenLetGo(event, () => pick(choice))}
            >
              {pickerName(choice)}
            </button>
          {/each}
          <span class="tape-label">Minor</span>
          {#each minorKeys as choice}
            <button
              type="button"
              class="tape-cell"
              style:--accent={colorForNote(choice.tonic).accent}
              onclick={(event) => clickThenLetGo(event, () => pick(choice))}
            >
              {pickerName(choice)}
            </button>
          {/each}
        </div>
      {:else if songIsEmpty}
        <p class="tape-empty">
          Your final progression goes here. Try chords freely, then add from
          your history or press Return.
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
                    <span class="tape-playhead" style:--played={played}></span>
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
    </div>

    {#if selectedSong !== null && selectedSong < view.progression.length}
      <div class="progression-tools">
        <span>Selected: {view.progression[selectedSong].name}</span>
        <button disabled={selectedSong === 0} onclick={() => moveSong("left")}
          >← Move</button
        >
        <button
          disabled={selectedSong === view.progression.length - 1}
          onclick={() => moveSong("right")}>Move →</button
        >
        <button
          onclick={() => {
            onEdit("remove", selectedSong!);
            selectedSong = null;
          }}>Remove</button
        >
      </div>
    {/if}

    <div class="exploration">
      <section class="exploration-history">
        <div class="exploration-heading">
          <h2>Tried chords</h2>
          <span>Explore first. Add your choices below.</span>
        </div>
        <div class="history-list">
          {#each view.history as chord, place}
            <div
              class="history-chord"
              class:is-selected={view.selected_history === place}
              style:--accent={colorForNote(chord.root).accent}
            >
              <button
                class="history-select"
                onclick={() => onEdit("select_history", place)}
                title="Use this chord as the guidance reference"
              >
                <strong>{chord.name}</strong><span
                  >{secondsWords(lengthNow(chord))}</span
                >
              </button>
              <button
                class="history-add"
                onclick={() => onEdit("add_history", place)}
                aria-label={`Add ${chord.name} to progression`}>+ Add</button
              >
            </div>
          {:else}
            <p class="exploration-empty">
              Play chords on the left. Each try stays here for this session.
            </p>
          {/each}
        </div>
      </section>
      <section class="exploration-guide">
        <div class="exploration-heading">
          <h2>After {view.reference?.name ?? "your next chord"}</h2>
          <button
            class:is-active={view.selected_history === null}
            onclick={() => onEdit("follow_live", 0)}>Follow playing</button
          >
        </div>
        <div class="guidance-columns">
          <div>
            <h3>Next chords <span>numbered keys</span></h3>
            {#each nextChords as [code, guide]}
              <p>
                <strong>{guide.next!.rank}. {guide.name}</strong><kbd
                  >{keyLabel(code)}</kbd
                ><span>{guide.next!.why}</span>
              </p>
            {:else}<p>Play a chord to get suggestions.</p>{/each}
          </div>
          <div>
            <h3>Work together <span>dotted keys</span></h3>
            <div class="companion-list">
              {#each companions as [code, why]}
                <span title={why}
                  >{view.live_guide[code].name}<kbd>{keyLabel(code)}</kbd></span
                >
              {:else}<p>Chords sharing notes appear here.</p>{/each}
            </div>
          </div>
          <div>
            <h3>Next melody <span>numbered notes</span></h3>
            {#each nextNotes as [code, next]}
              <p>
                <strong>{next.rank}. {melodyName(code)}</strong><kbd
                  >{keyLabel(code)}</kbd
                ><span>{next.why}</span>
              </p>
            {:else}<p>
                Play a melody note on the right. Suggestions update after every
                note.
              </p>{/each}
          </div>
        </div>
      </section>
    </div>

    <p class="tape-hint" class:is-hidden={pickerShown}>
      Return adds the chord you're holding or just let go of. Delete takes the
      last one off. {keysWords}
    </p>
  {/if}
</section>
