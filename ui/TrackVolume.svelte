<!-- The picked track's volume: a row of white dots lit up to it, and its
     number, which can be typed over. Clicking a dot sets the volume there,
     in steps of 5%, and dragging along the dots follows the pointer; past
     the left end is silence. -->
<script lang="ts">
  import DigitField from "./DigitField.svelte";

  interface Props {
    // Counted from 1.
    track: number;
    // In percent, 0 to 100.
    volume: number;
    onVolume: (volume: number) => void;
  }
  let { track, volume, onVolume }: Props = $props();

  // Each dot is 5%.
  const DOTS = 20;

  // Any sound at all lights the first dot.
  let lit = $derived(Math.ceil(volume / (100 / DOTS)));

  let row: HTMLElement;
  // True from pressing on the dots until letting go.
  let dragging = false;
  // The volume sent last while dragging, so the same one is not sent again.
  let sent: number | null = null;

  // The volume under the pointer: the dot it is on, lit up to it.
  function volumeAt(x: number): number {
    const place = row.getBoundingClientRect();
    let along = (x - place.left) / place.width;
    if (along < 0) {
      along = 0;
    }
    if (along > 1) {
      along = 1;
    }
    const dots = Math.ceil(along * DOTS);
    return dots * (100 / DOTS);
  }

  function send(newVolume: number) {
    if (newVolume === sent) {
      return;
    }
    sent = newVolume;
    onVolume(newVolume);
  }

  function startDrag(event: PointerEvent) {
    dragging = true;
    sent = volume;
    row.setPointerCapture(event.pointerId);
    send(volumeAt(event.clientX));
  }

  function drag(event: PointerEvent) {
    if (!dragging) {
      return;
    }
    send(volumeAt(event.clientX));
  }

  function stopDrag() {
    dragging = false;
  }
</script>

<div class="track-volume">
  <span class="track-volume-label">Volume</span>
  <!-- The field next to it is the way in for keys and screen readers. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="track-volume-dots"
    bind:this={row}
    title={`Track ${track}'s volume: click or drag`}
    onpointerdown={startDrag}
    onpointermove={drag}
    onpointerup={stopDrag}
    onpointercancel={stopDrag}
  >
    {#each { length: DOTS } as _, place}
      <i class:is-lit={place < lit}></i>
    {/each}
  </div>
  <DigitField
    value={volume}
    lowest={0}
    highest={100}
    label={`Track ${track}'s volume in percent`}
    onSet={onVolume}
  />
  <span class="track-volume-unit">%</span>
</div>

<style>
  .track-volume {
    display: flex;
    flex: none;
    align-items: center;
    gap: 0.62em;
  }
  .track-volume-label {
    margin-right: 0.3em;
    font-size: 0.81em;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  /* Tall enough to hit easily, though the dots are small. */
  .track-volume-dots {
    display: flex;
    align-items: center;
    gap: 0.23em;
    height: 1.85em;
    cursor: pointer;
    touch-action: none;
  }
  .track-volume-dots i {
    width: 0.31em;
    height: 0.31em;
    border-radius: 1px;
    background: rgba(var(--ink), 0.14);
  }
  .track-volume-dots i.is-lit {
    background: rgba(var(--ink), 0.9);
    box-shadow: 0 0 4px rgba(var(--ink), 0.5);
  }
  .track-volume-unit {
    color: var(--text-3);
    font-size: 0.92em;
  }
</style>
