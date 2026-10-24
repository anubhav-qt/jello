<!-- A card Jello draws in the agent's reply (docs/design.md#showing-instead-of-telling):
     keys marked, a lane of notes, a progression, takes compared, a chart,
     a sound, the player's presses or a render's meters. Rust checked every
     field (src-tauri/src/cards.rs). Options have their own card,
     AgentOptions.svelte. -->
<script lang="ts">
  import {
    envelopePath,
    envelopeSpanMs,
    filterPath,
    gapLabel,
    laneRange,
    shareOf,
    type Card,
    type LaneCard,
    type SoundSketch,
  } from "./agent";
  import { colorForNote } from "./note-colors";

  interface Props {
    card: Card;
    // Where jello_play is in the lane, in beats, or null while nothing plays.
    playhead: number | null;
  }
  let { card, playhead }: Props = $props();

  // The lane's drawing, in its own units.
  const LANE_WIDTH = 400;
  const LANE_ROW = 5;
  const LANE_LEAST_HEIGHT = 48;

  // The sound's drawings.
  const SKETCH_WIDTH = 180;
  const SKETCH_HEIGHT = 56;

  // The chart's drawing.
  const CHART_WIDTH = 400;
  const CHART_HEIGHT = 90;

  const FIT_WORDS: Record<string, string> = {
    in_key: "in key",
    can_work: "can work",
    outside: "outside",
  };

  function color(pitch: number | null): string {
    if (pitch === null) {
      return "var(--ink)";
    }
    return colorForNote(pitch).accent;
  }

  function laneHeight(lane: LaneCard): number {
    const range = laneRange(lane);
    const rows = range.high - range.low + 1;
    return Math.max(LANE_LEAST_HEIGHT, rows * LANE_ROW);
  }

  function laneY(lane: LaneCard, pitch: number): number {
    const range = laneRange(lane);
    return (range.high - pitch) * LANE_ROW + (laneHeight(lane) - (range.high - range.low + 1) * LANE_ROW) / 2;
  }

  function laneX(lane: LaneCard, beats: number): number {
    return (beats / lane.beats) * LANE_WIDTH;
  }

  function beatLines(lane: LaneCard): number[] {
    const lines: number[] = [];
    for (let beat = 1; beat < lane.beats; beat += 1) {
      lines.push(beat);
    }
    return lines;
  }

  function sketches(after: SoundSketch, before: SoundSketch | null | undefined): SoundSketch[] {
    if (before === null || before === undefined) {
      return [after];
    }
    return [after, before];
  }

  function chartPoints(values: number[]): string {
    let low = Math.min(...values);
    let high = Math.max(...values);
    if (low === high) {
      low -= 1;
      high += 1;
    }
    const points: string[] = [];
    for (let place = 0; place < values.length; place += 1) {
      const x = (place / (values.length - 1)) * CHART_WIDTH;
      const y = CHART_HEIGHT - 8 - ((values[place] - low) / (high - low)) * (CHART_HEIGHT - 16);
      points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return points.join(" ");
  }

  function decibels(value: number | null): string {
    if (value === null) {
      return "silent";
    }
    return `${value} dB`;
  }

  function hertz(value: number | null): string {
    if (value === null) {
      return "none";
    }
    if (value >= 1000) {
      return `${(value / 1000).toFixed(1)} kHz`;
    }
    return `${value} Hz`;
  }

  function percent(value: number | null): string {
    if (value === null) {
      return "none";
    }
    return `${value}%`;
  }

  // Where brightness falls from 100 Hz to 10 kHz, which are far apart, so
  // by their logarithm.
  function brightnessShare(value: number | null): number {
    if (value === null || value <= 0) {
      return 0;
    }
    return shareOf(Math.log10(value), 2, 4);
  }

  // A bar's width for a percentage, none when there is no value.
  function barWidth(value: number | null): string {
    if (value === null) {
      return "0%";
    }
    return `${value}%`;
  }

  function silentBeats(silence: { from_beat: number; to_beat: number }): string {
    return `Silent from beat ${silence.from_beat} to ${silence.to_beat}`;
  }
</script>

{#snippet soundSketch(after: SoundSketch, before: SoundSketch | null | undefined)}
  {@const span = envelopeSpanMs(sketches(after, before))}
  <div class="agent-sketch">
    <figure>
      <svg viewBox={`0 0 ${SKETCH_WIDTH} ${SKETCH_HEIGHT}`} aria-hidden="true">
        {#if before !== null && before !== undefined}
          <path
            class="agent-sketch-before"
            d={envelopePath(before.loudness, SKETCH_WIDTH, SKETCH_HEIGHT, span)}
          />
        {/if}
        <path d={envelopePath(after.loudness, SKETCH_WIDTH, SKETCH_HEIGHT, span)} />
      </svg>
      <figcaption>Loudness</figcaption>
    </figure>
    <figure>
      <svg viewBox={`0 0 ${SKETCH_WIDTH} ${SKETCH_HEIGHT}`} aria-hidden="true">
        {#if before !== null && before !== undefined}
          <path class="agent-sketch-before" d={filterPath(before.filter, SKETCH_WIDTH, SKETCH_HEIGHT)} />
        {/if}
        <path d={filterPath(after.filter, SKETCH_WIDTH, SKETCH_HEIGHT)} />
      </svg>
      <figcaption>{after.filter.type} {hertz(after.filter.cutoff_hz)}</figcaption>
    </figure>
    <div class="agent-sketch-oscillators">
      {#each after.oscillators as oscillator, place (place)}
        <div class="agent-meter-row">
          <span class="agent-meter-name">{place + 1} {oscillator.kind}</span>
          <span class="agent-meter-bar"><span style:width={`${oscillator.level}%`}></span></span>
          <span class="agent-meter-value">{oscillator.level}%</span>
        </div>
      {/each}
    </div>
  </div>
{/snippet}

<div class="agent-card" data-kind={card.kind}>
  {#if card.title !== ""}
    <h3 class="agent-card-title">{card.title}</h3>
  {/if}

  {#if card.kind === "keys"}
    <div class="agent-card-keys">
      {#each card.keys as key, place (place)}
        <span class="agent-mark-chip">
          <span class="agent-mark">{key.label}</span>
          <span>{key.what}</span>
        </span>
      {/each}
    </div>
    <p class="agent-quiet agent-card-note">Marked on the keyboard</p>
  {:else if card.kind === "lane"}
    {@const height = laneHeight(card)}
    <div class="agent-lane-parts">
      {#each card.parts as part (part.name)}
        <span>{part.name}</span>
      {/each}
    </div>
    <svg class="agent-lane" viewBox={`0 0 ${LANE_WIDTH} ${height}`} preserveAspectRatio="none" aria-hidden="true">
      {#each beatLines(card) as beat (beat)}
        <line
          class:is-bar={beat % 4 === 0}
          x1={laneX(card, beat)}
          x2={laneX(card, beat)}
          y1="0"
          y2={height}
        />
      {/each}
      {#each card.parts as part, partPlace (partPlace)}
        {#each part.notes as note, notePlace (notePlace)}
          {#each note.pitches as pitch (pitch)}
            <rect
              x={laneX(card, note.start)}
              y={laneY(card, pitch)}
              width={Math.max(1, laneX(card, note.hold) - 1)}
              height={LANE_ROW - 1}
              rx="1"
              style:fill={`rgb(${color(pitch)})`}
              class:is-second={partPlace % 2 === 1}
            />
          {/each}
        {/each}
      {/each}
      {#if playhead !== null && playhead <= card.beats}
        <line class="agent-lane-playhead" x1={laneX(card, playhead)} x2={laneX(card, playhead)} y1="0" y2={height} />
      {/if}
    </svg>
    <div class="agent-lane-beats agent-quiet">
      <span>1</span>
      <span>{card.beats} beats</span>
    </div>
  {:else if card.kind === "progression"}
    <p class="agent-quiet agent-card-note">{card.key}</p>
    <div class="agent-progression">
      {#each card.chords as chord, place (place)}
        <div class="agent-chord" data-fit={chord.fit}>
          <span class="agent-chord-numeral">{chord.numeral}</span>
          <span class="agent-chord-name">{chord.name}</span>
          <span class="agent-quiet">{FIT_WORDS[chord.fit]}</span>
        </div>
      {/each}
    </div>
  {:else if card.kind === "compare"}
    <div class="agent-compare" style:--columns={card.items.length}>
      {#each card.items as item, place (place)}
        <div class="agent-compare-item">
          <h4>{item.name}</h4>
          {#each item.measures as measure, measurePlace (measurePlace)}
            <div class="agent-compare-measure">
              <span class="agent-quiet">{measure.name}</span>
              <span class="agent-compare-value">{measure.value}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {:else if card.kind === "chart"}
    {@const values = card.points.map((point) => point.value)}
    <svg class="agent-chart" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} preserveAspectRatio="none" aria-hidden="true">
      <polyline points={chartPoints(values)} />
    </svg>
    <div class="agent-chart-labels">
      <span class="agent-quiet">{card.points[0].label}</span>
      <span class="agent-compare-value"
        >{card.points[card.points.length - 1].value} {card.unit}</span
      >
      <span class="agent-quiet">{card.points[card.points.length - 1].label}</span>
    </div>
  {:else if card.kind === "sound"}
    {@render soundSketch(card.after, card.before)}
    {#if card.before !== null && card.before !== undefined}
      <p class="agent-quiet agent-card-note">Dashed: the sound before</p>
    {/if}
  {:else if card.kind === "presses"}
    {#if card.presses.length === 0}
      <p class="agent-quiet agent-card-note">No presses yet</p>
    {/if}
    <div class="agent-presses">
      {#each card.presses as press, place (place)}
        <span class="agent-press" style:--accent={color(press.pitch)}>
          <span class="agent-press-name">{press.label}</span>
          {#if press.held_ms === null}
            <span class="agent-quiet">held</span>
          {:else}
            <span class="agent-quiet">{press.held_ms} ms</span>
          {/if}
        </span>
        {#if place + 1 < card.presses.length}
          {@const gap = gapLabel(press, card.presses[place + 1])}
          {#if gap !== ""}
            <span class="agent-press-gap">{gap}</span>
          {/if}
        {/if}
      {/each}
    </div>
  {:else if card.kind === "listen"}
    <div class="agent-meters">
      <div class="agent-meter-row">
        <span class="agent-meter-name">Loudness</span>
        <span class="agent-meter-bar"><span style:width={`${shareOf(card.whole.loudness_db, -60, 0) * 100}%`}></span></span>
        <span class="agent-meter-value">{decibels(card.whole.loudness_db)}</span>
      </div>
      <div class="agent-meter-row" class:is-over={card.whole.soft_clip_reached}>
        <span class="agent-meter-name">Peak</span>
        <span class="agent-meter-bar"><span style:width={`${shareOf(card.whole.peak_db, -60, 0) * 100}%`}></span></span>
        <span class="agent-meter-value">{decibels(card.whole.peak_db)}</span>
      </div>
      <div class="agent-meter-row">
        <span class="agent-meter-name">Brightness</span>
        <span class="agent-meter-bar"><span style:width={`${brightnessShare(card.whole.brightness_hz) * 100}%`}></span></span>
        <span class="agent-meter-value">{hertz(card.whole.brightness_hz)}</span>
      </div>
      <div class="agent-meter-row">
        <span class="agent-meter-name">Bands</span>
        <span class="agent-meter-bar agent-bands">
          <span style:width={barWidth(card.whole.lows_percent)}></span>
          <span style:width={barWidth(card.whole.mids_percent)}></span>
          <span style:width={barWidth(card.whole.highs_percent)}></span>
        </span>
        <span class="agent-meter-value"
          >{percent(card.whole.lows_percent)} {percent(card.whole.mids_percent)} {percent(card.whole.highs_percent)}</span
        >
      </div>
      <div class="agent-meter-row">
        <span class="agent-meter-name">Width</span>
        <span class="agent-meter-bar"><span style:width={barWidth(card.whole.width_percent)}></span></span>
        <span class="agent-meter-value">{percent(card.whole.width_percent)}</span>
      </div>
    </div>
    {#if card.loudness.length > 1}
      <div class="agent-beats" aria-label="Loudness beat by beat">
        {#each card.loudness as beat, place (place)}
          <span style:height={`${Math.max(4, shareOf(beat, -60, 0) * 100)}%`} title={`Beat ${place + 1}: ${decibels(beat)}`}></span>
        {/each}
      </div>
    {/if}
    {#each card.silences as silence, place (place)}
      <p class="agent-quiet agent-card-note">{silentBeats(silence)}</p>
    {/each}
  {:else if card.kind === "practice"}
    <p class="agent-card-note">{card.view.piece.name}, {card.view.piece.bars} bars</p>
    <div class="agent-practice-tiles">
      <span><span class="agent-quiet">Mode</span> {card.view.settings.mode}</span>
      <span><span class="agent-quiet">Hands</span> {card.view.settings.hands}</span>
      <span><span class="agent-quiet">Level</span> {card.view.settings.level}, {card.view.window_ms} ms</span>
      <span><span class="agent-quiet">Tempo</span> {card.view.settings.tempo}</span>
    </div>
    <p class="agent-quiet agent-card-note">On the Play tab</p>
  {/if}
</div>
