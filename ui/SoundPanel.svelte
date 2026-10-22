<!-- The sound panel (docs/adr/0033-the-sound-panel.md): the song's sound,
     set while the keyboard keeps playing. It opens in Compose's tape's
     place, as wide as the keyboard's panel, on one of four pages: Notes,
     Filter, LFOs and Connections. A note at the top right says when
     changes are heard. While a value is picked, it says which, and what the
     arrow keys do to it. Clicking empty space in the panel lets go. -->
<script lang="ts">
  import type { Picked } from "./sound-scales";
  import type { SoundSender, SoundView } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundConnections from "./SoundConnections.svelte";
  import SoundFilter from "./SoundFilter.svelte";
  import SoundLfos from "./SoundLfos.svelte";
  import SoundNotes from "./SoundNotes.svelte";

  interface Props {
    // The song's sound, as Rust answered last.
    sound: SoundView;
    // How many key widths wide the keyboard is, so the panel is as wide.
    keysAcross: number;
    // The value picked for the arrow keys, if any, what picks one, and
    // what lets go.
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    onLetGo: () => void;
    send: SoundSender;
  }

  let { sound, keysAcross, picked, onPick, onLetGo, send }: Props = $props();

  type Page = "notes" | "filter" | "lfos" | "connections";

  const PAGES: { page: Page; name: string; note: string }[] = [
    { page: "notes", name: "Notes", note: "Changes start with the next key you press" },
    { page: "filter", name: "Filter", note: "Cutoff and resonance move notes already sounding" },
    { page: "lfos", name: "LFOs", note: "Changes here move notes already sounding" },
    { page: "connections", name: "Connections", note: "Changes here move notes already sounding" },
  ];

  // The page shown. It opens on the notes.
  let page: Page = $state("notes");

  let note = $derived.by(() => {
    for (const each of PAGES) {
      if (each.page === page) {
        return each.note;
      }
    }
    return "";
  });

  function showPage(newPage: Page) {
    page = newPage;
    onLetGo();
  }

  // Pressing on the panel anywhere but a value, a drawing's square or a
  // button lets go of the value picked.
  function letGoOutside(event: PointerEvent) {
    const target = event.target as Element;
    if (target.closest("button, .sound-value, .sound-handle") !== null) {
      return;
    }
    onLetGo();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section class="sound-panel" style:--keys-across={keysAcross} onpointerdown={letGoOutside}>
  <div class="sound-panel-top">
    <span class="sound-panel-title">Sound</span>
    {#each PAGES as each (each.page)}
      <button
        type="button"
        class="sound-page-tab"
        class:is-open={page === each.page}
        onclick={(event) => clickThenLetGo(event, () => showPage(each.page))}
      >
        {each.name}
      </button>
    {/each}

    {#if picked === null}
      <span class="sound-panel-note">{note}</span>
    {:else}
      <!-- The picked value's name, then the arrow keys and what they do. -->
      <span class="sound-pick-hint">
        <span class="sound-picked-name">{picked.name}</span>
        <i class="sound-arrow"><svg viewBox="0 0 10 10"><path d="M8 5 H2 M4.5 2.5 L2 5 L4.5 7.5" /></svg></i>
        <i class="sound-arrow"><svg viewBox="0 0 10 10"><path d="M2 5 H8 M5.5 2.5 L8 5 L5.5 7.5" /></svg></i>
        <span class="sound-arrow-words">step</span>
        <i class="sound-arrow-gap"></i>
        <i class="sound-arrow"><svg viewBox="0 0 10 10"><path d="M5 8 V2 M2.5 4.5 L5 2 L7.5 4.5" /></svg></i>
        <i class="sound-arrow"><svg viewBox="0 0 10 10"><path d="M5 2 V8 M2.5 5.5 L5 8 L7.5 5.5" /></svg></i>
        <span class="sound-arrow-words">ten steps</span>
      </span>
    {/if}
  </div>

  {#if page === "notes"}
    <SoundNotes {sound} {picked} {onPick} {send} />
  {:else if page === "filter"}
    <SoundFilter {sound} {picked} {onPick} {send} />
  {:else if page === "lfos"}
    <SoundLfos {sound} {picked} {onPick} {onLetGo} {send} />
  {:else}
    <SoundConnections {sound} {picked} {onPick} {onLetGo} {send} />
  {/if}
</section>
