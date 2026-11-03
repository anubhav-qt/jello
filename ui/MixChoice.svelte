<!-- A row of tiles on a Mix effect page, one of which is chosen, like a
     saturation's curve; or switches, each lit while on. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";

  interface Props {
    tiles: { name: string; chosen: boolean; title?: string; onClick: () => void }[];
    // The tiles' name above them, if any.
    label?: string;
    dim?: boolean;
  }

  let { tiles, label = "", dim = false }: Props = $props();
</script>

<div class="mix-choice-group" class:is-dim={dim}>
  {#if label !== ""}
    <div class="sound-value-name">{label}</div>
  {/if}
  <div class="mix-tiles">
    {#each tiles as tile (tile.name)}
      <button
        type="button"
        class="mix-tile"
        class:is-chosen={tile.chosen}
        title={tile.title ?? ""}
        onclick={(event) => clickThenLetGo(event, tile.onClick)}
      >
        {tile.name}
      </button>
    {/each}
  </div>
</div>
