<!-- Practice lines in the rail (docs/design.md#ai-and-learning-rethought-2026-11-03):
     while a section of the player's song is practiced (PracticeLanes.svelte),
     each try in a line, a button to be coached on the last one, and what
     the looped bar needs: each skill with ten dots, its area, and the song
     and bar it came from (src-tauri/src/song_skills.rs). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { MemoryView, SongSkillView } from "./agent";
  import { offsetWords, type PracticeTry, type PracticeView, type Tally } from "./practice";

  interface Props {
    // Sends "Coach me on try N." to the agent.
    onCoach: (number: number) => void;
    // Whether the agent is answering, so the coach button waits.
    working: boolean;
  }
  let { onCoach, working }: Props = $props();

  const DOTS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  const AREA_WORDS: Record<string, string> = {
    technique: "Technique",
    rhythm: "Rhythm",
    harmony: "Harmony",
    ear: "Ear",
    improvising: "Improvising",
    pieces: "Pieces",
  };
  // How many tries the rail lists, newest last.
  const SHOWN_TRIES = 5;

  let view = $state<PracticeView | null>(null);
  let skills = $state<SongSkillView[]>([]);

  let song = $derived(view?.piece?.song ?? null);
  let tries = $derived(view?.tries ?? []);
  let shownTries = $derived(tries.slice(-SHOWN_TRIES));
  let loopBar: number | null = $derived.by(() => {
    const section = view?.settings?.section ?? null;
    if (section === null) {
      return null;
    }
    return Number(section.replace("Bar ", ""));
  });
  let barSkills: string[] = $derived.by(() => {
    if (song === null || loopBar === null) {
      return [];
    }
    return song.bar_skills[loopBar - 1] ?? [];
  });

  onMount(() => {
    invoke<PracticeView>("practice_view").then((found) => {
      view = found;
    });
    readSkills();
    const unlistenView = listen<PracticeView>("practice", (event) => {
      view = event.payload;
    });
    const unlistenTry = listen("practice-try", () => {
      readSkills();
    });
    return () => {
      unlistenView.then((unlisten) => unlisten());
      unlistenTry.then((unlisten) => unlisten());
    };
  });

  async function readSkills() {
    try {
      const memory = await invoke<MemoryView>("memory_view");
      skills = memory.song_skills ?? [];
    } catch {
      skills = [];
    }
  }

  // Why the piece has fewer notes than the track plays.
  function leftOutWords(count: number): string {
    if (count === 1) {
      return "1 note has no key in the song's keys now, so it is left out.";
    }
    return `${count} notes have no key in the song's keys now, so they are left out.`;
  }

  function handWords(name: string, tally: Tally | null | undefined): string {
    if (tally === null || tally === undefined) {
      return "";
    }
    return `${name} ${tally.hits}/${tally.notes}, ${offsetWords(tally.mean_offset_ms)}`;
  }

  // A try in one line, like "Try 3 · 84 bpm · Left 12/12, 8 ms late ·
  // Right 10/12, on time · Bar 2".
  function tryWords(attempt: PracticeTry): string {
    const parts = [`Try ${attempt.try}`, `${attempt.tempo} bpm`];
    const left = handWords("Left", attempt.left);
    if (left !== "") {
      parts.push(left);
    }
    const right = handWords("Right", attempt.right);
    if (right !== "") {
      parts.push(right);
    }
    if (attempt.section !== null && attempt.section !== undefined) {
      parts.push(attempt.section);
    }
    if (attempt.clean) {
      parts.push("clean");
    }
    return parts.join(" · ");
  }

  function skillOf(name: string): SongSkillView | null {
    for (const skill of skills) {
      if (skill.skill === name) {
        return skill;
      }
    }
    return null;
  }

  function levelOf(name: string): number {
    const level = skillOf(name)?.level;
    if (level === null || level === undefined) {
      return 0;
    }
    return level;
  }

  function skillWords(name: string): string {
    const skill = skillOf(name);
    if (skill === null) {
      return "new";
    }
    const area = AREA_WORDS[skill.area] ?? skill.area;
    if (skill.from === null) {
      return area;
    }
    return `${area} · ${skill.from.song} bar ${skill.from.bar}`;
  }
</script>

{#if view !== null && view.set && song !== null}
  <div class="agent-jam">
    <div class="agent-jam-top">
      <span class="agent-jam-title">Practice</span>
      <span class="agent-quiet">{song.section} · {song.track}</span>
    </div>
    {#if song.left_out > 0}
      <p class="agent-quiet">{leftOutWords(song.left_out)}</p>
    {/if}
    {#each shownTries as attempt (attempt.try)}
      <p class="agent-practice-try">{tryWords(attempt)}</p>
    {:else}
      <p class="agent-quiet">Start plays the section; Jello plays the other tracks.</p>
    {/each}
    {#if tries.length > 0}
      {@const last = tries[tries.length - 1]}
      <button
        type="button"
        class="agent-button-plain agent-practice-coach"
        disabled={working}
        onclick={() => onCoach(last.try)}>Coach me on try {last.try}</button
      >
    {/if}
    {#if loopBar !== null}
      <span class="agent-quiet">Bar {loopBar} is the loop until it is clean twice.</span>
      {#each barSkills as name (name)}
        <div class="agent-skill">
          <span class="agent-skill-name">{name}</span>
          <span class="agent-skill-dots" aria-label="{levelOf(name)} of 10">
            {#each DOTS as dot (dot)}
              <span class:is-on={dot <= levelOf(name)}></span>
            {/each}
          </span>
          <span class="agent-quiet agent-skill-words">{skillWords(name)}</span>
        </div>
      {/each}
    {/if}
  </div>
{/if}
