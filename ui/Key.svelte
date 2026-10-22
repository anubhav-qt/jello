<!-- One key: what is printed on it, and the note, chord or wave it plays,
     or what it does to the beat. -->
<script lang="ts">
  import type { ChordGuide, Fit, SongKeys } from "./compose";
  import { colorForNote, NOTE_COLORS } from "./note-colors";
  import { kindOf, noteName, notesOf, type KeyLayout, type KeySpec } from "./keyboard";
  import LooperFace from "./LooperFace.svelte";
  import { clearLabel, type SoundView } from "./sound";

  interface Props {
    spec: KeySpec;
    layout: KeyLayout;
    // Whether the key is held down right now.
    held: boolean;
    // The wave, the tempo and the looper, or null until Rust has said.
    sound: SoundView | null;
    // How many beats into what the looper is doing it is now, for right
    // Shift's face.
    looperBeats: number;
    // Compose's guidance for each chord key. Empty outside Compose.
    guide: Record<string, ChordGuide>;
    // Compose's guidance for each melody key. Empty outside Compose.
    melody: Record<string, Fit>;
    // The notes of the chords held now, as pitches from 0 (C) to 11 (B).
    // Empty outside Compose.
    chordNotes: number[];
    // The chord keys Compose's song lights while it plays, or null.
    song: SongKeys | null;
    // What each action key does on the open tab, like "add" for Return.
    actions: Record<string, string>;
    // True while a value is picked in the sound panel, so the arrow keys,
    // which step it, are framed.
    stepping: boolean;
  }

  let { spec, layout, held, sound, looperBeats, guide, melody, chordNotes, song, actions, stepping }: Props = $props();

  const ARROW_KEYS = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"];

  let isStepping = $derived.by(() => {
    if (!stepping) {
      return false;
    }
    return ARROW_KEYS.includes(spec.code);
  });

  let action = $derived(actions[spec.code]);
  let kind = $derived.by(() => {
    if (action !== undefined) {
      return "action";
    }
    return kindOf(spec.code, layout);
  });
  let note = $derived(layout.notes[spec.code]);
  // The notes it plays, lowest first. A chord's lowest note is its root, the
  // note it is named after.
  let notes = $derived(notesOf(spec.code, layout));
  let color = $derived.by(() => {
    if (notes.length > 0) {
      return colorForNote(notes[0]);
    }
    return NOTE_COLORS.C;
  });

  let name = $derived.by(() => {
    if (kind === "note") {
      return noteName(note);
    }
    return { pitch: "", octave: 0 };
  });

  // What Compose says about the chord this key plays, if anything.
  let chordGuide = $derived(guide[spec.code]);

  // A chord key shows its chord's name, the way a chord chart writes it,
  // like "C" or "Cm". Compose spells it for the song's key.
  let chordName = $derived.by(() => {
    if (kind !== "chord") {
      return "";
    }
    if (chordGuide !== undefined) {
      return chordGuide.name;
    }
    return layout.chords[spec.code].name;
  });

  // How well that chord fits the song's key, for Compose's outline.
  let fit = $derived.by(() => {
    if (kind !== "chord") {
      return undefined;
    }
    if (chordGuide === undefined) {
      return undefined;
    }
    return chordGuide.fit;
  });

  // Whether that chord is one of the best to play next, by its rank: 1 is
  // the strongest. Undefined if it isn't one, and while the song plays,
  // since the keys show the song then.
  let bestRank = $derived.by(() => {
    if (kind !== "chord") {
      return undefined;
    }
    if (song !== null) {
      return undefined;
    }
    if (chordGuide === undefined) {
      return undefined;
    }
    if (chordGuide.next === null) {
      return undefined;
    }
    return chordGuide.next.rank;
  });

  // The corner's three squares, lit like signal bars: all three for the
  // strongest, one for the third.
  let bestSquares = $derived.by(() => {
    const squares: boolean[] = [];
    if (bestRank === undefined) {
      return squares;
    }
    const lit = 4 - bestRank;
    for (let square = 1; square <= 3; square++) {
      squares.push(square <= lit);
    }
    return squares;
  });

  // While Compose's song plays, whether this key plays the chord playing...
  let isSongNow = $derived.by(() => {
    if (song === null) {
      return false;
    }
    return song.now.includes(spec.code);
  });

  // ...and how full it is if it plays the chord coming next, like "40%",
  // full as that chord starts. Undefined if it doesn't.
  let songFill = $derived.by(() => {
    if (song === null) {
      return undefined;
    }
    if (!song.next.includes(spec.code)) {
      return undefined;
    }
    return `${song.share * 100}%`;
  });

  // Compose dims a melody key whose note is outside the song's key...
  let isOutside = $derived.by(() => {
    if (kind !== "note") {
      return false;
    }
    return melody[spec.code] === "outside";
  });

  // ...and fills one that plays a note of a chord held now, even outside
  // the key, since that note sounds right over that chord.
  let isChordNote = $derived.by(() => {
    if (kind !== "note") {
      return false;
    }
    return chordNotes.includes(note % 12);
  });

  // The wave keys say "wave" until Rust has said which wave it is.
  let waveLabel = $derived.by(() => {
    if (sound === null) {
      return "wave";
    }
    return sound.wave;
  });

  // Right Shift runs the looper and shows what it is doing.
  let isLooper = $derived.by(() => {
    if (kind !== "beat") {
      return false;
    }
    return layout.beat_keys[spec.code] === "looper";
  });

  // What the other beat keys say: - and = say what they do, and Esc
  // whether there is a loop to clear.
  let beatLabel = $derived.by(() => {
    const beatKey = layout.beat_keys[spec.code];
    if (beatKey === "slower") {
      return "slower";
    }
    if (beatKey === "faster") {
      return "faster";
    }
    if (beatKey === "clear") {
      if (sound === null) {
        return "";
      }
      return clearLabel(sound);
    }
    return "";
  });
</script>

<div
  class="key"
  class:is-half={spec.half === true}
  class:is-held={held}
  class:is-outside={isOutside}
  class:is-chord-note={isChordNote}
  class:is-looper={isLooper}
  class:is-stepping={isStepping}
  class:is-song-now={isSongNow}
  class:is-song-next={songFill !== undefined}
  style:--w={spec.width}
  style:--song-fill={songFill}
  data-code={spec.code}
  data-kind={kind}
  data-note={note}
  data-fit={fit}
  data-best={bestRank}
  style:--accent={color.accent}
  style:--accent-core={color.core}
>
  {#if spec.symbol !== undefined}
    <span class="key-symbol">{spec.symbol}</span>
  {/if}
  <!-- Words like "shift" are printed smaller than single letters. -->
  <span class="key-label" class:is-word={spec.label.length > 1}>{spec.label}</span>

  {#if kind === "note"}
    <!-- The note's name in two parts, so the octave can be drawn smaller.
         They sit on one line, so no space comes between them. -->
    <span class="key-note"><span class="key-pitch">{name.pitch}</span><span class="key-octave">{name.octave}</span></span>
  {/if}
  {#if kind === "chord"}
    <span class="key-note">{chordName}</span>
  {/if}
  {#if bestRank !== undefined}
    <span class="key-best">
      {#each bestSquares as lit}
        <span class="key-best-square" class:is-lit={lit}></span>
      {/each}
    </span>
  {/if}
  {#if kind === "wave"}
    <span class="key-note">{waveLabel}</span>
  {/if}
  {#if kind === "vibrato"}
    <span class="key-note">vibrato</span>
  {/if}
  {#if isLooper}
    <LooperFace {sound} beats={looperBeats} />
  {:else if kind === "beat"}
    <span class="key-note">{beatLabel}</span>
  {/if}
  {#if kind === "action"}
    <span class="key-note">{action}</span>
  {/if}
</div>
