<!-- The lane under the keyboard
     (docs/adr/0025-the-looper-s-lane-as-wide-as-the-keyboard.md), as wide
     as the keyboard's panel, in every tab. It draws each layer of the loop
     in a box of its own (docs/adr/0026-layers.md): each note as a row of
     dots in its note color, low notes at the bottom, and a playhead that
     sweeps across as the loop plays. Up to four boxes sit side by side, and
     from the fifth on four sit on each level. Each box's number has a ring
     of dots around it, its volume knob. Before the loop, the lane shows the
     count-in, and before the first press how to start. Its right end says
     how long the loop is and holds the snap switch. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";
  import { colorForNote } from "./note-colors";
  import { BEATS_PER_BAR, barNow, isLoopPlaying, litBeat, loopBeats, type LayerView, type LoopNote, type SoundView } from "./sound";

  interface Props {
    // The wave, the tempo, snap and the looper.
    sound: SoundView;
    // How many beats into the count-in, the recording or the repeat playing
    // now the looper is, counted on to this frame.
    beats: number;
    // Where the lane sits, in pixels from the page's top left corner.
    left: number;
    top: number;
    width: number;
    // What the snap switch does.
    onSnapClick: () => void;
    // What turning a layer's knob does: `layer` counts from 1, and
    // `volume` is in percent, from 0 to 100.
    onVolumeChange: (layer: number, volume: number) => void;
  }

  let { sound, beats, left, top, width, onSnapClick, onVolumeChange }: Props = $props();

  // While the first loop is recorded its length isn't known yet, so the
  // box is marked out this many bars at a time.
  const BARS_MARKED_OUT = 4;
  // The count-in's beats.
  const COUNT_IN = [1, 2, 3, 4];
  // Up to this many boxes share a level.
  const BOXES_PER_LEVEL = 4;

  // The knob: dots on three quarters of a circle, from down left, over the
  // top, to down right, lit up to the volume.
  const RING_DOTS = 9;
  const RING_START = 135;
  const RING_TURN = 270;
  // Dragging the knob this many pixels up or down turns it all the way.
  const DRAG_FOR_FULL = 100;
  // A scroll's lines, in pixels.
  const LINE_PIXELS = 16;
  // How long the right end keeps showing the volume once the knob stops.
  const VOLUME_SHOWN_MS = 800;

  let looper = $derived(sound.looper);
  let looperState = $derived(looper.state);
  let lit = $derived(litBeat(looper, beats));
  let layers = $derived(looper.layers);

  // How many beats each box spans: the loop's length once it plays, and 4
  // bars at a time while the first loop is recorded.
  let span = $derived.by(() => {
    if (isLoopPlaying(looper)) {
      return loopBeats(looper);
    }
    const marked = BARS_MARKED_OUT * BEATS_PER_BAR;
    return (Math.floor(beats / marked) + 1) * marked;
  });

  // Where in a box a beat is drawn, as a share of its width in percent.
  function across(beat: number): number {
    return (beat / span) * 100;
  }

  // How the boxes are laid out: how many across, and on how many levels.
  let columns = $derived.by(() => {
    if (layers.length < 1) {
      return 1;
    }
    if (layers.length > BOXES_PER_LEVEL) {
      return BOXES_PER_LEVEL;
    }
    return layers.length;
  });
  let levels = $derived.by(() => {
    if (layers.length < 1) {
      return 1;
    }
    return Math.ceil(layers.length / columns);
  });
  let stacked = $derived(levels > 1);
  // The knob is a little smaller on short boxes.
  let ringSize = $derived.by(() => {
    if (stacked) {
      return 20;
    }
    return 22;
  });

  // The layer whose knob is being turned, from 1, or 0 for none, and the
  // volume it is turned to. Rust's answer can lag behind the knob, so the
  // knob shows its own volume while it turns.
  let turning = $state(0);
  let turnedVolume = $state(100);
  let dragging = $state(false);
  let dragStartY = 0;
  let dragStartVolume = 0;
  let turnTimer = 0;

  // How loud layer `number` (from 1) plays, in percent.
  function volumeOf(number: number, layer: LayerView): number {
    if (number === turning) {
      return turnedVolume;
    }
    return layer.volume;
  }

  // How many of the knob's dots are lit at `volume`.
  function litDots(volume: number): number {
    return Math.round((volume / 100) * RING_DOTS);
  }

  // Where the knob's dot `dot` sits, in pixels from its top left corner.
  function dotPlace(dot: number, size: number): { x: number; y: number } {
    const middle = size / 2;
    const radius = size / 2 - 1.6;
    const angle = ((RING_START + (dot * RING_TURN) / (RING_DOTS - 1)) * Math.PI) / 180;
    return { x: middle + radius * Math.cos(angle), y: middle + radius * Math.sin(angle) };
  }

  // The knob's dots, numbered from 0.
  const DOTS: number[] = [];
  for (let dot = 0; dot < RING_DOTS; dot++) {
    DOTS.push(dot);
  }

  // Notes dim with their layer's volume, but never quite out of sight.
  function noteOpacity(volume: number): number {
    return 0.25 + 0.75 * (volume / 100);
  }

  // Turns layer `number`'s knob to `volume`, kept from 0 to 100, and tells
  // Rust if it changed.
  function turnTo(number: number, volume: number) {
    const kept = Math.round(Math.min(100, Math.max(0, volume)));
    window.clearTimeout(turnTimer);
    if (turning !== number) {
      turning = number;
      turnedVolume = kept;
      onVolumeChange(number, kept);
      return;
    }
    if (kept !== turnedVolume) {
      turnedVolume = kept;
      onVolumeChange(number, kept);
    }
  }

  // The right end goes back to the loop's length a little after the knob
  // stops.
  function stopTurningSoon() {
    window.clearTimeout(turnTimer);
    turnTimer = window.setTimeout(() => {
      turning = 0;
    }, VOLUME_SHOWN_MS);
  }

  function startDrag(event: PointerEvent, number: number, layer: LayerView) {
    // Holding the knob never gives it the focus, so keys still play.
    event.preventDefault();
    const knob = event.currentTarget as HTMLElement;
    knob.setPointerCapture(event.pointerId);
    dragging = true;
    dragStartY = event.clientY;
    dragStartVolume = volumeOf(number, layer);
    turnTo(number, dragStartVolume);
  }

  function drag(event: PointerEvent, number: number) {
    if (!dragging) {
      return;
    }
    if (turning !== number) {
      return;
    }
    // Up turns it up.
    const moved = dragStartY - event.clientY;
    turnTo(number, dragStartVolume + (moved / DRAG_FOR_FULL) * 100);
  }

  function endDrag(event: PointerEvent) {
    if (!dragging) {
      return;
    }
    dragging = false;
    const knob = event.currentTarget as HTMLElement;
    knob.blur();
    stopTurningSoon();
  }

  // Two fingers moving up the trackpad turn it up, like dragging.
  function scroll(event: WheelEvent, number: number, layer: LayerView) {
    event.preventDefault();
    let pixels = event.deltaY;
    if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
      pixels = event.deltaY * LINE_PIXELS;
    }
    turnTo(number, volumeOf(number, layer) + (pixels / DRAG_FOR_FULL) * 100);
    stopTurningSoon();
  }

  // A double click sets it back to full.
  function reset(event: MouseEvent, number: number) {
    turnTo(number, 100);
    const knob = event.currentTarget as HTMLElement;
    knob.blur();
    stopTurningSoon();
  }

  // Each box's lowest and highest notes, so its notes spread over its
  // height.
  function lowestOf(notes: LoopNote[]): number {
    let low = 127;
    for (const played of notes) {
      low = Math.min(low, played.note);
    }
    return low;
  }
  function highestOf(notes: LoopNote[]): number {
    let high = 0;
    for (const played of notes) {
      high = Math.max(high, played.note);
    }
    return high;
  }

  // How high in its box a note is drawn, in percent from the bottom.
  function height(note: number, notes: LoopNote[]): number {
    const lowest = lowestOf(notes);
    const highest = highestOf(notes);
    let share = 0.5;
    if (highest > lowest) {
      share = (note - lowest) / (highest - lowest);
    }
    return 18 + share * 58;
  }

  // Where a note ends: where its key was let go, or, while still held, the
  // playhead.
  function endOf(played: LoopNote): number {
    if (played.end !== null) {
      return played.end;
    }
    return beats;
  }

  // A bar line between every two bars.
  let barLines = $derived.by(() => {
    const lines: number[] = [];
    for (let beat = BEATS_PER_BAR; beat < span; beat += BEATS_PER_BAR) {
      lines.push(beat);
    }
    return lines;
  });

  // What the right end says: the tempo until recording starts, then the
  // bar being recorded, then how long the loop is. While a knob turns, its
  // layer's volume, and while Esc waits, which layer it removes.
  let sideLabel = $derived.by(() => {
    if (looper.removing !== null) {
      return "Remove";
    }
    if (turning !== 0) {
      return `Vol ${turning}`;
    }
    if (looperState === "recording") {
      return "Bar";
    }
    if (isLoopPlaying(looper)) {
      return "Loop";
    }
    return "Tempo";
  });
  let sideValue = $derived.by(() => {
    if (looper.removing !== null) {
      return `layer ${looper.removing}`;
    }
    if (turning !== 0) {
      return `${turnedVolume}%`;
    }
    if (looperState === "recording") {
      return String(barNow(beats));
    }
    if (isLoopPlaying(looper)) {
      if (looper.number === 1) {
        return "1 bar";
      }
      return `${looper.number} bars`;
    }
    return `${sound.tempo} bpm`;
  });

  // What the snap switch says.
  let snapValue = $derived.by(() => {
    if (sound.snap) {
      return "on";
    }
    return "off";
  });
</script>

<div class="looper-lane" style:left={`${left}px`} style:top={`${top}px`} style:width={`${width}px`}>
  {#if looperState === "idle"}
    <div class="lane-loops">
      <span class="lane-number">1</span>
      <div class="lane-box is-empty">Right Shift records a loop</div>
    </div>
  {:else if looperState === "counting in"}
    <div class="lane-loops">
      <span class="lane-number">1</span>
      <div class="lane-box is-counting">
        {#each COUNT_IN as beat}
          <span class="lane-count" class:is-on={beat === lit} class:is-counted={beat < lit}>{beat}</span>
        {/each}
      </div>
    </div>
  {:else}
    <div
      class="lane-loops is-layers"
      class:is-stacked={stacked}
      class:is-picking={looper.removing !== null}
      style:--columns={columns}
      style:--levels={levels}
      style:--head={`${ringSize}px`}
    >
      {#each layers as layer, place}
        {@const number = place + 1}
        {@const volume = volumeOf(number, layer)}
        <span class="lane-head" class:is-turning={number === turning}>
          <button
            type="button"
            class="lane-knob"
            data-layer={number}
            title={`Layer ${number}'s volume: drag, scroll, or double-click for full`}
            style:width={`${ringSize}px`}
            style:height={`${ringSize}px`}
            onpointerdown={(event) => startDrag(event, number, layer)}
            onpointermove={(event) => drag(event, number)}
            onpointerup={endDrag}
            onpointercancel={endDrag}
            onwheel={(event) => scroll(event, number, layer)}
            ondblclick={(event) => reset(event, number)}
          >
            <svg width={ringSize} height={ringSize} viewBox={`0 0 ${ringSize} ${ringSize}`}>
              {#each DOTS as dot}
                {@const spot = dotPlace(dot, ringSize)}
                <circle
                  class="ring-dot"
                  class:is-lit={dot < litDots(volume)}
                  cx={spot.x.toFixed(2)}
                  cy={spot.y.toFixed(2)}
                  r="1.15"
                ></circle>
              {/each}
            </svg>
            <span class="ring-number" class:is-quiet={volume === 0}>{number}</span>
          </button>
        </span>
        <div
          class="lane-box"
          class:is-recording={layer.recording}
          class:is-going={looper.removing === number}
          data-layer={number}
        >
          {#if layer.recording}
            <span class="lane-recorded" style:width={`${across(beats)}%`}></span>
          {/if}
          {#each layer.notes as played}
            <span
              class="lane-note"
              data-note={played.note}
              style:left={`${across(played.start)}%`}
              style:width={`max(2px, calc(${across(endOf(played) - played.start)}% - 3px))`}
              style:bottom={`${height(played.note, layer.notes)}%`}
              style:opacity={noteOpacity(volume)}
              style:--note={colorForNote(played.note).accent}
            ></span>
          {/each}
          {#each barLines as beat}
            <span class="lane-bar-line" style:left={`${across(beat)}%`}></span>
          {/each}
          <span class="lane-playhead" class:is-recording={layer.recording} style:left={`${across(beats)}%`}></span>
        </div>
      {/each}
    </div>
  {/if}

  <div class="lane-side">
    <p class="lane-side-line">
      <span class="lane-label">{sideLabel}</span>
      <span class="lane-value">{sideValue}</span>
    </p>
    <button
      type="button"
      class="lane-snap"
      class:is-off={!sound.snap}
      title="Snap notes to the nearest half beat"
      onclick={(event) => clickThenLetGo(event, onSnapClick)}
    >
      <span class="lane-snap-label">Snap</span>
      <span class="lane-snap-value">{snapValue}</span>
    </button>
  </div>
</div>
