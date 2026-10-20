<!-- One key: what is printed on it, and the note, chord or wave it plays. -->
<script lang="ts">
  import { kindOf, noteName, notesOf, pitchHue, type KeyLayout, type KeySpec } from "./keyboard";

  interface Props {
    spec: KeySpec;
    layout: KeyLayout;
    // Whether the key is held down right now.
    held: boolean;
    // The wave new notes play, like "saw", or "" until Rust has said.
    wave: string;
  }

  let { spec, layout, held, wave }: Props = $props();

  let kind = $derived(kindOf(spec.code, layout));
  let note = $derived(layout.notes[spec.code]);
  // The notes it plays, lowest first. A chord's lowest note is its root, the
  // note it is named after.
  let notes = $derived(notesOf(spec.code, layout));

  // Only keys that play a note or chord get a color: a chord gets its root's.
  let hue = $derived.by(() => {
    if (notes.length > 0) {
      return pitchHue(notes[0]);
    }
    return undefined;
  });
  let name = $derived.by(() => {
    if (kind === "note") {
      return noteName(note);
    }
    return { pitch: "", octave: 0 };
  });

  // A chord key shows the chord's name, the way a chord chart writes it.
  // Every chord is major for now, and a major chord is written as just its
  // root, like "C".
  let chordName = $derived.by(() => {
    if (kind === "chord") {
      return noteName(notes[0]).pitch;
    }
    return "";
  });

  // The wave keys say "wave" until Rust has said which wave it is.
  let waveLabel = $derived.by(() => {
    if (wave === "") {
      return "wave";
    }
    return wave;
  });
</script>

<div
  class="key"
  class:is-half={spec.half === true}
  class:is-held={held}
  style:--w={spec.width}
  data-code={spec.code}
  data-kind={kind}
  data-note={note}
  style:--pitch-hue={hue}
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
  {#if kind === "wave"}
    <span class="key-note">{waveLabel}</span>
  {/if}
</div>
