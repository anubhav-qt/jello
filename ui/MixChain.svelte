<!-- The picked strip's chain (docs/design.md#mix): its 8 slots, top to
     bottom, then Out, where the sound leaves. A click opens a slot or Out on
     the right, the first empty slot offers "+ Add effect", the dot at the
     right switches an effect on or off, and dragging a slot moves it. -->
<script lang="ts">
  import { clickThenLetGo } from "./dom";
  import { kindName, type Chain } from "./mix";

  interface Props {
    chain: Chain;
    // The slot open on the right, from 1, or "out" or "add".
    open: number | "out" | "add";
    // The slot "+ Add effect" was clicked for, while the list is open.
    adding: number | null;
    // How far each slot turns the sound down now, in dB.
    reductions: number[];
    // What Out reads, like "22 · L24 · 2 sends".
    outWords: string;
    onOpen: (slot: number | "out") => void;
    onAdd: (slot: number) => void;
    onSwitch: (slot: number, on: boolean) => void;
    onMove: (from: number, to: number) => void;
  }

  let { chain, open, adding, reductions, outWords, onOpen, onAdd, onSwitch, onMove }: Props = $props();

  // The first empty slot, from 1, which says "+ Add effect", or null when
  // every slot is full.
  let firstEmpty = $derived.by(() => {
    for (let slot = 1; slot <= chain.length; slot += 1) {
      if (chain[slot - 1] === null) {
        return slot;
      }
    }
    return null;
  });

  // How far it turns down, written like "-4.2", or "" under 0.1 dB.
  function reductionWords(slot: number): string {
    const reduction = reductions[slot - 1] ?? 0;
    if (reduction < 0.1) {
      return "";
    }
    return `-${reduction.toFixed(1)}`;
  }

  // A press on a slot: a click opens it; moving 4px or more drags it
  // over the others, and letting go moves it where the pointer is.
  const DRAG_DISTANCE = 4;
  let rows: HTMLElement;
  let pressed: { slot: number; y: number } | null = null;
  let dragging = $state(false);
  let from: number | null = $state(null);
  let over: number | null = $state(null);

  function slotAt(y: number): number {
    const slots = rows.querySelectorAll(".mix-slot");
    let nearest = 1;
    for (let place = 0; place < slots.length; place += 1) {
      const box = slots[place].getBoundingClientRect();
      if (y >= box.top) {
        nearest = place + 1;
      }
    }
    return nearest;
  }

  function press(event: PointerEvent, slot: number) {
    if (event.button !== 0) {
      return;
    }
    pressed = { slot, y: event.clientY };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function move(event: PointerEvent) {
    if (pressed === null) {
      return;
    }
    if (!dragging) {
      if (Math.abs(event.clientY - pressed.y) < DRAG_DISTANCE) {
        return;
      }
      if (chain[pressed.slot - 1] === null) {
        return;
      }
      dragging = true;
      from = pressed.slot;
    }
    over = slotAt(event.clientY);
  }

  function release() {
    if (pressed === null) {
      return;
    }
    const slot = pressed.slot;
    if (dragging && from !== null && over !== null && over !== from) {
      onMove(from, over);
    } else if (!dragging) {
      clickSlot(slot);
    }
    pressed = null;
    dragging = false;
    from = null;
    over = null;
  }

  function cancel() {
    pressed = null;
    dragging = false;
    from = null;
    over = null;
  }

  function clickSlot(slot: number) {
    if (chain[slot - 1] === null) {
      onAdd(slot);
      return;
    }
    onOpen(slot);
  }

  function switchOn(event: MouseEvent, slot: number) {
    event.stopPropagation();
    const effect = chain[slot - 1];
    if (effect === null) {
      return;
    }
    clickThenLetGo(event, () => onSwitch(slot, !effect.on));
  }

  function isOpen(slot: number): boolean {
    if (open === "add") {
      return adding === slot;
    }
    return open === slot;
  }
</script>

<div class="mix-chain" class:is-dragging={dragging} bind:this={rows}>
  {#each chain as effect, place (place)}
    {@const slot = place + 1}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="mix-slot"
      class:is-empty={effect === null}
      class:is-open={isOpen(slot)}
      class:is-off={effect !== null && !effect.on}
      class:is-from={from === slot}
      class:is-over={dragging && over === slot && over !== from}
      onpointerdown={(event) => press(event, slot)}
      onpointermove={move}
      onpointerup={release}
      onpointercancel={cancel}
    >
      <span class="mix-slot-number">{slot}</span>
      {#if effect !== null}
        <span class="mix-slot-name">{kindName(effect.kind)}</span>
        <span class="mix-slot-reduction">{reductionWords(slot)}</span>
        <button
          type="button"
          class="mix-on"
          class:is-on={effect.on}
          title={effect.on ? "On: click to switch it off" : "Off: click to switch it on"}
          onpointerdown={(event) => event.stopPropagation()}
          onclick={(event) => switchOn(event, slot)}
        ></button>
      {:else if slot === firstEmpty || isOpen(slot)}
        <span class="mix-slot-add">+ Add effect</span>
      {/if}
    </div>
  {/each}
  <button
    type="button"
    class="mix-slot mix-out"
    class:is-open={open === "out"}
    onclick={(event) => clickThenLetGo(event, () => onOpen("out"))}
  >
    <span class="mix-slot-number">→</span>
    <span class="mix-slot-name">Out</span>
    <span class="mix-out-words">{outWords}</span>
  </button>
</div>
