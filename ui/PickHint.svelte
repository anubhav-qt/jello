<!-- What a picked value's arrow keys do, at the top right of the sound
     and Mix panels: its name, the modifier, then left and right for a
     step and up and down for ten. -->
<script lang="ts">
  interface Props {
    name: string;
    adjustModifier: string;
  }

  let { name, adjustModifier }: Props = $props();
</script>

<span class="sound-pick-hint">
  <span class="sound-picked-name">{name}</span>
  <span class="sound-shortcut">{adjustModifier} +</span>
  <i class="sound-arrow"
    ><svg viewBox="0 0 10 10"><path d="M8 5 H2 M4.5 2.5 L2 5 L4.5 7.5" /></svg></i
  >
  <i class="sound-arrow"
    ><svg viewBox="0 0 10 10"><path d="M2 5 H8 M5.5 2.5 L8 5 L5.5 7.5" /></svg></i
  >
  <span class="sound-arrow-words">step</span>
  <i class="sound-arrow-gap"></i>
  <i class="sound-arrow"
    ><svg viewBox="0 0 10 10"><path d="M5 8 V2 M2.5 4.5 L5 2 L7.5 4.5" /></svg></i
  >
  <i class="sound-arrow"
    ><svg viewBox="0 0 10 10"><path d="M5 2 V8 M2.5 5.5 L5 8 L7.5 5.5" /></svg></i
  >
  <span class="sound-arrow-words">ten steps</span>
</span>
