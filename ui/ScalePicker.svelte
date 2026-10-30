<!-- Picks a scale: its home note, then its kind. Keys play nothing until one
     is picked (docs/adr/0047-song-key-mappings.md). Compose shows it where the
     tape's progression goes, and Play where the held notes show. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";
  import { HOME_NOTES, SCALE_KINDS } from "./keys";
  import { colorForNote } from "./note-colors";

  interface Props {
    // The home note showing as picked, from 0 (C) to 11 (B), or null.
    tonic: number | null;
    // The kind picked now, or null.
    kind: string | null;
    onPick: (tonic: number, kind: string) => void;
  }
  let { tonic, kind, onPick }: Props = $props();

  // The home note clicked, until a kind finishes the pick.
  let chosenTonic: number | null = $state(null);
  let shownTonic = $derived.by(() => {
    if (chosenTonic !== null) {
      return chosenTonic;
    }
    return tonic;
  });

  function pickKind(picked: string) {
    if (shownTonic === null) {
      return;
    }
    onPick(shownTonic, picked);
  }
</script>

<div class="scale-picker">
  <span class="tape-label">Home</span>
  <div class="scale-row">
    {#each HOME_NOTES as name, pitch}
      <button
        type="button"
        class="tape-cell"
        class:is-picked={shownTonic === pitch}
        style:--accent={colorForNote(pitch).accent}
        onclick={(event) =>
          clickThenLetGo(event, () => {
            chosenTonic = pitch;
          })}
      >
        {name}
      </button>
    {/each}
  </div>
  <span class="tape-label">Scale</span>
  <div class="scale-row">
    {#each SCALE_KINDS as choice}
      <button
        type="button"
        class="scale-kind"
        class:is-picked={kind === choice && shownTonic === tonic}
        disabled={shownTonic === null}
        onclick={(event) => clickThenLetGo(event, () => pickKind(choice))}
      >
        {choice}
      </button>
    {/each}
  </div>
</div>
