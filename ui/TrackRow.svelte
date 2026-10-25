<script lang="ts">
  // The song's tracks, one tile each, over the looper's transport
  // (docs/design.md#tracks-and-sections). The picked track is lit in
  // lavender: the keys play it and the sound panel shows it. A click picks a
  // track, a double click renames it, and "+" makes the next one.
  import { clickThenLetGo } from "./dom";
  import type { SoundView, TrackView } from "./sound";

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
  // Removing is refused while the loop records, so the button says so.
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

  // What a tile shows under its name: the drum kit for a drums track.
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
      title={`Track ${track} (${pickModifier} + ${track})`}
    >
      {#if renaming === track}
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
      {/if}
    </div>
  {/each}
  {#if sound.tracks.length < MOST_TRACKS}
    <button
      type="button"
      class="track-add"
      title={`Make track ${sound.tracks.length + 1} in the starting sound`}
      onclick={(event) =>
        clickThenLetGo(event, () => onPick(sound.tracks.length + 1))}
    >
      +
    </button>
  {/if}
  {#if pickedTrack !== undefined}
    <label class="track-volume">
      Track {picked} volume
      <input
        aria-label={`Track ${picked}'s volume in percent`}
        type="number"
        min="0"
        max="100"
        step="1"
        value={pickedTrack.volume}
        onchange={(event) => onVolume(picked, event.currentTarget.valueAsNumber)}
      />
      %
    </label>
    <button
      type="button"
      class="track-remove"
      disabled={sound.tracks.length < 2 || recording}
      onclick={(event) => clickThenLetGo(event, () => onRemove(picked))}
    >
      Remove track {picked}
    </button>
  {/if}
</div>

<style>
  .track-row {
    display: flex;
    justify-content: center;
    align-items: stretch;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 6px;
    color: var(--text-2);
    font-size: 12px;
  }
  .track {
    display: flex;
  }
  .track-pick,
  .track-add,
  .track-remove,
  input {
    padding: 5px 9px;
    color: var(--text);
    background: var(--panel-bg);
    border: 1px solid rgba(var(--accent), 0.2);
    border-radius: 3px;
    font: inherit;
  }
  /* Number, name and kit on one line, so the row stays one line tall. */
  .track-pick {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 96px;
    max-width: 220px;
    text-align: left;
    cursor: pointer;
  }
  .track-number {
    font-family: "Doto", monospace;
    color: var(--text-2);
  }
  .track-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track-kind {
    color: var(--text-2);
    font-size: 11px;
    white-space: nowrap;
  }
  .track.is-picked .track-pick {
    color: rgb(var(--accent));
    border-color: rgb(var(--accent));
  }
  .track.is-picked .track-number {
    color: rgb(var(--accent));
  }
  .track-name-input {
    width: 140px;
  }
  .track-add {
    min-width: 36px;
    color: rgb(var(--accent));
    cursor: pointer;
  }
  .track-volume {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .track-volume input {
    width: 55px;
  }
  .track-remove {
    color: var(--text-2);
  }
  :disabled {
    opacity: 0.5;
  }
</style>
