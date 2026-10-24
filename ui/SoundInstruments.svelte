<!-- The instruments to pick from (docs/design.md#sampler): the folders in
     Music/Jello/Instruments, as tiles sorted by name, with None first. The
     picked one is lavender, and says so while it loads or if it can't be
     read. Under them, the starters not fetched yet, each fetched only by
     its own Download button, with whose it is and on what terms, and a
     button that opens the folder. The Oscillators page shows the note
     starters and the Drums page the drum kits. -->
<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { clickThenLetGo } from "./dom";
  import type { InstrumentsView, InstrumentView, SoundSender, StarterProgress } from "./sound";

  interface Props {
    // The instrument picked now, and what picking one does: null is none.
    chosen: InstrumentView;
    onChoose: (name: string | null) => void;
    // True on the Drums page.
    drums: boolean;
    send: SoundSender;
  }

  let { chosen, onChoose, drums, send }: Props = $props();

  // The instruments, as Rust listed them when the page opened or after the
  // folder last changed. null until the first list comes back.
  let view: InstrumentsView | null = $state(null);

  // How far the starter downloading now has got, or null, and the starter
  // this page asked for, until its download is done.
  let progress: StarterProgress | null = $state(null);
  let asked: string | null = $state(null);

  async function relist() {
    const listed = await send.listInstruments();
    if (listed !== null) {
      view = listed;
    }
  }

  onMount(() => {
    relist();
    const stops = [
      listen("instruments-changed", () => {
        progress = null;
        relist();
      }),
      listen<StarterProgress>("starter-download", (event) => {
        progress = event.payload;
      }),
    ];
    return () => {
      for (const stop of stops) {
        stop.then((unlisten) => unlisten());
      }
    };
  });

  // The starters this page offers that aren't in the folder yet.
  let starters = $derived.by(() => {
    if (view === null) {
      return [];
    }
    return view.starters.filter((starter) => starter.drums === drums && !starter.fetched);
  });

  // Whether starter `name` is downloading now.
  function isDownloading(name: string, listedDownloading: boolean): boolean {
    if (listedDownloading) {
      return true;
    }
    return asked === name;
  }

  // Whether any starter is downloading, on either page: one at a time.
  let anyDownloading = $derived.by(() => {
    if (asked !== null) {
      return true;
    }
    if (view === null) {
      return false;
    }
    return view.starters.some((starter) => starter.downloading);
  });

  // How far starter `name` has got, like "34%".
  function percentOf(name: string): string {
    if (progress === null) {
      return "0%";
    }
    if (progress.name !== name) {
      return "0%";
    }
    if (progress.total === 0) {
      return "0%";
    }
    return `${Math.floor((progress.received / progress.total) * 100)}%`;
  }

  async function download(name: string) {
    asked = name;
    await send.downloadStarter(name);
    asked = null;
    await relist();
  }

  // What the picked tile says under its name while it isn't ready.
  let chosenWords = $derived.by(() => {
    if (chosen.state === "loading") {
      return "loading";
    }
    if (chosen.state === "failed") {
      return "can't be read";
    }
    return "";
  });

  // The picked instrument, if it is gone from the folder, still gets a
  // tile, so the song's pick shows.
  let names = $derived.by(() => {
    if (view === null) {
      return [];
    }
    const all = [...view.names];
    if (chosen.name !== null && !all.includes(chosen.name)) {
      all.push(chosen.name);
    }
    return all;
  });
</script>

<div class="instrument-tiles">
  <button
    type="button"
    class="sound-tile instrument-tile"
    class:is-chosen={chosen.name === null}
    onclick={(event) => clickThenLetGo(event, () => onChoose(null))}
  >
    <span class="instrument-name">None</span>
  </button>
  {#each names as name (name)}
    <button
      type="button"
      class="sound-tile instrument-tile"
      class:is-chosen={chosen.name === name}
      title={name}
      onclick={(event) => clickThenLetGo(event, () => onChoose(name))}
    >
      <span class="instrument-name">{name}</span>
      {#if chosen.name === name && chosenWords !== ""}
        <span class="instrument-state">{chosenWords}</span>
      {/if}
    </button>
  {/each}
</div>

{#if chosen.state === "failed" && chosen.problem !== null}
  <p class="instrument-problem">{chosen.problem}</p>
{/if}
{#if view !== null && view.names.length === 0}
  <p class="instrument-note">No instruments yet. Put a folder of SFZ or WAV and FLAC files in Instruments.</p>
{/if}

<div class="instrument-actions">
  {#each starters as starter (starter.name)}
    {#if isDownloading(starter.name, starter.downloading)}
      <span class="instrument-downloading">Downloading {starter.name} {percentOf(starter.name)}</span>
    {:else}
      <button
        type="button"
        class="instrument-action"
        disabled={anyDownloading}
        onclick={(event) => clickThenLetGo(event, () => download(starter.name))}
      >
        Download {starter.name}, {starter.megabytes} MB
      </button>
    {/if}
    <span class="instrument-credit">{starter.license}, {starter.credit}</span>
  {/each}
  <button
    type="button"
    class="instrument-action"
    onclick={(event) => clickThenLetGo(event, () => send.openInstrumentsFolder())}
  >
    Open Instruments folder
  </button>
</div>

<style>
  .instrument-tiles {
    display: flex;
    flex-wrap: wrap;
    align-content: flex-start;
    gap: 4px;
    min-height: 0;
    overflow-y: auto;
  }

  .instrument-tile {
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    max-width: 100%;
    height: auto;
    min-height: 19px;
    padding: 2px 7px;
  }

  .instrument-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .instrument-state {
    font-size: 10px;
    color: var(--text-2);
  }

  .instrument-problem,
  .instrument-note {
    margin: 6px 0 0;
    font-size: 11px;
    color: var(--text-3);
  }

  .instrument-actions {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    margin-top: 8px;
  }

  .instrument-action {
    padding: 2px 0;
    font-size: 11.5px;
    color: var(--text-2);
    transition: color 600ms ease-out;
  }

  .instrument-action:hover {
    color: var(--text);
    transition: color 40ms;
  }

  .instrument-action:disabled {
    opacity: 0.4;
  }

  .instrument-downloading {
    padding: 2px 0;
    font-size: 11.5px;
    color: rgb(var(--accent));
  }

  .instrument-credit {
    margin-bottom: 4px;
    font-size: 10px;
    color: var(--text-3);
  }
</style>
