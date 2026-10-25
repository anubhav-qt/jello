<!-- The sections lane, under the progression on Compose's tape
     (docs/design.md#tracks-and-sections): one block per place in the order
     the song plays its sections, a play button for the whole order, and a
     button that saves the layers playing now as a new section. Clicking a
     block picks it and opens its details; clicking it again closes them. -->
<script lang="ts">
  import {
    sectionWords,
    type SectionEdit,
    type SectionsView,
  } from "./compose";
  import { clickThenLetGo } from "./dom";

  interface Props {
    sections: SectionsView | null;
    // Left Cmd on a Mac, left Alt on Windows.
    modifierName: string;
    // The place in the order picked, from 1, or null.
    picked: number | null;
    onPick: (place: number | null) => void;
    onEdit: (edit: SectionEdit) => void;
  }
  let { sections, modifierName, picked, onPick, onEdit }: Props = $props();

  // How wide one bar of a section is drawn, in pixels.
  const PIXELS_PER_BAR = 22;

  let isEmpty = $derived(sections === null || sections.order.length === 0);
  let isPlaying = $derived(sections !== null && sections.playing !== null);
  let pickedName = $derived.by(() => {
    if (sections === null || picked === null) {
      return null;
    }
    const number = sections.order[picked - 1];
    if (number === undefined) {
      return null;
    }
    return sections.sections[number - 1].name;
  });

  // A click on a block picks it and turns its layers on in the looper, at
  // its tempo; a click on the picked one closes it.
  function pick(place: number) {
    if (sections === null) {
      return;
    }
    if (picked === place) {
      onPick(null);
      return;
    }
    onPick(place);
    onEdit({ action: "open", section: sections.order[place - 1] });
  }

  function playOrStop() {
    if (isPlaying) {
      onEdit({ action: "stop" });
    } else {
      onEdit({ action: "play" });
    }
  }

  function move(place: number, to: number) {
    onEdit({ action: "move", place, to });
    onPick(to);
  }

  function useAgain(place: number) {
    onEdit({ action: "use_again", place });
    onPick(place + 1);
  }

  function remove(place: number) {
    onEdit({ action: "remove_use", place });
    onPick(null);
  }
</script>

<div class="tape-row sections-slot">
  <span class="tape-row-label">Sections</span>
  {#if sections === null || isEmpty}
    <p class="tape-empty sections-empty">
      Sections go here. Turn on the layers you want in the looper, then press
      {modifierName} + Return to save them as a section.
    </p>
  {:else}
    <div class="tape-lane">
      <button
        type="button"
        class="tape-play"
        aria-label={isPlaying ? "Stop the sections" : "Play the sections"}
        onclick={(event) => clickThenLetGo(event, playOrStop)}
      >
        {#if isPlaying}
          &#9632;
        {:else}
          &#9654;
        {/if}
      </button>
      <div class="tape-scroller">
        <div class="tape-track">
          {#each sections.order as number, index}
            {@const place = index + 1}
            {@const section = sections.sections[number - 1]}
            <button
              type="button"
              class="tape-block section-block"
              class:is-playing={sections.playing === place}
              class:is-selected={picked === place}
              style:width={`${section.bars * PIXELS_PER_BAR}px`}
              onclick={(event) => clickThenLetGo(event, () => pick(place))}
            >
              <span class="tape-block-name"
                ><span class="section-number">{number}</span>
                {section.name}</span
              >
              <span class="tape-block-length"
                >{sectionWords(section, sections.tempo)}</span
              >
            </button>
          {/each}
        </div>
      </div>
      <button
        type="button"
        class="section-save"
        title="Save the layers playing now as a section"
        onclick={(event) => clickThenLetGo(event, () => onEdit({ action: "save" }))}
      >
        + Save layers as section <kbd>{modifierName} Return</kbd>
      </button>
    </div>
  {/if}
</div>

{#if sections !== null && picked !== null && pickedName !== null}
  <div class="tape-row tape-bottom">
    <span class="tape-row-label">Section</span>
    <span class="tape-chosen">{pickedName}</span>
    <div class="tape-tools">
      <button
        type="button"
        disabled={picked === 1}
        onclick={(event) => clickThenLetGo(event, () => move(picked, picked - 1))}
        >← Move</button
      >
      <button
        type="button"
        disabled={picked === sections.order.length}
        onclick={(event) => clickThenLetGo(event, () => move(picked, picked + 1))}
        >Move →</button
      >
      <button
        type="button"
        onclick={(event) => clickThenLetGo(event, () => useAgain(picked))}
        >Use again</button
      >
      <button
        type="button"
        onclick={(event) => clickThenLetGo(event, () => remove(picked))}
        >Remove</button
      >
    </div>
  </div>
{/if}

<style>
  .sections-slot {
    height: 46px;
  }
  /* The save button follows the last block. */
  .sections-slot :global(.tape-scroller) {
    flex: 0 1 auto;
  }
  .sections-empty {
    font-size: 11px;
  }
  /* White blocks, lavender once picked, as wide as their bars. */
  .section-block {
    min-width: 96px;
    padding: 4px 7px;
    border-color: rgba(var(--ink), 0.35);
    background: rgba(var(--ink), 0.04);
  }
  .section-block:hover {
    background: rgba(var(--ink), 0.08);
  }
  .section-block :global(.tape-block-name) {
    font-size: 13px;
    line-height: 18px;
    color: var(--text);
  }
  .section-block.is-selected {
    border-color: rgb(var(--accent));
  }
  .section-block.is-selected :global(.tape-block-name) {
    color: rgb(var(--accent));
  }
  .section-block.is-playing {
    background: rgba(var(--ink), 0.16);
  }
  .section-number {
    font-family: "Doto", monospace;
    font-weight: 800;
    color: var(--text-3);
  }
  .section-block.is-selected .section-number {
    color: rgba(var(--accent), 0.6);
  }
  /* Dashed, like a block still to come. */
  .section-save {
    flex: none;
    margin: 0;
    padding: 0 12px;
    border: 1px dashed rgba(var(--ink), 0.2);
    border-radius: 4px;
    background: none;
    font: inherit;
    font-size: 11px;
    color: var(--text-2);
    white-space: nowrap;
    cursor: default;
  }
  .section-save:hover {
    border-color: rgba(var(--ink), 0.45);
    color: var(--text);
  }
  .section-save kbd {
    margin-left: 5px;
    font: 10px monospace;
    color: var(--text-3);
  }
</style>
