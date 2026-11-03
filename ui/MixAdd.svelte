<!-- "+ Add effect", open on the Mix panel's right: every kind of effect as
     a tile, with what it does. A click puts it in the slot. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";
  import { KINDS } from "./mix";

  interface Props {
    onChoose: (kind: string) => void;
  }

  let { onChoose }: Props = $props();
</script>

<div class="mix-add-list">
  {#each KINDS as each (each.kind)}
    <button
      type="button"
      class="mix-add-kind"
      onclick={(event) => clickThenLetGo(event, () => onChoose(each.kind))}
    >
      <span class="mix-add-name">{each.name}</span>
      <span class="mix-add-words">{each.words}</span>
    </button>
  {/each}
</div>
