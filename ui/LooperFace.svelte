<!-- Right Shift's face (docs/adr/0024-the-looper-s-look.md): four beat
     lights, what the looper is doing in a few words, and a bar along the
     key's bottom edge that fills as the loop goes round, red while
     recording the first loop or a layer. -->
<script lang="ts">
  import { isRecording, litBeat, looperFill, looperWords, type SoundView } from "./sound";

  interface Props {
    // The wave, the tempo and the looper, or null until Rust has said.
    sound: SoundView | null;
    // How many beats into the count-in, the recording or the repeat playing
    // now the looper is, counted on to this frame.
    beats: number;
  }

  let { sound, beats }: Props = $props();

  // One light for each beat of a bar.
  const BEAT_LIGHTS = [1, 2, 3, 4];

  let state = $derived.by(() => {
    if (sound === null) {
      return "idle";
    }
    return sound.looper.state;
  });

  // Recording the first loop, or a layer over it.
  let recording = $derived.by(() => {
    if (sound === null) {
      return false;
    }
    return isRecording(sound.looper);
  });

  let lit = $derived.by(() => {
    if (sound === null) {
      return 0;
    }
    return litBeat(sound.looper, beats);
  });

  // During the count-in, the beats already counted stay half lit.
  function isCounted(beat: number): boolean {
    if (state !== "counting in") {
      return false;
    }
    return beat < lit;
  }

  let words = $derived.by(() => {
    if (sound === null) {
      return "looper";
    }
    return looperWords(sound, beats);
  });

  let fill = $derived.by(() => {
    if (sound === null) {
      return 0;
    }
    return looperFill(sound.looper, beats);
  });
</script>

<span class="key-note looper-face">
  <span class="looper-beats">
    {#each BEAT_LIGHTS as beat}
      <span class="looper-beat" class:is-on={beat === lit} class:is-counted={isCounted(beat)}></span>
    {/each}
  </span>
  {#if recording}
    <span class="looper-rec"></span>
  {/if}
  <span class="looper-words">{words}</span>
</span>
<span class="looper-bar" class:is-recording={recording} style:width={`${fill * 100}%`}></span>
