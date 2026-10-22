<!-- One value of the sound panel (docs/design.md#sound-panel): its name, its
     number and unit, and a row of dots lit up to it. Dragging along the
     dots sets it, and clicking a dot jumps there. Clicking anywhere on it
     picks it for the arrow keys, framing it in lavender. A connection's
     amount has a middle zero, and its dots light either way from it. -->
<script lang="ts">
  import { isPicked, litDots, litSideDots, pickedOf, type Picked, type Setting } from "./sound-scales";

  interface Props {
    setting: Setting;
    // How many dots its row has.
    dots: number;
    // The value picked now, if any, and what picks this one.
    picked: Picked | null;
    onPick: (picked: Picked) => void;
    // Dimmed when it would do nothing, like spread on a shared LFO.
    dim?: boolean;
    // A connection's amount, with dots either way from a middle zero.
    bipolar?: boolean;
    // The tuning, drawn in big digits with no name over it.
    big?: boolean;
  }

  let { setting, dots, picked, onPick, dim = false, bipolar = false, big = false }: Props = $props();

  // The value now, as Rust answered last. It changes with every answer.
  let value = $derived(setting.now());
  let share = $derived(setting.scale.share(value));
  let words = $derived(setting.scale.words(value));
  let unit = $derived(setting.scale.unit(value));

  // Where the middle zero is on a connection's row, and how many dots each
  // side of it has.
  let side = $derived((dots - 1) / 2);

  // `share` kept within 0 and 1, or -1 and 1 for a connection's amount.
  function withinRow(newShare: number): number {
    let lowest = 0;
    if (bipolar) {
      lowest = -1;
    }
    return Math.max(lowest, Math.min(1, newShare));
  }

  // Whether each dot is lit: from the first, or out from a connection's
  // middle zero, on the side the amount is.
  let lit = $derived.by(() => {
    const dotsLit: boolean[] = [];
    if (!bipolar) {
      const count = litDots(withinRow(share), dots);
      for (let place = 0; place < dots; place += 1) {
        dotsLit.push(place < count);
      }
      return dotsLit;
    }
    const count = litSideDots(withinRow(share), side);
    for (let place = 0; place < dots; place += 1) {
      let out = place - side;
      if (share < 0) {
        out = side - place;
      }
      dotsLit.push(out > 0 && out <= count);
    }
    return dotsLit;
  });

  // A connection's middle zero is lit dimly, so its row shows where 0 is.
  function isZero(place: number): boolean {
    if (!bipolar) {
      return false;
    }
    return place === side;
  }

  let row: HTMLElement;
  // True from pressing on the dots until letting go.
  let dragging = false;
  // The value sent last while dragging, so the same one is not sent again.
  let sent: number | null = null;

  function pick() {
    onPick(pickedOf(setting));
  }

  // The share of the row under the pointer, from the first dot's middle to
  // the last's: 0 to 1, or -1 to 1 from a connection's middle zero.
  function shareAt(x: number): number {
    const first = row.children[0].getBoundingClientRect();
    const last = row.children[dots - 1].getBoundingClientRect();
    const firstMiddle = first.left + first.width / 2;
    const lastMiddle = last.left + last.width / 2;
    const along = (x - firstMiddle) / (lastMiddle - firstMiddle);
    if (!bipolar) {
      return withinRow(along);
    }
    return withinRow(along * 2 - 1);
  }

  // A click jumps to the nearest dot.
  function dotShareAt(x: number): number {
    const along = shareAt(x);
    if (!bipolar) {
      return Math.round(along * (dots - 1)) / (dots - 1);
    }
    return Math.round(along * side) / side;
  }

  function send(newShare: number) {
    const newValue = setting.scale.at(newShare);
    if (newValue === sent) {
      return;
    }
    sent = newValue;
    setting.set(newValue);
  }

  function startDrag(event: PointerEvent) {
    dragging = true;
    sent = null;
    row.setPointerCapture(event.pointerId);
    send(dotShareAt(event.clientX));
  }

  function drag(event: PointerEvent) {
    if (!dragging) {
      return;
    }
    send(shareAt(event.clientX));
  }

  function stopDrag() {
    dragging = false;
  }
</script>

<!-- The panel is played with the pointer and the arrow keys, which main.ts
     sends to the picked value, so its parts take no focus of their own. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="sound-value"
  class:is-picked={isPicked(picked, setting.id)}
  class:is-dim={dim}
  class:is-big={big}
  onpointerdown={pick}
>
  {#if !big}
    <div class="sound-value-name">{setting.name}</div>
  {/if}
  <div class="sound-value-number">
    {words}{#if unit !== ""}{" "}<small>{unit}</small>{/if}
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="sound-dots"
    bind:this={row}
    onpointerdown={startDrag}
    onpointermove={drag}
    onpointerup={stopDrag}
    onpointercancel={stopDrag}
  >
    {#each lit as on, place}
      <i class:is-lit={on} class:is-zero={isZero(place)}></i>
    {/each}
  </div>
</div>
