<!-- A square on one of the sound panel's drawings, for dragging the values
     it shows: solid or hollow, ringed in lavender while its value is
     picked. A bigger, unseen square around it is easier to catch. -->
<svelte:options namespace="svg" />

<script lang="ts">
  import { dragInDrawing } from "./dom";

  interface Props {
    // Where it sits, in the drawing's own units.
    x: number;
    y: number;
    solid?: boolean;
    ringed: boolean;
    // Pressing on it picks its value; dragging calls `onMove` with where
    // the pointer is, in the drawing's units.
    onGrab: () => void;
    onMove: (x: number, y: number) => void;
  }

  let { x, y, solid = false, ringed, onGrab, onMove }: Props = $props();

  function grab(event: PointerEvent) {
    onGrab();
    dragInDrawing(event, onMove);
  }
</script>

<!-- Like the values, it is played with the pointer and the arrow keys. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<rect class="sound-handle" x={x - 8} y={y - 8} width="16" height="16" onpointerdown={grab} />
{#if ringed}
  <rect class="sound-ring" x={x - 7.5} y={y - 7.5} width="15" height="15" />
{/if}
<rect class="sound-square" class:is-solid={solid} x={x - 3.5} y={y - 3.5} width="7" height="7" />
