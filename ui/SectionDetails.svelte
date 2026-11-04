<!-- A section's details, in a rail on the left of Compose while its block
     is picked on the tape (docs/design.md#tracks-and-sections): its name,
     bars, tempo and key shift, each track's layers on or off, and the
     values it moves. Every change is the section's, so every place in the
     order that uses it changes too. Practice on a track plays the section
     as a practice piece for that track. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    barsWords,
    type SectionEdit,
    type SectionsView,
  } from "./compose";
  import { clickThenLetGo } from "./dom";

  interface Props {
    sections: SectionsView;
    // The section shown, from 1.
    number: number;
    onEdit: (edit: SectionEdit) => void;
    onClose: () => void;
  }
  let { sections, number, onEdit, onClose }: Props = $props();

  let section = $derived(sections.sections[number - 1]);

  // Practicing a track of this section (src-tauri/src/practice_song.rs),
  // and why it couldn't start, if it couldn't.
  let practiceProblem = $state("");
  let practicing = false;

  function playsHere(track: number): boolean {
    for (const [onTrack] of section.layers) {
      if (onTrack === track) {
        return true;
      }
    }
    return false;
  }

  async function practice(track: number) {
    if (practicing) {
      return;
    }
    practicing = true;
    practiceProblem = "";
    try {
      await invoke("practice_section", { section: number - 1, track: track - 1 });
    } catch (error) {
      practiceProblem = String(error);
    }
    practicing = false;
  }

  // The move being added: its track, from 1, or 0 for the tempo.
  let newTrack = $state(1);
  let newValue = $state("filter cutoff_hz");
  let newFrom = $state(200);
  let newTo = $state(8000);

  function isOn(track: number, layer: number): boolean {
    for (const [onTrack, onLayer] of section.layers) {
      if (onTrack === track && onLayer === layer) {
        return true;
      }
    }
    return false;
  }

  function rename(event: Event) {
    const field = event.currentTarget as HTMLInputElement;
    onEdit({ action: "rename", section: number, name: field.value });
  }

  function onNameKey(event: KeyboardEvent) {
    if (event.key === "Enter") {
      rename(event);
    }
  }

  function wholeNumber(event: Event): number {
    const field = event.currentTarget as HTMLInputElement;
    return Math.round(field.valueAsNumber);
  }

  function switchLayer(track: number, layer: number) {
    const on = !isOn(track, layer);
    onEdit({ action: "layer", section: number, track, layer, on });
  }

  function trackOf(choice: number): number | null {
    if (choice === 0) {
      return null;
    }
    return choice;
  }

  function addMove() {
    onEdit({
      action: "automate",
      section: number,
      track: trackOf(newTrack),
      value: newValue,
      from: newFrom,
      to: newTo,
    });
  }

  function whoseValue(track: number | null): string {
    if (track === null) {
      return "Song";
    }
    return `Track ${track}`;
  }

  // Where the section plays, with the key it is shifted to.
  function keyWords(): string {
    if (section.key === null) {
      return "";
    }
    return `plays in ${section.key}`;
  }
</script>

<section class="section-details">
  <div class="details-heading">
    <input
      class="section-name"
      aria-label="The section's name"
      value={section.name}
      onchange={rename}
      onkeydown={onNameKey}
    />
    <div class="section-actions">
      <button
        title="The section's layers become the ones the looper plays now"
        onclick={(event) =>
          clickThenLetGo(event, () =>
            onEdit({ action: "save_again", section: number }),
          )}>Save playing layers</button
      >
      <button onclick={(event) => clickThenLetGo(event, onClose)}
        >Close</button
      >
    </div>
  </div>

  <div class="section-columns">
    <div class="section-column">
      <h3>Length and key</h3>
      <label>
        Bars
        <input
          type="number"
          min="1"
          max="64"
          step="1"
          value={section.bars}
          onchange={(event) =>
            onEdit({ action: "bars", section: number, bars: wholeNumber(event) })}
        />
        <span>{barsWords(section.bars)}</span>
      </label>
      <label>
        Tempo
        <input
          type="number"
          min="40"
          max="240"
          step="1"
          value={section.tempo}
          onchange={(event) =>
            onEdit({ action: "tempo", section: number, tempo: wholeNumber(event) })}
        />
        <span>bpm</span>
      </label>
      <label>
        Key shift
        <input
          type="number"
          min="-12"
          max="12"
          step="1"
          value={section.key_shift}
          onchange={(event) =>
            onEdit({
              action: "key_shift",
              section: number,
              key_shift: wholeNumber(event),
            })}
        />
        <span>half steps, {keyWords()}</span>
      </label>
    </div>

    <div class="section-column">
      <h3>Layers</h3>
      {#each sections.tracks as track, index}
        {@const trackNumber = index + 1}
        <div class="section-track">
          <span class="section-track-name">{trackNumber}. {track.name}</span>
          {#each { length: track.layers } as _, layerIndex}
            {@const layer = layerIndex + 1}
            <button
              class="section-layer"
              class:is-on={isOn(trackNumber, layer)}
              aria-pressed={isOn(trackNumber, layer)}
              onclick={(event) =>
                clickThenLetGo(event, () => switchLayer(trackNumber, layer))}
              >{layer}</button
            >
          {:else}
            <span class="section-quiet">No layers</span>
          {/each}
          {#if playsHere(trackNumber)}
            <button
              class="section-practice"
              title="Practice this track's part of the section, scored per hand"
              onclick={(event) => clickThenLetGo(event, () => practice(trackNumber))}>Practice</button
            >
          {/if}
        </div>
      {/each}
      {#if practiceProblem !== ""}
        <p class="section-problem">{practiceProblem}</p>
      {/if}
    </div>

    <div class="section-column">
      <h3>Moves <span>from the start to the end</span></h3>
      {#each section.automation as moved}
        <p class="section-move">
          <span>{whoseValue(moved.track)}: {moved.value}</span>
          <span>{moved.from} to {moved.to}</span>
          <button
            onclick={(event) =>
              clickThenLetGo(event, () =>
                onEdit({
                  action: "stop_automating",
                  section: number,
                  track: moved.track,
                  value: moved.value,
                }),
              )}>Remove</button
          >
        </p>
      {:else}
        <p class="section-quiet">Nothing moves yet.</p>
      {/each}
      <div class="section-new-move">
        <select aria-label="Whose value moves" bind:value={newTrack}>
          <option value={0}>Song tempo</option>
          {#each sections.tracks as track, index}
            <option value={index + 1}>Track {index + 1}: {track.name}</option>
          {/each}
        </select>
        {#if newTrack !== 0}
          <input
            class="section-value"
            aria-label="The value that moves, like filter cutoff_hz"
            bind:value={newValue}
          />
        {/if}
        <input
          type="number"
          aria-label="Where it starts"
          bind:value={newFrom}
        />
        <input type="number" aria-label="Where it ends" bind:value={newTo} />
        <button onclick={(event) => clickThenLetGo(event, addMove)}>Add</button>
      </div>
    </div>
  </div>
</section>

<style>
  .section-details {
    flex: none;
    width: var(--section-width);
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid rgba(var(--accent), 0.35);
    border-radius: 5px;
    background: var(--panel-bg);
    overflow: hidden;
    font-size: 11px;
    color: var(--text-2);
  }
  .details-heading {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    margin-bottom: 0;
  }
  .section-name {
    width: 100%;
    font:
      800 14px Doto,
      monospace;
    text-transform: uppercase;
  }
  .section-actions {
    display: flex;
    gap: 6px;
  }
  /* One column under another, scrolling in the rail when they don't fit. */
  .section-columns {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    overflow: auto;
    scrollbar-width: none;
  }
  .section-column {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  h3 {
    margin: 0 0 2px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
  }
  h3 span {
    font-weight: 400;
    color: var(--text-3);
  }
  label {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  input,
  select,
  button {
    padding: 4px 7px;
    color: var(--text);
    background: transparent;
    border: 1px solid rgba(var(--ink), 0.2);
    border-radius: 3px;
    font: inherit;
  }
  input[type="number"] {
    width: 58px;
  }
  select {
    background: var(--panel-bg);
  }
  .section-track {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }
  .section-track-name {
    min-width: 110px;
    color: var(--text);
  }
  .section-layer {
    min-width: 26px;
    font-family: "Doto", monospace;
  }
  .section-layer.is-on {
    color: rgb(var(--accent));
    border-color: rgb(var(--accent));
  }
  .section-quiet {
    margin: 0;
    color: var(--text-3);
  }
  .section-practice {
    margin-left: auto;
  }
  .section-problem {
    margin: 4px 0 0;
    color: rgb(var(--rec));
  }
  .section-move {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--text);
  }
  .section-new-move {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }
  .section-value {
    width: 100%;
  }
</style>
