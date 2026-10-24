<!-- The choices jello_options offers (docs/design.md#showing-instead-of-telling):
     2 to 4 chords, melodies or sounds, each with Play and Use. Play plays
     it on the keys; Use puts it in the song, and its Undo takes it back.
     Rust keeps the choices, so the panel only sends the offer's number and
     the choice's place (src-tauri/src/cards.rs). -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AgentCard from "./AgentCard.svelte";
  import type { OptionsCard, OptionView } from "./agent";

  interface Props {
    card: OptionsCard;
    // Whether the agent is still answering, so nothing is used meanwhile.
    working: boolean;
    // The song changed, by Use or Undo: redraw.
    onSongChanged: () => void;
  }
  let { card, working, onSongChanged }: Props = $props();

  // The choice used, with the number its Undo takes, or null.
  let used: { choice: number; reply: number } | null = $state(null);
  let undone = $state(false);
  let problem = $state("");

  async function play(choice: number) {
    problem = "";
    try {
      await invoke("play_agent_option", { offer: card.offer, choice });
    } catch (error) {
      problem = String(error);
    }
  }

  async function use(choice: number) {
    problem = "";
    try {
      const reply = await invoke<number>("use_agent_option", { offer: card.offer, choice });
      used = { choice, reply };
      undone = false;
    } catch (error) {
      problem = String(error);
    }
    onSongChanged();
  }

  async function undo() {
    if (used === null) {
      return;
    }
    problem = "";
    try {
      await invoke("undo_agent_reply", { reply: used.reply });
      undone = true;
    } catch (error) {
      problem = String(error);
    }
    onSongChanged();
  }

  // What a melody option plays, short: its first notes.
  function melodyWords(option: OptionView): string {
    if (option.presses === undefined) {
      return "";
    }
    const names = option.presses.slice(0, 8).map((press) => press.label);
    let words = names.join(" ");
    if (option.presses.length > names.length) {
      words += " …";
    }
    return words;
  }

  // One choice is used at a time: Undo it to use another.
  function useLocked(): boolean {
    if (working) {
      return true;
    }
    if (used === null) {
      return false;
    }
    return !undone;
  }

  function soundBefore(option: OptionView) {
    if (option.before === undefined) {
      return null;
    }
    return option.before;
  }

  function isUsed(choice: number): boolean {
    if (used === null || undone) {
      return false;
    }
    return used.choice === choice;
  }
</script>

<div class="agent-card agent-options">
  {#if card.title !== ""}
    <h3 class="agent-card-title">{card.title}</h3>
  {/if}
  {#each card.options as option, place (place)}
    <div class="agent-option" class:is-picked={isUsed(place)}>
      <div class="agent-option-top">
        <span class="agent-option-name">{option.name}</span>
        <span class="agent-option-buttons">
          <button type="button" class="agent-button-plain" onclick={() => play(place)}>Play</button>
          {#if isUsed(place)}
            <button type="button" class="agent-button-plain" disabled={working} onclick={undo}>Undo</button>
          {:else}
            <button
              type="button"
              class="agent-button-plain"
              disabled={useLocked()}
              onclick={() => use(place)}>Use</button
            >
          {/if}
        </span>
      </div>
      {#if option.chords !== undefined}
        <div class="agent-progression">
          {#each option.chords as chord, chordPlace (chordPlace)}
            <div class="agent-chord">
              {#if chord.numeral !== undefined}
                <span class="agent-chord-numeral">{chord.numeral}</span>
              {/if}
              <span class="agent-chord-name">{chord.name}</span>
              <span class="agent-quiet">{chord.hold} beats</span>
            </div>
          {/each}
        </div>
      {:else if option.presses !== undefined}
        <p class="agent-option-melody">{melodyWords(option)}</p>
      {:else if option.after !== undefined}
        <AgentCard
          card={{ kind: "sound", title: "", after: option.after, before: soundBefore(option) }}
          playhead={null}
        />
        {#if option.lines !== undefined}
          <ul class="agent-changes">
            {#each option.lines as line, linePlace (linePlace)}
              <li>{line}</li>
            {/each}
          </ul>
        {/if}
      {/if}
    </div>
  {/each}
  {#if problem !== ""}
    <p class="agent-error">{problem}</p>
  {/if}
</div>
