<!-- How far a compressor, a gate or the limiter turns the sound down now:
     a column of bars lit down from the top, and the number under it. -->
<script lang="ts">
  interface Props {
    // In dB, 0 for not at all.
    db: number;
    // How many dB the whole column reads.
    most?: number;
  }

  let { db, most = 24 }: Props = $props();

  const BARS = 14;

  let lit = $derived(Math.min(BARS, Math.round((db / most) * BARS)));

  let words = $derived.by(() => {
    if (db < 0.05) {
      return "0";
    }
    return `-${db.toFixed(1)}`;
  });
</script>

<div class="mix-reduction" title="How far it turns the sound down now">
  <div class="mix-reduction-bars">
    {#each { length: BARS } as _, bar}
      <i class:is-lit={bar < lit}></i>
    {/each}
  </div>
  <span class="mix-reduction-number">{words}</span>
  <span class="mix-reduction-unit">dB</span>
</div>
