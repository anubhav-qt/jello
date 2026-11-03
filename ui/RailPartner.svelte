<!-- The jam partner in the rail (docs/design.md#ai-and-learning-rethought-2026-11-03):
     Start counts in a bar, then the player plays a 2 bar call and the AI
     answers it (src-tauri/src/jam_partner.rs). One line says whose turn it
     is; Stop ends it at once. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

  interface PartnerEvent {
    state: string;
    said: string;
    looping: boolean;
  }

  const STATE_WORDS: Record<string, string> = {
    count_in: "Counting in",
    call: "Your call: 2 bars",
    thinking: "Thinking of an answer",
    answer: "Jello answers",
  };

  let phase = $state("off");
  let looping = $state(false);
  let said = $state("");
  // A click on Start is on its way, so a quick second one waits.
  let busy = $state(false);
  let unlisten: UnlistenFn | null = null;

  let on = $derived(phase !== "off");

  onMount(async () => {
    unlisten = await listen<PartnerEvent>("partner", (event) => {
      phase = event.payload.state;
      looping = event.payload.looping;
      if (event.payload.state === "off") {
        said = event.payload.said;
      }
    });
  });

  onDestroy(() => {
    if (unlisten !== null) {
      unlisten();
    }
  });

  function stateWords(): string {
    const words = STATE_WORDS[phase] ?? "";
    if (phase === "thinking" && looping) {
      return `${words}, the beat goes on`;
    }
    return words;
  }

  async function start() {
    if (busy || on) {
      return;
    }
    busy = true;
    said = "";
    try {
      await invoke("start_partner");
      phase = "count_in";
    } catch (error) {
      said = String(error);
    }
    busy = false;
  }

  async function stop() {
    await invoke("stop_partner");
  }
</script>

<div class="agent-jam">
  <div class="agent-jam-top">
    <span class="agent-jam-title">Jam partner</span>
    {#if on}
      <span class="agent-quiet">{stateWords()}</span>
      <button type="button" class="agent-button-plain is-open" onclick={stop}>Stop</button>
    {:else}
      <button type="button" class="agent-button-plain" disabled={busy} onclick={start}>Start</button>
    {/if}
  </div>
  {#if !on && said !== ""}
    <span class="agent-quiet">{said}</span>
  {/if}
</div>
