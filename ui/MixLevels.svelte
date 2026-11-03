<!-- The Mix panel's levels (docs/design.md#mix): every track, bus and the
     master as a thin strip, with its meter, its volume as a column of dots
     to drag, the volume's number and its pan. A click on a strip picks it. -->
<script lang="ts">
  import type { Strip } from "./mix";
  import { litDots } from "./sound-scales";

  interface Props {
    strips: Strip[];
    picked: string;
    onPick: (place: string) => void;
    onVolume: (place: string, volume: number) => void;
  }

  let { strips, picked, onPick, onVolume }: Props = $props();

  // The meter reads from LOWEST_DB to 0 dB in METER_BARS bars.
  const METER_BARS = 16;
  const LOWEST_DB = -48;
  const FADER_DOTS = 16;
  const PAN_TICKS = 9;

  function meterLit(db: number): number {
    const share = (db - LOWEST_DB) / -LOWEST_DB;
    if (share <= 0) {
      return 0;
    }
    return Math.min(METER_BARS, Math.ceil(share * METER_BARS));
  }

  function faderLit(volume: number): number {
    return litDots(volume / 100, FADER_DOTS);
  }

  // Which of the pan's ticks is lit: the middle one for centre.
  function panTick(pan: number): number {
    const middle = (PAN_TICKS - 1) / 2;
    return Math.round(middle + (pan / 100) * middle);
  }

  // Dragging a fader sets its strip's volume from where the pointer is
  // along its dots, top for 100%.
  let dragged: { place: string; fader: HTMLElement } | null = null;
  let sent: number | null = null;

  function volumeAt(fader: HTMLElement, y: number): number {
    const box = fader.getBoundingClientRect();
    const share = (box.bottom - y) / box.height;
    const within = Math.max(0, Math.min(1, share));
    return Math.round(within * 100);
  }

  function sendVolume(y: number) {
    if (dragged === null) {
      return;
    }
    const volume = volumeAt(dragged.fader, y);
    if (volume === sent) {
      return;
    }
    sent = volume;
    onVolume(dragged.place, volume);
  }

  function grabFader(event: PointerEvent, place: string) {
    event.stopPropagation();
    const fader = event.currentTarget as HTMLElement;
    fader.setPointerCapture(event.pointerId);
    dragged = { place, fader };
    sent = null;
    onPick(place);
    sendVolume(event.clientY);
  }

  function dragFader(event: PointerEvent) {
    sendVolume(event.clientY);
  }

  function letGoFader() {
    dragged = null;
  }
</script>

<div class="mix-strips">
  {#each strips as strip (strip.place)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="mix-strip"
      class:is-picked={strip.place === picked}
      class:has-gap={strip.gap}
      title={strip.title}
      onclick={() => onPick(strip.place)}
    >
      <span class="mix-strip-name">{strip.label}</span>
      <div class="mix-strip-columns">
        <div class="mix-meter">
          {#each { length: METER_BARS } as _, bar}
            <i class:is-lit={bar < meterLit(strip.db)} class:is-hot={bar === METER_BARS - 1 && bar < meterLit(strip.db)}></i>
          {/each}
        </div>
        {#if strip.volume !== null}
          <div
            class="mix-fader"
            onpointerdown={(event) => grabFader(event, strip.place)}
            onpointermove={dragFader}
            onpointerup={letGoFader}
            onpointercancel={letGoFader}
          >
            {#each { length: FADER_DOTS } as _, dot}
              <i class:is-lit={dot < faderLit(strip.volume)}></i>
            {/each}
          </div>
        {/if}
      </div>
      <span class="mix-strip-volume">
        {#if strip.volume !== null}{strip.volume}{:else}&nbsp;{/if}
      </span>
      <span class="mix-strip-pan">
        {#if strip.pan !== null}
          {#each { length: PAN_TICKS } as _, tick}
            <i class:is-lit={tick === panTick(strip.pan ?? 0)}></i>
          {/each}
        {/if}
      </span>
    </div>
  {/each}
</div>
