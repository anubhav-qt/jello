<script lang="ts">
  // The song's tracks, one tile each, over the looper's transport
  // (docs/design.md#tracks-and-sections). The picked track is lit in
  // lavender: the keys play it and the sound panel shows it. A click picks a
  // track, a double click renames it, its x removes it, and "+" makes the
  // next one. The picked track's volume sits at the row's right end.
  import { clickThenLetGo } from "./dom";
  import type { SoundView, TrackView } from "./sound";
  import TrackVolume from "./TrackVolume.svelte";

  interface Props {
    sound: SoundView;
    // Left Cmd on a Mac, left Alt on Windows: with a number it picks a track.
    pickModifier: string;
    // Each track counts from 1.
    onPick: (track: number) => void;
    onRename: (track: number, name: string) => void;
    onVolume: (track: number, volume: number) => void;
    onRemove: (track: number) => void;
  }
  let { sound, pickModifier, onPick, onRename, onVolume, onRemove }: Props =
    $props();

  // The most tracks a song has, one per number key.
  const MOST_TRACKS = 9;

  // The track being renamed, counted from 1, or null.
  let renaming: number | null = $state(null);
  let newName = $state("");

  let picked = $derived(sound.picked_track);
  let pickedTrack = $derived(sound.tracks[picked - 1]);
  // Removing is refused while the loop records, so the x says so.
  let recording = $derived(
    sound.looper.state === "recording" ||
      sound.looper.state === "layering" ||
      sound.looper.state === "counting in",
  );

  function startRenaming(track: number, view: TrackView) {
    renaming = track;
    newName = view.name;
  }

  function finishRenaming() {
    if (renaming === null) {
      return;
    }
    const track = renaming;
    renaming = null;
    onRename(track, newName);
  }

  function onRenameKey(event: KeyboardEvent) {
    if (event.key === "Enter") {
      finishRenaming();
    } else if (event.key === "Escape") {
      renaming = null;
    }
  }

  // What a tile shows after its name: the drum kit for a drums track.
  function kindOf(view: TrackView): string {
    if (!view.drums) {
      return "";
    }
    if (view.kit === null) {
      return "Drums, no kit";
    }
    return `Drums: ${view.kit}`;
  }
</script>

<div class="track-row">
  {#each sound.tracks as view, place (place)}
    {@const track = place + 1}
    <div
      class="track"
      class:is-picked={track === picked}
      title={`${view.name} (${pickModifier} + ${track})`}
    >
      {#if renaming === track}
        <span class="track-number">{track}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="track-name-input"
          aria-label={`Track ${track}'s name`}
          bind:value={newName}
          autofocus
          onkeydown={onRenameKey}
          onblur={finishRenaming}
        />
      {:else}
        <button
          type="button"
          class="track-pick"
          onclick={(event) => clickThenLetGo(event, () => onPick(track))}
          ondblclick={() => startRenaming(track, view)}
        >
          <span class="track-number">{track}</span>
          <span class="track-name">{view.name}</span>
          {#if view.drums}
            <span class="track-kind">{kindOf(view)}</span>
          {/if}
        </button>
        {#if sound.tracks.length > 1}
          <button
            type="button"
            class="track-remove"
            aria-label={`Remove track ${track}`}
            title={`Remove track ${track} with its layers`}
            disabled={recording}
            onclick={(event) => clickThenLetGo(event, () => onRemove(track))}
          >
            ×
          </button>
        {/if}
      {/if}
    </div>
  {/each}
  {#if sound.tracks.length < MOST_TRACKS}
    <button
      type="button"
      class="track-add"
      title={`Make track ${sound.tracks.length + 1} in the starting sound`}
      aria-label={`Make track ${sound.tracks.length + 1}`}
      onclick={(event) =>
        clickThenLetGo(event, () => onPick(sound.tracks.length + 1))}
    >
      +
    </button>
  {/if}
  <span class="track-row-gap"></span>
  {#if pickedTrack !== undefined}
    <TrackVolume
      track={picked}
      volume={pickedTrack.volume}
      onVolume={(volume) => onVolume(picked, volume)}
    />
  {/if}
</div>

<style>
  /* Sizes are in em, so the row scales with the keys (.stage-controls). */
  .track-row {
    display: flex;
    align-items: center;
    gap: 0.46em;
  }
  /* Tiles look like the keys under them. With many tracks they shrink and
     long names are cut off, so the row stays one line. */
  .track {
    display: flex;
    align-items: center;
    flex: 0 1 auto;
    min-width: 3.6em;
    max-width: 15.4em;
    height: 2.92em;
    background: var(--key-bg);
    border: 1px solid rgba(var(--ink), 0.09);
    border-radius: 5px;
  }
  .track.is-picked {
    border-color: rgb(var(--accent));
    box-shadow: 0 0 14px -4px rgba(var(--accent), 0.5);
  }
  .track-pick {
    display: flex;
    align-items: center;
    gap: 0.62em;
    min-width: 0;
    height: 100%;
    padding: 0 0.92em;
    color: var(--text);
    background: none;
    border: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .track-number {
    flex: none;
    font-family: "Doto", monospace;
    font-weight: 800;
    font-size: 1.08em;
    color: var(--text-3);
  }
  .track-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track-kind {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85em;
    color: var(--text-2);
  }
  .track.is-picked .track-name,
  .track.is-picked .track-number {
    color: rgb(var(--accent));
  }
  /* While renaming, the number stays and the name becomes a field. */
  .track:has(.track-name-input) {
    gap: 0.62em;
    padding-left: 0.92em;
  }
  .track-name-input {
    width: 10em;
    height: 100%;
    padding: 0 0.92em 0 0;
    color: rgb(var(--accent));
    background: none;
    border: 0;
    outline: none;
    font: inherit;
  }
  /* The x sits at the tile's right end, faint until the pointer is on it. */
  .track-remove {
    flex: none;
    height: 100%;
    padding: 0 0.7em 0 0.1em;
    color: var(--text-3);
    background: none;
    border: 0;
    font: inherit;
    font-size: 1.15em;
    cursor: pointer;
  }
  .track-remove:hover:not(:disabled) {
    color: rgb(var(--rec));
  }
  .track-remove:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .track:has(.track-remove) .track-pick {
    padding-right: 0.4em;
  }
  .track-add {
    flex: none;
    height: 2.92em;
    color: var(--text-2);
    background: var(--key-bg);
    border: 1px solid rgba(var(--ink), 0.09);
    border-radius: 5px;
    font: inherit;
    cursor: pointer;
    width: 2.92em;
  }
  .track-add:hover,
  .track:not(.is-picked):hover {
    border-color: rgba(var(--accent), 0.45);
  }
  .track-row-gap {
    flex: 1 0 0.92em;
  }
</style>
