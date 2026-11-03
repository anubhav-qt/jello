<!-- The volume shaper's page (docs/design.md#mix): its volume over two
     cycles, the shape faint and the volume heard, after the depth, lit.
     The first cycle's points are squares to drag; a click in the drawing
     adds a point (up to 16) and a double click on one removes it. Then its
     cycle, depth and the picked point's bend. -->
<script lang="ts">
  import {
    beatScale,
    effectSetting,
    linearScale,
    numberOf,
    percentScale,
    SHAPER_MOST_POINTS,
    type MixSender,
    type SlotView,
  } from "./mix";
  import { EDGE, HEIGHT, pathOf, valueOfX, valueOfY, WIDTH, xOf, yOf } from "./mix-drawing";
  import { isPicked, pickedOf, type Picked, type Setting } from "./sound-scales";
  import SoundHandle from "./SoundHandle.svelte";
  import SoundValue from "./SoundValue.svelte";

  interface Props {
    place: string;
    slot: number;
    effect: SlotView;
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    send: MixSender;
  }

  let { place, slot, effect, picked, onPick, send }: Props = $props();

  // How far a bend of 1 bends a segment, as the engine bends it
  // (engine/src/volume_shaper.cpp).
  const MOST_BEND_POWER = 8;
  const CYCLES = 2;

  interface Point {
    place: number;
    level: number;
    bend: number;
  }

  // The point picked, from 1, kept among those in use.
  let chosen = $state(1);

  function value(word: string): number {
    return numberOf(effect.values[word]);
  }

  let count = $derived(Math.round(value("points")));
  let point = $derived(Math.max(1, Math.min(chosen, count)));

  // The points in use, places and levels from 0 to 1.
  let points = $derived.by(() => {
    const all: Point[] = [];
    for (let at = 1; at <= count; at += 1) {
      all.push({
        place: value(`point_${at}_place`) / 100,
        level: value(`point_${at}_level`) / 100,
        bend: value(`point_${at}_bend`),
      });
    }
    return all;
  });

  function bent(share: number, bend: number): number {
    if (bend === 0) {
      return share;
    }
    return Math.pow(share, Math.pow(MOST_BEND_POWER, -bend));
  }

  // The shape's level at `phase`, from 0 to 1 in a cycle: from the last
  // point at or before it towards the next, round to the first.
  function levelAt(phase: number): number {
    const last = points.length - 1;
    let from = last;
    for (let at = 0; at < points.length; at += 1) {
      if (points[at].place <= phase) {
        from = at;
      }
    }
    let start = points[from].place;
    let end = 0;
    let to = from + 1;
    if (from === last) {
      to = 0;
      end = points[0].place + 1;
      if (phase < points[0].place) {
        start -= 1;
      }
    } else {
      end = points[to].place;
    }
    let share = 0;
    if (end > start) {
      share = (phase - start) / (end - start);
    }
    const fromLevel = points[from].level;
    const toLevel = points[to].level;
    return fromLevel + (toLevel - fromLevel) * bent(Math.max(0, Math.min(1, share)), points[from].bend);
  }

  function lineOf(depth: number): string {
    const line: [number, number][] = [];
    const steps = 200;
    for (let step = 0; step <= steps; step += 1) {
      const along = (step / steps) * CYCLES;
      const phase = along - Math.floor(along);
      let level = levelAt(phase);
      if (step === steps) {
        level = levelAt(0.99999);
      }
      const heard = 1 - depth * (1 - level);
      line.push([xOf(along, 0, CYCLES), yOf(heard, 0, 1)]);
    }
    return pathOf(line);
  }

  let shape = $derived(lineOf(1));
  let heard = $derived(lineOf(value("depth") / 100));

  let cycle = $derived(
    effectSetting(place, slot, "cycle_beats", "Cycle", beatScale(16), () => value("cycle_beats"), send),
  );
  let depth = $derived(effectSetting(place, slot, "depth", "Depth", percentScale(100), () => value("depth"), send));
  let bend: Setting = $derived(
    effectSetting(
      place,
      slot,
      `point_${point}_bend`,
      `Bend ${point}`,
      linearScale(-1, 1, 0.05, "", true),
      () => value(`point_${point}_bend`),
      send,
    ),
  );
  let placeOfPoint: Setting = $derived(
    effectSetting(
      place,
      slot,
      `point_${point}_place`,
      `Place ${point}`,
      percentScale(100),
      () => value(`point_${point}_place`),
      send,
    ),
  );

  function grabPoint(at: number) {
    chosen = at;
    onPick(pickedOf(placeOfPoint));
  }

  // Dragging a point moves its place, between its neighbours', and its
  // level.
  function movePoint(at: number, x: number, y: number) {
    let lowest = 0;
    let highest = 1;
    if (at > 1) {
      lowest = points[at - 2].place;
    }
    if (at < count) {
      highest = points[at].place;
    }
    const along = Math.max(lowest, Math.min(highest, valueOfX(x, 0, CYCLES)));
    const level = Math.max(0, Math.min(1, valueOfY(y, 0, 1)));
    send.setEffectValue(place, slot, `point_${at}_place`, Math.round(along * 100));
    send.setEffectValue(place, slot, `point_${at}_level`, Math.round(level * 100));
  }

  function sendPoint(at: number, values: Point) {
    send.setEffectValue(place, slot, `point_${at}_place`, Math.round(values.place * 100));
    send.setEffectValue(place, slot, `point_${at}_level`, Math.round(values.level * 100));
    send.setEffectValue(place, slot, `point_${at}_bend`, values.bend);
  }

  // A click in the drawing adds a point where it is, in place order: the
  // points after it move up one.
  function addPoint(event: MouseEvent) {
    if (count >= SHAPER_MOST_POINTS) {
      return;
    }
    const drawing = (event.currentTarget as SVGElement).ownerSVGElement;
    const toPage = drawing?.getScreenCTM();
    if (drawing === null || toPage === null || toPage === undefined) {
      return;
    }
    const inDrawing = new DOMPoint(event.clientX, event.clientY).matrixTransform(toPage.inverse());
    const along = valueOfX(inDrawing.x, 0, CYCLES);
    const phase = along - Math.floor(along);
    const level = Math.max(0, Math.min(1, valueOfY(inDrawing.y, 0, 1)));
    let at = 0;
    while (at < count && points[at].place <= phase) {
      at += 1;
    }
    for (let moved = count; moved > at; moved -= 1) {
      sendPoint(moved + 1, points[moved - 1]);
    }
    sendPoint(at + 1, { place: phase, level, bend: 0 });
    send.setEffectValue(place, slot, "points", count + 1);
    chosen = at + 1;
  }

  // A double click on a point removes it, unless it is the last one: the
  // points after it move down one.
  function removePoint(at: number) {
    if (count <= 1) {
      return;
    }
    for (let moved = at; moved < count; moved += 1) {
      sendPoint(moved, points[moved]);
    }
    send.setEffectValue(place, slot, "points", count - 1);
    chosen = Math.max(1, at - 1);
  }

  let beatMarks = $derived.by(() => {
    const marks: { x: number; words: string }[] = [];
    for (let at = 0; at < CYCLES; at += 1) {
      marks.push({ x: xOf(at, 0, CYCLES), words: `cycle ${at + 1}` });
    }
    return marks;
  });
</script>

<div class="mix-effect">
  <div class="mix-drawing-column">
    <div class="sound-drawing is-fixed mix-drawing">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <rect class="mix-add-area" x="0" y="0" width={WIDTH} height={HEIGHT} onclick={addPoint} />
        {#each beatMarks as mark (mark.x)}
          <path class="sound-lit-soft is-faint" d={pathOf([[mark.x, EDGE], [mark.x, HEIGHT - EDGE]])} />
          <text class="sound-axis" x={mark.x + 4} y={EDGE + 8}>{mark.words}</text>
        {/each}
        <path class="sound-lit-soft" d={shape} />
        <path class="sound-lit is-fine" d={heard} />
        {#each points as each, at (at)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <g ondblclick={() => removePoint(at + 1)}>
            <SoundHandle
              x={xOf(each.place, 0, CYCLES)}
              y={yOf(each.level, 0, 1)}
              solid={point === at + 1}
              ringed={point === at + 1 && (isPicked(picked, placeOfPoint.id) || isPicked(picked, bend.id))}
              onGrab={() => grabPoint(at + 1)}
              onMove={(x, y) => movePoint(at + 1, x, y)}
            />
          </g>
        {/each}
      </svg>
    </div>
    <p class="mix-note">Click to add a point · double-click one to remove it</p>
  </div>
  <div class="mix-fields">
    <SoundValue setting={cycle} dots={9} {picked} {onPick} />
    <SoundValue setting={depth} dots={9} {picked} {onPick} />
    <SoundValue setting={placeOfPoint} dots={9} {picked} {onPick} />
    <SoundValue setting={bend} dots={9} {picked} {onPick} />
    <div class="mix-readout">
      <div class="sound-value-name">Points</div>
      <div class="sound-value-number">{count} <small>of {SHAPER_MOST_POINTS}</small></div>
    </div>
  </div>
</div>
