<!-- One key: what is printed on it, and the note or wave it plays. -->
<script lang="ts">
  import { kindOf, noteName, pitchHue, type KeyLayout, type KeySpec } from "./keyboard";

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

  // Only keys that play a note get a color and a note name.
  let hue = $derived.by(() => {
    if (kind === "note") {
      return pitchHue(note);
    }
    return undefined;
  });
  let name = $derived.by(() => {
    if (kind === "note") {
      return noteName(note);
    }
    return { pitch: "", octave: 0 };
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
  {#if kind === "wave"}
    <span class="key-note">{waveLabel}</span>
  {/if}
</div>
