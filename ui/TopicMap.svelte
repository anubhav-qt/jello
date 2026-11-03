<!-- The topic map, in the tape's place while open
     (docs/design.md#ai-and-learning-rethought-2026-11-03): every topic of
     Theory, Composition, Lyrics and Finishing, white once covered and
     lavender for the one being learned now. Clicking the stage strip or
     Esc closes it. -->
<script lang="ts">
  import { coveredCount, type PathView } from "./path";

  interface Props {
    path: PathView;
  }
  let { path }: Props = $props();

  let counts = $derived(coveredCount(path));
</script>

<div class="topic-map">
  <p class="topic-map-head">
    <span class="tape-label">Topics</span>
    <span class="topic-map-count">{counts.covered} of {counts.all} covered</span>
    <span class="topic-map-hint">Esc closes</span>
  </p>
  <div class="topic-areas">
    {#each path.areas as area (area.name)}
      <div class="topic-area">
        <p class="tape-label">{area.name}</p>
        <div class="topic-list">
          {#each area.topics as topic (topic.name)}
            <span
              class="topic"
              class:is-done={topic.state === "done"}
              class:is-current={topic.state === "current"}>{topic.name}</span
            >
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>
