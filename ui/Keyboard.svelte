<!-- The laptop keyboard, row by row, with each key's note from Rust. -->
<script lang="ts">
  import type { ChordGuide, Fit, SongKeys } from "./compose";
  import Key from "./Key.svelte";
  import { keyRows, type KeyLayout, type Platform } from "./keyboard";
  import type { SoundView } from "./sound";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    // The keys held down right now, by place, like "KeyQ".
    held: Set<string>;
    // The sound, the tempo and the looper, or null until Rust has said.
    sound: SoundView | null;
    // How many beats into what the looper is doing it is now.
    looperBeats: number;
    // Compose's guidance for each chord key. Empty outside Compose.
    guide: Record<string, ChordGuide>;
    // Compose's guidance for each melody key. Empty outside Compose.
    melody: Record<string, Fit>;
    // The notes of the chords held now, as pitches from 0 (C) to 11 (B),
    // for Compose to fill on the melody keys. Empty outside Compose.
    chordNotes: number[];
    // The chord keys Compose's song lights while it plays, or null.
    song: SongKeys | null;
    // What each action key does on the open tab, like "add" for Return.
    actions: Record<string, string>;
    // True while a value is picked in the sound panel: the arrow keys step
    // it, so they are framed.
    stepping: boolean;
  }

  let { platform, layout, held, sound, looperBeats, guide, melody, chordNotes, song, actions, stepping }: Props = $props();

  let rows = $derived(keyRows(platform));
</script>

<div class="keyboard" data-platform={platform}>
  {#each rows as row}
    <div class="keyboard-row" class:is-short={row.short === true}>
      {#each row.keys as spec}
        {#if spec.gap === true}
          <!-- Not a key, just room between keys. -->
          <div class="key-gap" style:--w={spec.width}></div>
        {:else if spec.below === undefined}
          <Key
            {spec}
            {layout}
            {sound}
            {looperBeats}
            {guide}
            {melody}
            {chordNotes}
            {song}
            {actions}
            {stepping}
            held={held.has(spec.code)}
          />
        {:else}
          <!-- A key with another stacked under it shares its spot in the row. -->
          <div class="key-stack" style:--w={spec.width}>
            <Key
              {spec}
              {layout}
              {sound}
              {looperBeats}
              {guide}
              {melody}
              {chordNotes}
              {song}
              {actions}
              {stepping}
              held={held.has(spec.code)}
            />
            <Key
              spec={spec.below}
              {layout}
              {sound}
              {looperBeats}
              {guide}
              {melody}
              {chordNotes}
              {song}
              {actions}
              {stepping}
              held={held.has(spec.below.code)}
            />
          </div>
        {/if}
      {/each}
    </div>
  {/each}
</div>
