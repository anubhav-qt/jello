<!-- The page's top bar, built like GitHub's repository header: a title row
     with the open song's name on the left, which opens the Songs panel
     (docs/adr/0038-song-files.md), and actions on the right, then a row of
     tabs: Play, Compose and Produce. Messages for the player are typed out
     after the title, wrapping onto a second line when long. The Sound and
     Render WAV buttons sit on the right of the title row, one width, and the
     agent's button at the right end of the tabs, named for its job in the
     open tab: Tutor, Composer or Producer, the same width in every tab
     (docs/adr/0041-one-agent-for-every-mode.md). -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { clickThenLetGo } from "./dom";
  import type { Tab } from "./tabs";

  interface Props {
    // The tab open now.
    tab: Tab;
    // The open song's name, or "" until Rust has said, whether the Songs
    // panel is open, and what clicking the name does.
    songName: string;
    songsOpen: boolean;
    onSongClick: () => void;
    onTabClick: (tab: Tab) => void;
    onRenderClick: () => void;
    // The message typed out so far, and whether it is shown.
    message: string;
    messageShown: boolean;
    // The Sound button, drawn by Stage.
    endButtons: Snippet;
    // Whether the agent's panel is open, and what its button does.
    agentOpen: boolean;
    onAgentClick: () => void;
  }

  let {
    tab,
    songName,
    songsOpen,
    onSongClick,
    onTabClick,
    onRenderClick,
    message,
    messageShown,
    endButtons,
    agentOpen,
    onAgentClick,
  }: Props = $props();

  // The agent's name in each tab.
  let agentName = $derived.by(() => {
    if (tab === "compose") {
      return "Composer";
    }
    if (tab === "produce") {
      return "Producer";
    }
    return "Tutor";
  });
</script>

<header class="topbar">
  <div class="topbar-row">
    <div class="crumbs">
      <span class="logo"></span>
      <span class="crumb">Jello</span>
      <span class="crumb-divider">/</span>
      <button
        type="button"
        class="crumb is-current crumb-song"
        class:is-open={songsOpen}
        title="Songs: open, make or rename one"
        onclick={(event) => clickThenLetGo(event, onSongClick)}
      >
        {songName}
      </button>
    </div>

    <!-- The text and cursor sit on one line, so no space comes between them. -->
    <p class="topbar-message" class:is-shown={messageShown}>
      <span class="topbar-message-text">{message}</span><span
        class="topbar-message-cursor"
      ></span>
    </p>

    <div class="actions">
      {@render endButtons()}
      <button
        type="button"
        class="action top-button"
        title="Save a held middle C as a WAV file"
        onclick={(event) => clickThenLetGo(event, onRenderClick)}
      >
        <span class="action-name">Render WAV</span>
      </button>
    </div>
  </div>

  <nav class="tabs">
    <button
      type="button"
      class="tab"
      class:is-active={tab === "play"}
      onclick={(event) => clickThenLetGo(event, () => onTabClick("play"))}
    >
      <!-- A small piano, drawn in the text's color. -->
      <svg
        class="tab-icon"
        viewBox="0 0 16 16"
        width="16"
        height="16"
        aria-hidden="true"
      >
        <rect
          x="1.75"
          y="2.75"
          width="12.5"
          height="10.5"
          rx="1.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        />
        <path
          d="M6 9v4.25M10 9v4.25"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        />
        <path
          d="M4.75 2.75h2.5V9h-2.5zM8.75 2.75h2.5V9h-2.5z"
          fill="currentColor"
        />
      </svg>
      <span class="tab-label">Play</span>
    </button>

    <button
      type="button"
      class="tab"
      class:is-active={tab === "compose"}
      onclick={(event) => clickThenLetGo(event, () => onTabClick("compose"))}
    >
      <!-- A pencil. -->
      <svg
        class="tab-icon"
        viewBox="0 0 16 16"
        width="16"
        height="16"
        aria-hidden="true"
      >
        <path
          d="M10.5 2.25l3.25 3.25-8 8H2.5v-3.25z"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linejoin="round"
        />
        <path
          d="M8.75 4l3.25 3.25"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        />
      </svg>
      <span class="tab-label">Compose</span>
    </button>

    <button
      type="button"
      class="tab"
      class:is-active={tab === "produce"}
      onclick={(event) => clickThenLetGo(event, () => onTabClick("produce"))}
    >
      <!-- Three mixer faders. -->
      <svg
        class="tab-icon"
        viewBox="0 0 16 16"
        width="16"
        height="16"
        aria-hidden="true"
      >
        <path
          d="M3.5 1.75v12.5M8 1.75v12.5M12.5 1.75v12.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        />
        <path
          d="M1.75 9h3.5v2.5h-3.5zM6.25 4h3.5v2.5h-3.5zM10.75 7h3.5v2.5h-3.5z"
          fill="currentColor"
        />
      </svg>
      <span class="tab-label">Produce</span>
    </button>

    <div class="tabs-end">
      <button
        type="button"
        class="action agent-button"
        class:is-open={agentOpen}
        title="The agent's panel"
        onclick={(event) => clickThenLetGo(event, onAgentClick)}
      >
        <span class="action-name">{agentName}</span>
      </button>
    </div>
  </nav>
</header>
