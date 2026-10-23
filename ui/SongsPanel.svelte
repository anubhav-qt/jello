<!-- The Songs panel (docs/adr/0038-song-files.md): the songs in
     Music/Jello/Songs, as tiles sorted by name, with the open song lit.
     Clicking one opens it, after the open song is saved. "+ New song" makes
     a new one. At the right, the open song is renamed. Every song saves
     itself as it changes, so there is no save button. Songs are removed in
     the folder. The panel opens from the top bar's song name, in every tab,
     in the place of Compose's tape, as wide as the keyboard's panel. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { clickThenLetGo } from "./dom";
  import type { SongSender, SongsView, SongView } from "./songs";

  interface Props {
    // The songs and the one open, as Rust answered last, or null until it
    // has.
    songs: SongsView | null;
    // How many key widths wide the keyboard is, so the panel is as wide.
    keysAcross: number;
    send: SongSender;
  }

  let { songs, keysAcross, send }: Props = $props();

  // Whether the first list has come back, so "no songs yet" doesn't flash
  // while it is on its way.
  let listed = $state(false);

  // The panel lists the folder again each time it opens, so songs added,
  // renamed or removed there show up.
  onMount(() => {
    send.listSongs().then(() => {
      listed = true;
    });
  });

  let songList: SongView[] = $derived.by(() => {
    if (songs === null) {
      return [];
    }
    return songs.songs;
  });

  let openName = $derived.by(() => {
    if (songs === null) {
      return "";
    }
    return songs.open;
  });

  // The name typed in the field, which starts as the open song's, and why
  // Rust says the song can't be renamed that, or null if it can.
  let name = $state("");
  let problem: string | null = $state(null);

  // Each time another song opens or the song is renamed, the field starts
  // again from its name.
  $effect(() => {
    name = openName;
    problem = null;
  });

  // Asks Rust about the name each time it changes. An answer about a name
  // typed since is left out.
  async function checkName() {
    const asked = name;
    const answer = await send.songNameProblem(asked);
    if (name === asked) {
      problem = answer;
    }
  }

  let typed = $derived(name.trim());

  let canRename = $derived.by(() => {
    if (typed === "") {
      return false;
    }
    if (typed === openName) {
      return false;
    }
    return problem === null;
  });

  // The line under the button: why the name can't be used, or where songs
  // are kept.
  let renameHint = $derived.by(() => {
    if (typed !== "" && problem !== null) {
      return problem;
    }
    return "Kept in Music/Jello/Songs";
  });

  let showsProblem = $derived(typed !== "" && problem !== null);

  async function rename() {
    if (!canRename) {
      return;
    }
    await send.renameSong(typed);
  }

  // Return in the field renames, like the button.
  function renameOnReturn(event: KeyboardEvent) {
    if (event.code === "Enter" || event.code === "NumpadEnter") {
      rename();
    }
  }

  // The open song is open already, so clicking its tile does nothing.
  function open(song: SongView) {
    if (song.name === openName) {
      return;
    }
    send.openSong(song.name);
  }
</script>

<section class="sound-panel songs-panel" style:--keys-across={keysAcross}>
  <div class="sound-panel-top">
    <span class="sound-panel-title">Songs</span>
    <span class="sound-panel-note">Each song saves itself as it changes</span>
  </div>

  <div class="sound-modules" style:grid-template-columns="3fr 1fr">
    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">All songs</span>
        <span class="sound-hint">click one to open it</span>
      </div>
      <div class="preset-tiles">
        <button
          type="button"
          class="sound-tile song-tile is-new"
          onclick={(event) => clickThenLetGo(event, () => send.newSong())}
        >
          <span class="preset-name">+ New song</span>
        </button>
        {#each songList as song (song.name)}
          <button
            type="button"
            class="sound-tile song-tile"
            class:is-chosen={song.name === openName}
            class:is-dim={!song.readable}
            disabled={!song.readable}
            data-song={song.name}
            onclick={(event) => clickThenLetGo(event, () => open(song))}
          >
            <span class="preset-name">{song.name}</span>
            {#if !song.readable}
              <span class="preset-unreadable">can't be read</span>
            {/if}
          </button>
        {/each}
      </div>
      {#if listed && songList.length === 0}
        <p class="sound-empty">No songs yet.</p>
      {/if}
    </div>

    <div class="sound-module">
      <div class="sound-module-top">
        <span class="sound-label">Name</span>
        <span class="sound-hint">the open song</span>
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
        onkeydown={renameOnReturn}
      />
      <div class="sound-choice is-second" class:is-dim={!canRename}>
        <button type="button" disabled={!canRename} onclick={(event) => clickThenLetGo(event, rename)}>Rename</button>
      </div>
      <span class="preset-save-hint" class:is-problem={showsProblem}>{renameHint}</span>
    </div>
  </div>
</section>
