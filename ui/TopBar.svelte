<!-- The page's top bar, built like GitHub's repository header: a title row
     with where you are on the left and actions on the right, then a row of
     tabs. It starts sparse: one tab and two actions. -->
<script lang="ts">
  interface Props {
    // The wave new notes play, like "saw", or "" until Rust has said.
    wave: string;
    onWaveClick: () => void;
    onRenderClick: () => void;
  }

  let { wave, onWaveClick, onRenderClick }: Props = $props();

  // Runs the button's action, then gives up the focus, so Space or Enter
  // played later can never press it again.
  function clickThenLetGo(event: MouseEvent, action: () => void) {
    action();
    const button = event.currentTarget as HTMLButtonElement;
    button.blur();
  }
</script>

<header class="topbar">
  <div class="topbar-row">
    <div class="crumbs">
      <span class="logo"></span>
      <span class="crumb">Jello</span>
      <span class="crumb-divider">/</span>
      <span class="crumb is-current">Keyboard</span>
    </div>

    <div class="actions">
      <button
        type="button"
        class="action"
        title="Next wave (the ` key)"
        onclick={(event) => clickThenLetGo(event, onWaveClick)}
      >
        <span class="action-label">Wave</span>
        <span class="action-value">{wave}</span>
      </button>
      <button
        type="button"
        class="action"
        title="Save a held middle C as a WAV file"
        onclick={(event) => clickThenLetGo(event, onRenderClick)}
      >
        <span class="action-value">Render WAV</span>
      </button>
    </div>
  </div>

  <nav class="tabs">
    <span class="tab is-active">
      <!-- A small piano, drawn in the text's color. -->
      <svg class="tab-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.5" />
        <path d="M6 9v4.25M10 9v4.25" fill="none" stroke="currentColor" stroke-width="1.5" />
        <path d="M4.75 2.75h2.5V9h-2.5zM8.75 2.75h2.5V9h-2.5z" fill="currentColor" />
      </svg>
      <span class="tab-label">Play</span>
    </span>
  </nav>
</header>
