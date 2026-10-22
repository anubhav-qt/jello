<!-- The sound panel's LFOs page (docs/design.md#sound-panel): the song's 8
     LFOs, each a slow wave that moves whatever its connections say. The
     list on the left opens one; the rest of the page is the open one's
     shape, speed, delay and fade-in. Changes move notes already sounding. -->
<script lang="ts">
  import {
    evenScale,
    isPicked,
    LFO_BEATS,
    LFO_DELAY_BEATS,
    LFO_HZ,
    LFO_PRESSES,
    LFO_SHAPES,
    lfoHeight,
    nameWords,
    PERCENT,
    pickedOf,
    shapeGlyph,
    shapeJumps,
    TIME,
    type Picked,
    type Setting,
  } from "./sound-scales";
  import { LFO_MOST_STEPS, type LfoView, type SoundSender, type SoundView } from "./sound";
  import { clickThenLetGo } from "./dom";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    sound: SoundView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    onLetGo: () => void;
    send: SoundSender;
  }

  let { sound, picked, onPick, onLetGo, send }: Props = $props();

  // The LFO open now, counted from 0.
  let open = $state(0);
  let lfo = $derived(sound.lfos[open]);

  function openLfo(index: number) {
    open = index;
    onLetGo();
  }

  // The open LFO as it is now, with `change` made to it. It is sent as a
  // plain copy, not the page's live view of it.
  function sendLfo(change: Partial<LfoView>) {
    const now = $state.snapshot(sound.lfos[open]);
    send.setLfo(open, { ...now, ...change });
  }

  // What each LFO moves, from the connections, like "pitch, cutoff", or
  // "" for one connected to nothing.
  function movesOf(index: number): string {
    const source = `lfo ${index + 1}`;
    const targets: string[] = [];
    for (const connection of sound.connections) {
      if (connection.source === source) {
        targets.push(nameWords(connection.target));
      }
    }
    return targets.join(", ");
  }

  const speedHz: Setting = {
    id: "lfo speed hz",
    name: "Speed",
    scale: LFO_HZ,
    now: () => lfo.hz,
    set: (hz) => sendLfo({ hz }),
  };
  const speedBeats: Setting = {
    id: "lfo speed beats",
    name: "Speed",
    scale: LFO_BEATS,
    now: () => lfo.beats,
    set: (beats) => sendLfo({ beats }),
  };
  const speedPresses: Setting = {
    id: "lfo speed presses",
    name: "Speed",
    scale: LFO_PRESSES,
    now: () => lfo.presses,
    set: (presses) => sendLfo({ presses }),
  };
  const start: Setting = {
    id: "lfo start",
    name: "Start",
    scale: PERCENT,
    now: () => lfo.start,
    set: (newStart) => sendLfo({ start: newStart }),
  };
  const spread: Setting = {
    id: "lfo spread",
    name: "Spread",
    scale: PERCENT,
    now: () => lfo.spread,
    set: (newSpread) => sendLfo({ spread: newSpread }),
  };
  const smooth: Setting = {
    id: "lfo smooth",
    name: "Smooth",
    scale: PERCENT,
    now: () => lfo.smooth,
    set: (newSmooth) => sendLfo({ smooth: newSmooth }),
  };
  const delayMs: Setting = {
    id: "lfo delay ms",
    name: "Delay",
    scale: TIME,
    now: () => lfo.delay_ms,
    set: (ms) => sendLfo({ delay_ms: ms }),
  };
  const fadeMs: Setting = {
    id: "lfo fade ms",
    name: "Fade-in",
    scale: TIME,
    now: () => lfo.fade_ms,
    set: (ms) => sendLfo({ fade_ms: ms }),
  };
  const delayBeats: Setting = {
    id: "lfo delay beats",
    name: "Delay",
    scale: LFO_DELAY_BEATS,
    now: () => lfo.delay_beats,
    set: (beats) => sendLfo({ delay_beats: beats }),
  };
  const fadeBeats: Setting = {
    id: "lfo fade beats",
    name: "Fade-in",
    scale: LFO_DELAY_BEATS,
    now: () => lfo.fade_beats,
    set: (beats) => sendLfo({ fade_beats: beats }),
  };
  const stepCount: Setting = {
    id: "lfo step count",
    name: "Steps",
    scale: evenScale(1, LFO_MOST_STEPS, 1, "step", "steps"),
    now: () => lfo.step_count,
    set: (count) => sendLfo({ step_count: count }),
  };

  // The steps shape's step at `place`, from 0: its height, in percent.
  const STEP_HEIGHT = evenScale(-100, 100, 1, "%", "%");

  function stepSetting(place: number): Setting {
    return {
      id: `lfo step ${place}`,
      name: `Step ${place + 1}`,
      scale: STEP_HEIGHT,
      now: () => lfo.steps[place],
      set: (height) => {
        const steps = [...lfo.steps];
        steps[place] = height;
        sendLfo({ steps });
      },
    };
  }

  // The speed, delay and fade-in shown: in the open LFO's units.
  let speed = $derived.by(() => {
    if (lfo.speed === "beats") {
      return speedBeats;
    }
    if (lfo.speed === "presses") {
      return speedPresses;
    }
    return speedHz;
  });
  let inBeats = $derived(lfo.speed === "beats");
  let delay = $derived.by(() => {
    if (inBeats) {
      return delayBeats;
    }
    return delayMs;
  });
  let fade = $derived.by(() => {
    if (inBeats) {
      return fadeBeats;
    }
    return fadeMs;
  });

  // Switching units shows other values, so the one picked is let go.
  function setSpeedUnit(unit: string) {
    sendLfo({ speed: unit });
    onLetGo();
  }

  // The shape drawing, 286 by 160: two cycles from the start, or for the
  // steps shape one cycle of its steps. Heights go from -1 at the bottom
  // to 1 at the top.
  const LEFT = 14;
  const RIGHT = 272;
  const MID = 80;
  const SWING = 58;
  const POINTS = 400;
  const CYCLE = (RIGHT - LEFT) / 2;

  function heightY(height: number): number {
    return MID - height * SWING;
  }

  let isSteps = $derived(lfo.shape === "steps");

  let shapePath = $derived.by(() => {
    const points: string[] = [];
    for (let point = 0; point <= POINTS; point += 1) {
      const x = LEFT + ((RIGHT - LEFT) * point) / POINTS;
      const cycles = lfo.start / 100 + (x - LEFT) / CYCLE;
      const y = heightY(lfoHeight(lfo, cycles));
      points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  });

  // Where a per-note LFO's notes can start: from its start to as far on as
  // its spread, shaded.
  let bandEnd = $derived(LEFT + (lfo.spread / 100) * CYCLE);
  let startY = $derived(heightY(lfoHeight(lfo, lfo.start / 100)));
  let bandEndY = $derived(heightY(lfoHeight(lfo, (lfo.start + lfo.spread) / 100)));

  // Dragging the start's square pulls the wave along with the pointer, so
  // the start goes back as the pointer goes right. The first move only
  // says where the drag began.
  let dragFromX: number | null = null;
  let dragFromStart = 0;

  function grabStart() {
    dragFromX = null;
    dragFromStart = lfo.start;
    onPick(pickedOf(start));
  }

  function moveStart(x: number) {
    if (dragFromX === null) {
      dragFromX = x;
      return;
    }
    const moved = ((x - dragFromX) / CYCLE) * 100;
    const newStart = Math.round(dragFromStart - moved);
    // Round the cycle: 100% is back at 0%.
    start.set(((newStart % 100) + 100) % 100);
  }

  function moveSpread(x: number) {
    const share = (x - LEFT) / CYCLE;
    spread.set(PERCENT.at(share));
  }

  // The steps shape: each step's stretch, and the square on its held part.
  let stepWidth = $derived((RIGHT - LEFT) / lfo.step_count);

  let stepsPath = $derived.by(() => {
    const share = lfo.smooth / 100;
    const count = lfo.step_count;
    let path = `M${LEFT} ${heightY(lfo.steps[count - 1] / 100)}`;
    for (let place = 0; place < count; place += 1) {
      const x = LEFT + place * stepWidth;
      const y = heightY(lfo.steps[place] / 100);
      path += ` L ${x + share * stepWidth} ${y} L ${x + stepWidth} ${y}`;
    }
    return path;
  });

  // The faint lines between the steps.
  let dividerXs = $derived.by(() => {
    const xs: number[] = [];
    for (let place = 1; place < lfo.step_count; place += 1) {
      xs.push(LEFT + place * stepWidth);
    }
    return xs;
  });

  let stepSquares = $derived.by(() => {
    const share = lfo.smooth / 100;
    const squares: { x: number; y: number; setting: Setting }[] = [];
    for (let place = 0; place < lfo.step_count; place += 1) {
      const held = stepWidth * (1 - share);
      const x = LEFT + place * stepWidth + share * stepWidth + held / 2;
      const y = heightY(lfo.steps[place] / 100);
      squares.push({ x, y, setting: stepSetting(place) });
    }
    return squares;
  });

  function heightAt(y: number): number {
    const height = (MID - y) / SWING;
    return STEP_HEIGHT.at((height + 1) / 2);
  }

  // The count's square moves one step for every COUNT_DRAG across.
  const COUNT_DRAG = 8;
  let dragFromCount = 0;

  function grabCount() {
    dragFromX = null;
    dragFromCount = lfo.step_count;
    onPick(pickedOf(stepCount));
  }

  function moveCount(x: number) {
    if (dragFromX === null) {
      dragFromX = x;
      return;
    }
    const moved = Math.round((x - dragFromX) / COUNT_DRAG);
    const count = stepCount.scale.step(dragFromCount, moved);
    stepCount.set(count);
  }

  // The delay and fade-in drawing, 187 by 96: a wobble that waits, then
  // grows to its full size. Each part takes up to FADE_MOST on its curve.
  const FADE_LEFT = 10;
  const FADE_RIGHT = 177;
  const FADE_MOST = 78;
  const FADE_MID = 49;
  const FADE_SWING = 28;
  const WOBBLE = 22;

  let delayX = $derived.by(() => {
    const share = delay.scale.share(delay.now());
    return FADE_LEFT + share * FADE_MOST;
  });
  let fadeX = $derived.by(() => {
    const share = fade.scale.share(fade.now());
    return delayX + share * FADE_MOST;
  });

  // How big the wobble is at `x`: nothing while it waits, then growing.
  function sizeAt(x: number): number {
    if (x <= delayX) {
      return 0;
    }
    if (x >= fadeX) {
      return 1;
    }
    return (x - delayX) / (fadeX - delayX);
  }

  let wobblePath = $derived.by(() => {
    const points: string[] = [];
    for (let point = 0; point <= 240; point += 1) {
      const x = FADE_LEFT + ((FADE_RIGHT - FADE_LEFT) * point) / 240;
      const turn = (2 * Math.PI * (x - FADE_LEFT)) / WOBBLE;
      const swing = sizeAt(x) * FADE_SWING * Math.sin(turn);
      const y = FADE_MID - swing;
      points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    }
    return "M" + points.join(" L ");
  });

  function moveDelay(x: number) {
    const share = (x - FADE_LEFT) / FADE_MOST;
    delay.set(delay.scale.at(share));
  }

  function moveFade(x: number) {
    const share = (x - delayX) / FADE_MOST;
    fade.set(fade.scale.at(share));
  }

  let shared = $derived(!lfo.per_note);
</script>

<div class="sound-modules" style:grid-template-columns="176fr 330fr 250fr 231fr">
  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">LFOs</span>
      <span class="sound-hint">what each moves</span>
    </div>
    <div class="sound-list is-lfos">
      {#each sound.lfos as each, index}
        <button
          type="button"
          class="sound-list-row is-lfo"
          class:is-open={index === open}
          class:is-unused={movesOf(index) === ""}
          onclick={(event) => clickThenLetGo(event, () => openLfo(index))}
        >
          <span class="sound-list-number">{index + 1}</span>
          <svg class="sound-glyph" viewBox="0 0 20 12"><path d={shapeGlyph(each.shape)} /></svg>
          <span class="sound-list-words">
            {#if movesOf(index) === ""}
              not connected
            {:else}
              {movesOf(index)}
            {/if}
          </span>
        </button>
      {/each}
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">LFO {open + 1} shape</span>
      <span class="sound-hint">{lfo.shape}</span>
    </div>
    <div class="sound-drawing is-fixed" style:height="160px">
      <svg viewBox="0 0 286 160" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M{LEFT} {MID} L {RIGHT} {MID}" />
        {#if isSteps}
          {#each dividerXs as x}
            <path class="sound-lit-soft is-faint" d="M{x} 18 L {x} 142" />
          {/each}
          <path class="sound-lit" d={stepsPath} />
          {#each stepSquares as square}
            <SoundHandle
              x={square.x}
              y={square.y}
              solid
              ringed={isPicked(picked, square.setting.id)}
              onGrab={() => onPick(pickedOf(square.setting))}
              onMove={(_x, y) => square.setting.set(heightAt(y))}
            />
          {/each}
          <path class="sound-lit-soft" d="M{RIGHT} 10 L {RIGHT} 150" />
          <SoundHandle x={RIGHT} y={150} ringed={isPicked(picked, stepCount.id)} onGrab={grabCount} onMove={moveCount} />
          <text class="sound-axis" text-anchor="end" x={RIGHT - 8} y="154">
            {lfo.step_count}
            {stepCount.scale.unit(lfo.step_count).toUpperCase()}
          </text>
        {:else}
          {#if lfo.per_note}
            <rect class="sound-band" x={LEFT} y="10" width={bandEnd - LEFT} height="140" />
            <path class="sound-lit-soft" d="M{bandEnd} 10 L {bandEnd} 150" />
            <text class="sound-axis" x={LEFT + 2} y="155">EACH NOTE STARTS IN THE SHADED PART</text>
          {/if}
          <path class="sound-lit" d={shapePath} />
          <SoundHandle x={LEFT} y={startY} solid ringed={isPicked(picked, start.id)} onGrab={grabStart} onMove={moveStart} />
          {#if lfo.per_note}
            <SoundHandle
              x={bandEnd}
              y={bandEndY}
              ringed={isPicked(picked, spread.id)}
              onGrab={() => onPick(pickedOf(spread))}
              onMove={moveSpread}
            />
          {/if}
        {/if}
      </svg>
    </div>
    <div class="sound-shapes">
      {#each LFO_SHAPES as shape (shape.name)}
        <button
          type="button"
          class="sound-shape"
          class:is-chosen={lfo.shape === shape.name}
          title={shape.name}
          onclick={(event) => clickThenLetGo(event, () => sendLfo({ shape: shape.name }))}
        >
          <svg class="sound-glyph" viewBox="0 0 20 12"><path d={shape.glyph} /></svg>
        </button>
      {/each}
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Speed</span>
      <span class="sound-hint">
        {#if lfo.per_note}
          each note runs its own
        {:else}
          one runs for every note
        {/if}
      </span>
    </div>
    <div class="sound-choice">
      <button
        type="button"
        class:is-chosen={lfo.per_note}
        onclick={(event) => clickThenLetGo(event, () => sendLfo({ per_note: true }))}
      >
        Per note
      </button>
      <button
        type="button"
        class:is-chosen={shared}
        onclick={(event) => clickThenLetGo(event, () => sendLfo({ per_note: false }))}
      >
        Shared
      </button>
    </div>
    <div class="sound-choice is-second">
      <button
        type="button"
        class:is-chosen={lfo.speed === "hz"}
        onclick={(event) => clickThenLetGo(event, () => setSpeedUnit("hz"))}
      >
        Hz
      </button>
      <button
        type="button"
        class:is-chosen={lfo.speed === "beats"}
        onclick={(event) => clickThenLetGo(event, () => setSpeedUnit("beats"))}
      >
        Beats
      </button>
      <button
        type="button"
        class:is-chosen={lfo.speed === "presses"}
        onclick={(event) => clickThenLetGo(event, () => setSpeedUnit("presses"))}
      >
        Presses
      </button>
    </div>
    <div class="sound-fields is-speed" style:grid-template-columns="1fr 1fr">
      <SoundValue setting={speed} dots={14} {picked} {onPick} />
      <SoundValue setting={start} dots={14} {picked} {onPick} />
      <SoundValue setting={spread} dots={14} {picked} {onPick} dim={shared} />
      <SoundValue setting={smooth} dots={14} {picked} {onPick} dim={!shapeJumps(lfo.shape)} />
    </div>
  </div>

  <div class="sound-module">
    <div class="sound-module-top">
      <span class="sound-label">Delay, fade-in</span>
      <!-- Empty, to keep the name level with the others (sound-panel.css). -->
      <span class="sound-hint">&nbsp;</span>
    </div>
    <div class="sound-drawing is-fixed" style:height="96px">
      <svg viewBox="0 0 187 96" preserveAspectRatio="xMidYMid meet">
        <path class="sound-lit-soft" d="M{delayX} {FADE_MID} L {fadeX} {FADE_MID - FADE_SWING} L {FADE_RIGHT} {FADE_MID - FADE_SWING}" />
        <path class="sound-lit-soft" d="M{delayX} {FADE_MID} L {fadeX} {FADE_MID + FADE_SWING} L {FADE_RIGHT} {FADE_MID + FADE_SWING}" />
        <path class="sound-lit is-fine" d={wobblePath} />
        <text class="sound-axis" x={FADE_LEFT} y="93">NOTE STARTS</text>
        <SoundHandle
          x={delayX}
          y={FADE_MID}
          solid
          ringed={isPicked(picked, delay.id)}
          onGrab={() => onPick(pickedOf(delay))}
          onMove={moveDelay}
        />
        <SoundHandle
          x={fadeX}
          y={FADE_MID - FADE_SWING}
          solid
          ringed={isPicked(picked, fade.id)}
          onGrab={() => onPick(pickedOf(fade))}
          onMove={moveFade}
        />
      </svg>
    </div>
    <div class="sound-fields" style:grid-template-columns="1fr 1fr">
      <SoundValue setting={delay} dots={12} {picked} {onPick} />
      <SoundValue setting={fade} dots={12} {picked} {onPick} />
    </div>
    <div class="sound-choice is-last" class:is-dim={shared}>
      <button
        type="button"
        class:is-chosen={!lfo.once}
        onclick={(event) => clickThenLetGo(event, () => sendLfo({ once: false }))}
      >
        Keeps going
      </button>
      <button
        type="button"
        class:is-chosen={lfo.once}
        onclick={(event) => clickThenLetGo(event, () => sendLfo({ once: true }))}
      >
        Once
      </button>
    </div>
  </div>
</div>
