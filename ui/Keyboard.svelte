<!-- The laptop keyboard, row by row, with each key's note from Rust. -->
<script lang="ts">
  import Key from "./Key.svelte";
  import { keyRows, type KeyLayout, type Platform } from "./keyboard";

  interface Props {
    platform: Platform;
    layout: KeyLayout;
    // The keys held down right now, by place, like "KeyQ".
    held: Set<string>;
    // The wave new notes play, like "saw", or "" until Rust has said.
    wave: string;
  }

  let { platform, layout, held, wave }: Props = $props();

  let rows = $derived(keyRows(platform));
</script>

<div class="keyboard" data-platform={platform}>
  {#each rows as row}
    <div class="keyboard-row" class:is-short={row.short === true}>
      {#each row.keys as spec}
        {#if spec.gap === true}
          <!-- Not a key, just room between keys. -->
          <div class="key-gap" style:--w={spec.width}></div>
        {:else if spec.below === undefined}
          <Key {spec} {layout} {wave} held={held.has(spec.code)} />
        {:else}
          <!-- A key with another stacked under it shares its spot in the row. -->
          <div class="key-stack" style:--w={spec.width}>
            <Key {spec} {layout} {wave} held={held.has(spec.code)} />
            <Key spec={spec.below} {layout} {wave} held={held.has(spec.below.code)} />
          </div>
        {/if}
      {/each}
    </div>
  {/each}
</div>
