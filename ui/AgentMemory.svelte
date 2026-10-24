<!-- The agent panel's Memory page (docs/design.md#chats-caching-and-memory):
     the skills Jello measured from practice, ten dots each with a trend,
     then the agent's notes about the player by kind, each with Forget, and
     the switch that makes the agent ask before saving one. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { NOTE_KINDS, type MemoryNote, type MemoryView, type SkillView } from "./agent";

  let view: MemoryView | null = $state(null);
  let problem = $state("");

  const AREA_WORDS: Record<string, string> = {
    technique: "Technique",
    rhythm: "Rhythm",
    harmony: "Harmony",
    ear: "Ear",
    improvising: "Improvising",
    pieces: "Pieces",
    writing: "Writing",
  };

  const TREND_WORDS: Record<string, string> = {
    up: "going up",
    down: "going down",
    steady: "steady",
    new: "new",
  };

  const DOTS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  async function load() {
    try {
      view = await invoke<MemoryView>("memory_view");
      problem = "";
    } catch (error) {
      problem = String(error);
    }
  }

  async function forget(note: MemoryNote) {
    try {
      await invoke("forget_memory", { id: note.id });
    } catch (error) {
      problem = String(error);
    }
  }

  async function switchAskFirst() {
    if (view === null) {
      return;
    }
    try {
      await invoke("set_memory_ask_first", { askFirst: !view.ask_first });
    } catch (error) {
      problem = String(error);
    }
  }

  function askWords(memory: MemoryView): string {
    if (memory.ask_first) {
      return "on";
    }
    return "off";
  }

  function notesOf(kind: string): MemoryNote[] {
    if (view === null) {
      return [];
    }
    return view.notes.filter((note) => note.kind === kind);
  }

  function skillWords(skill: SkillView): string {
    if (!skill.measured) {
      return "not measured yet";
    }
    if (skill.tries === undefined || skill.tries === 0) {
      return "no tries yet";
    }
    let tries = `${skill.tries} tries`;
    if (skill.tries === 1) {
      tries = "1 try";
    }
    let trend = "";
    if (skill.trend !== null && skill.trend !== undefined) {
      trend = `, ${TREND_WORDS[skill.trend]}`;
    }
    return `${tries}${trend}`;
  }

  function level(skill: SkillView): number {
    if (skill.level === null || skill.level === undefined) {
      return 0;
    }
    return skill.level;
  }

  onMount(() => {
    load();
    const stops = [
      listen("memory-changed", () => load()),
      listen<{ tool: string }>("agent-acted", (event) => {
        if (event.payload.tool === "jello_memory") {
          load();
        }
      }),
      listen("practice-try", () => load()),
    ];
    return () => {
      for (const stop of stops) {
        stop.then((unlisten) => unlisten());
      }
    };
  });
</script>

<div class="agent-memory">
  <h1>Memory</h1>
  {#if problem !== ""}
    <p class="agent-error">{problem}</p>
  {/if}
  {#if view === null}
    <p class="agent-quiet">Reading the memory…</p>
  {:else}
    <h3 class="agent-chat-day">Skills</h3>
    {#each view.skills as skill (skill.area)}
      <div class="agent-skill" class:is-unmeasured={!skill.measured}>
        <span class="agent-skill-name">{AREA_WORDS[skill.area] ?? skill.area}</span>
        <span class="agent-skill-dots" aria-label="{level(skill)} of 10">
          {#each DOTS as dot (dot)}
            <span class:is-on={dot <= level(skill)}></span>
          {/each}
        </span>
        <span class="agent-quiet agent-skill-words">{skillWords(skill)}</span>
      </div>
    {/each}

    <div class="agent-memory-ask">
      <h3 class="agent-chat-day">Notes</h3>
      <button
        type="button"
        class="agent-button-plain"
        class:is-open={view.ask_first}
        title="The agent asks you before it saves a note"
        onclick={switchAskFirst}
      >
        Ask before saving: {askWords(view)}
      </button>
    </div>
    {#if view.notes.length === 0}
      <p class="agent-quiet">
        No notes yet. The agent keeps notes about how you play, what you like and what to do next, in
        Music/Jello/Memory.
      </p>
    {/if}
    {#each NOTE_KINDS as kind (kind.kind)}
      {@const kept = notesOf(kind.kind)}
      {#if kept.length > 0}
        <h4 class="agent-memory-kind">{kind.words}</h4>
        {#each kept as note (note.id)}
          <div class="agent-memory-note">
            <span class="agent-memory-text">{note.text}</span>
            <span class="agent-step-time">{note.date}</span>
            <button type="button" class="agent-button-plain" onclick={() => forget(note)}>Forget</button>
          </div>
        {/each}
      {/if}
    {/each}
  {/if}
</div>
