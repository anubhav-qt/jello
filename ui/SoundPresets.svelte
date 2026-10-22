<!-- The sound panel's Presets page (docs/adr/0037-presets.md): the sounds
     saved in Music/Jello/Sounds, as tiles sorted by name. Clicking one
     copies it into the song. A tile is lit while the song sounds exactly
     like it. The START mark in a tile's corner makes it the sound every new
     song starts with. At the right, the song's sound is saved by name.
     Renaming and removing presets is done in the folder. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { clickThenLetGo } from "./dom";
  import type { PresetView, SoundSender, SoundView } from "./sound";

  interface Props {
    sound: SoundView;
    send: SoundSender;
  }

  let { sound, send }: Props = $props();

  // The presets, as Rust listed them when the page opened or after the
  // last save or mark.
  let presets: PresetView[] = $state([]);

  // Whether the first list has come back, so "no presets yet" doesn't
  // flash while it is on its way.
  let listed = $state(false);

  // The name typed in the field, and why Rust says it can't name a preset,
  // or null if it can.
  let name = $state("");
  let problem: string | null = $state(null);

  async function relist() {
    presets = await send.listPresets();
    listed = true;
  }

  // The page lists the folder again each time it opens.
  onMount(() => {
    relist();
  });

  // Asks Rust about the name each time it changes. An answer about a name
  // typed since is left out.
  async function checkName() {
    const asked = name;
    const answer = await send.presetNameProblem(asked);
    if (name === asked) {
      problem = answer;
    }
  }

  // The preset the typed name would replace: one called the same, with
  // upper and lower case taken as the same. null if there is none.
  let replacing = $derived.by(() => {
    const typed = name.trim().toLowerCase();
    for (const preset of presets) {
      if (preset.name.toLowerCase() === typed) {
        return preset;
      }
    }
    return null;
  });

  let canSave = $derived(name.trim() !== "" && problem === null);

  let saveWord = $derived.by(() => {
    if (replacing === null) {
      return "Save";
    }
    return "Replace";
  });

  // The line under the button: why the name can't be used, what saving
  // replaces, or where presets are kept.
  let saveHint = $derived.by(() => {
    if (name.trim() !== "" && problem !== null) {
      return problem;
    }
    if (replacing !== null) {
      return `Saves over ${replacing.name}`;
    }
    return "Kept in Music/Jello/Sounds";
  });

  async function save() {
    if (!canSave) {
      return;
    }
    await send.savePreset(name);
    await relist();
  }

  // Marks `preset` as the starting sound, or unmarks it if it is already.
  async function switchStarting(preset: PresetView) {
    if (preset.starting) {
      await send.markStartingPreset(null);
    } else {
      await send.markStartingPreset(preset.name);
    }
    await relist();
  }
</script>

<div class="sound-modules" style:grid-template-columns="3fr 1fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Presets</span>
      <span class="sound-hint">click one to load it into the song</span>
    </div>
    <div class="preset-tiles">
      {#each presets as preset (preset.name)}
        <div
          class="sound-tile preset-tile"
          class:is-chosen={sound.preset === preset.name}
          class:is-dim={!preset.readable}
        >
          <button
            type="button"
            class="preset-load"
            disabled={!preset.readable}
            onclick={(event) => clickThenLetGo(event, () => send.loadPreset(preset.name))}
          >
            <span class="preset-name">{preset.name}</span>
            {#if !preset.readable}
              <span class="preset-unreadable">can't be read</span>
            {/if}
          </button>
          {#if preset.readable}
            <button
              type="button"
              class="preset-start"
              class:is-starting={preset.starting}
              onclick={(event) => clickThenLetGo(event, () => switchStarting(preset))}
            >
              Start
            </button>
          {/if}
        </div>
      {/each}
    </div>
    {#if listed && presets.length === 0}
      <p class="sound-empty">No presets yet. Save the song's sound at the right to make one.</p>
    {/if}
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Save</span>
      <span class="sound-hint">the song's sound</span>
    </div>
    <input
      class="preset-name-field"
      type="text"
      placeholder="Name"
      spellcheck="false"
      autocomplete="off"
      maxlength="60"
      bind:value={name}
      oninput={checkName}
    />
    <div class="sound-choice is-second" class:is-dim={!canSave}>
      <button
        type="button"
        disabled={!canSave}
        onclick={(event) => clickThenLetGo(event, save)}
      >
        {saveWord}
      </button>
    </div>
    <span class="preset-save-hint" class:is-problem={name.trim() !== "" && problem !== null}>{saveHint}</span>
  </div>
</div>
