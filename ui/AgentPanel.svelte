<!-- The agent's panel, docked on the right in every tab
     (docs/design.md#the-agent-mode): Tutor in Play, Composer in Compose,
     Producer in Produce, all one chat. Each run draws its steps live as
     rows: thinking, every tool with what it changed, the text as it comes.
     Stop, or Escape, ends a run. Before a model is signed in to, the panel
     shows where to sign in. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { tick } from "svelte";
  import AgentCard from "./AgentCard.svelte";
  import AgentOptions from "./AgentOptions.svelte";
  import Markdown from "./Markdown.svelte";
  import {
    addToTurn,
    cachedPercent,
    changedSomething,
    chatsByDay,
    chatsMatching,
    hostEndedWhy,
    jobLetter,
    jobName,
    newTurn,
    replayedTurn,
    thoughtSeconds,
    toolDetail,
    toolTitle,
    type Card,
    type ChatView,
    type HostEvent,
    type Job,
    type ModelView,
    type ProviderView,
    type SavedTurn,
    type Turn,
  } from "./agent";

  interface Props {
    visible: boolean;
    job: Job;
    // The song may have changed, by Undo: redraw the sound, Compose and
    // the songs.
    onSongChanged: () => void;
    // The player sent a message, so the agent's marks on the keys go.
    onSent: () => void;
  }
  let { visible, job, onSongChanged, onSent }: Props = $props();

  interface Status {
    models: ModelView[];
    model: ModelView | null;
    providers: ProviderView[];
  }
  // What signing in to one provider asks and says, as it goes.
  interface SignIn {
    provider: string;
    prompt: {
      kind: string;
      message: string;
      placeholder: string;
      options: { id: string; label: string }[];
    } | null;
    info: { message: string; url: string; code: string } | null;
    failed: string | null;
  }

  // How many providers show before "More providers".
  const FIRST_PROVIDERS_SHOWN = 2;

  // The models and providers, or null until the host has said.
  let status: Status | null = $state(null);
  let turns: Turn[] = $state([]);
  let draft = $state("");
  // A problem not part of any run, like the host failing to start.
  let problem = $state("");
  let started = $state(false);
  let modelsOpen = $state(false);
  let signInOpen = $state(false);
  let allProviders = $state(false);
  let signIn: SignIn | null = $state(null);
  let signInAnswer = $state("");
  // The Chats page: open or not, the chats once the host has listed them,
  // the search, and the chat open now, if it is saved yet.
  let chatsOpen = $state(false);
  let chats: ChatView[] | null = $state(null);
  let chatSearch = $state("");
  let openChatPath: string | null = $state(null);
  // The thinking rows opened to their text, by turn and step.
  let openThoughts: string[] = $state([]);
  // The job the chat was told last.
  let lastJob: Job | null = null;
  // The clock thinking rows count by, while a run goes.
  let now = $state(performance.now());
  let list: HTMLDivElement | undefined = $state();
  // The phrase jello_play is playing: when it started, by performance.now(),
  // and how long it lasts, so the newest lane card can follow it.
  let phrase: { startedAt: number; lengthMs: number } | null = $state(null);
  let phraseNow = $state(0);

  let working = $derived.by(() => {
    const turn = turns[turns.length - 1];
    if (turn === undefined) {
      return false;
    }
    return turn.running;
  });

  let needsSignIn = $derived.by(() => {
    if (status === null) {
      return false;
    }
    if (signInOpen) {
      return true;
    }
    return status.model === null;
  });

  let chatGroups = $derived.by(() => {
    if (chats === null) {
      return [];
    }
    return chatsByDay(chatsMatching(chats, chatSearch), Date.now());
  });

  let providersShown = $derived.by(() => {
    if (status === null) {
      return [];
    }
    if (allProviders) {
      return status.providers;
    }
    return status.providers.slice(0, FIRST_PROVIDERS_SHOWN);
  });

  // The panel opening the first time starts the agent host.
  $effect(() => {
    if (!visible || started) {
      return;
    }
    started = true;
    invoke("agent_status").catch((error) => {
      problem = String(error);
      started = false;
    });
  });

  $effect(() => {
    const unlistenHost = listen<HostEvent>("agent-host", (event) => {
      hear(event.payload);
    });
    const unlistenPhrase = listen<{ length_ms: number }>("agent-phrase", (event) => {
      phrase = { startedAt: performance.now(), lengthMs: event.payload.length_ms };
      phraseNow = phrase.startedAt;
    });
    const unlistenOption = listen<{ why: string }>("agent-option-failed", (event) => {
      problem = event.payload.why;
    });
    const unlistenAsks = listen<{ number: number; question: string }>(
      "agent-asks",
      (event) => {
        const turn = turns[turns.length - 1];
        if (turn === undefined) {
          return;
        }
        turn.questions.push({ ...event.payload, answer: null });
        showLatest();
      },
    );
    return () => {
      unlistenHost.then((unlisten) => unlisten());
      unlistenAsks.then((unlisten) => unlisten());
      unlistenPhrase.then((unlisten) => unlisten());
      unlistenOption.then((unlisten) => unlisten());
    };
  });

  // While a phrase plays, the newest lane card's playhead moves.
  $effect(() => {
    if (phrase === null) {
      return;
    }
    const playing = phrase;
    let frame = 0;
    const move = () => {
      phraseNow = performance.now();
      if (phraseNow - playing.startedAt > playing.lengthMs) {
        phrase = null;
        return;
      }
      frame = requestAnimationFrame(move);
    };
    frame = requestAnimationFrame(move);
    return () => cancelAnimationFrame(frame);
  });

  // The newest lane card, by turn and step, which the playhead follows.
  let liveLane = $derived.by(() => {
    const turnPlace = turns.length - 1;
    const turn = turns[turnPlace];
    if (turn === undefined) {
      return "";
    }
    for (let stepPlace = turn.steps.length - 1; stepPlace >= 0; stepPlace -= 1) {
      const step = turn.steps[stepPlace];
      if (step.kind === "tool" && step.card !== null && step.card.kind === "lane") {
        return `${turnPlace}-${stepPlace}`;
      }
    }
    return "";
  });

  // Where the playhead is on card `card` at `place`, in beats, or null when
  // it isn't the newest lane or nothing plays.
  function playheadOf(place: string, card: Card): number | null {
    if (phrase === null || place !== liveLane) {
      return null;
    }
    if (card.kind !== "lane") {
      return null;
    }
    const msPerBeat = 60000 / card.tempo;
    return (phraseNow - phrase.startedAt) / msPerBeat;
  }

  // While a run goes, thinking rows count the seconds.
  $effect(() => {
    if (!working) {
      return;
    }
    const timer = window.setInterval(() => {
      now = performance.now();
    }, 250);
    return () => window.clearInterval(timer);
  });

  function hear(event: HostEvent) {
    if (event.type === "status") {
      status = event as unknown as Status;
      problem = "";
      return;
    }
    if (event.type === "sign_in_prompt" || event.type === "sign_in_info") {
      hearSignIn(event);
      return;
    }
    if (event.type === "sign_in_done") {
      if (signIn === null) {
        return;
      }
      if (event.ok === true) {
        signIn = null;
        signInOpen = false;
      } else {
        signIn.failed = String(event.why);
        signIn.prompt = null;
      }
      return;
    }
    if (event.type === "new_chat") {
      turns = [];
      lastJob = null;
      openChatPath = null;
      return;
    }
    if (event.type === "chat") {
      openChatPath = String(event.path);
      return;
    }
    if (event.type === "chats") {
      chats = event.chats as ChatView[];
      openChatPath = event.open as string | null;
      return;
    }
    if (event.type === "chat_opened") {
      hearChatOpened(event);
      return;
    }
    if (event.type === "log") {
      console.debug("agent host:", event.text);
      return;
    }
    const turn = turns[turns.length - 1];
    const inRun = turn !== undefined && turn.running;
    if (inRun) {
      addToTurn(turn, event, performance.now());
      showLatest();
      return;
    }
    if (event.type === "error") {
      problem = String(event.message);
    } else if (event.type === "host_ended") {
      problem = hostEndedWhy(event);
      status = null;
      started = false;
    }
  }

  async function hearChatOpened(event: HostEvent) {
    const saved = event.turns as SavedTurn[];
    turns = saved.map((turn) => replayedTurn(turn));
    lastJob = event.job as Job | null;
    openChatPath = String(event.path);
    openThoughts = [];
    chatsOpen = false;
    problem = "";
    await tick();
    if (list !== undefined) {
      list.scrollTop = list.scrollHeight;
    }
  }

  function hearSignIn(event: HostEvent) {
    if (signIn === null) {
      return;
    }
    if (event.type === "sign_in_prompt") {
      signInAnswer = "";
      signIn.prompt = {
        kind: String(event.kind),
        message: String(event.message),
        placeholder: String(event.placeholder),
        options: event.options as { id: string; label: string }[],
      };
      return;
    }
    let url = "";
    if (typeof event.url === "string") {
      url = event.url;
    }
    let code = "";
    if (typeof event.code === "string") {
      code = event.code;
    }
    signIn.info = { message: String(event.message), url, code };
  }

  // Scrolls to the newest step, unless the player scrolled up to read.
  async function showLatest() {
    if (list === undefined) {
      return;
    }
    const nearEnd = list.scrollHeight - list.scrollTop - list.clientHeight < 80;
    await tick();
    if (nearEnd) {
      list.scrollTop = list.scrollHeight;
    }
  }

  async function send(text: string) {
    const said = text.trim();
    if (working || said === "") {
      return;
    }
    // Before the agent acts, Jello keeps the song, so the reply can be
    // undone.
    let reply: number | null = null;
    try {
      reply = await invoke<number>("agent_reply_starts");
    } catch {
      reply = null;
    }
    const jobChanged = lastJob !== job;
    lastJob = job;
    onSent();
    turns.push(newTurn(said, job, jobChanged, reply));
    draft = "";
    await tick();
    if (list !== undefined) {
      list.scrollTop = list.scrollHeight;
    }
    try {
      await invoke("agent_say", { text: said, job });
    } catch (error) {
      const turn = turns[turns.length - 1];
      turn.errors.push(String(error));
      turn.running = false;
    }
  }

  // Stop, or Escape while the agent works.
  export function stop() {
    invoke("stop_agent");
  }

  export function isWorking(): boolean {
    return working;
  }

  // Coaching after a practice try: the try's results go with the message
  // (src-tauri/src/practice_run.rs). While the agent works, they wait for
  // the next message.
  export function coach(number: number) {
    send(`Coach me on try ${number}.`);
  }

  async function newChat() {
    if (working) {
      return;
    }
    modelsOpen = false;
    chatsOpen = false;
    try {
      await invoke("agent_new_chat");
    } catch (error) {
      problem = String(error);
    }
  }

  async function undo(turn: Turn) {
    if (turn.reply === null) {
      return;
    }
    try {
      const left = await invoke<number[]>("undo_agent_reply", {
        reply: turn.reply,
      });
      for (const other of turns) {
        if (other.reply !== null && !left.includes(other.reply)) {
          other.undone = true;
        }
      }
    } catch (error) {
      turn.errors.push(String(error));
    }
    onSongChanged();
  }

  function answerQuestion(number: number, yes: boolean) {
    for (const turn of turns) {
      for (const question of turn.questions) {
        if (question.number === number) {
          question.answer = yes;
        }
      }
    }
    invoke("answer_agent", { number, yes });
  }

  async function pickModel(model: ModelView) {
    modelsOpen = false;
    try {
      await invoke("agent_pick_model", {
        provider: model.provider,
        model: model.id,
      });
    } catch (error) {
      problem = String(error);
    }
  }

  async function switchChats() {
    modelsOpen = false;
    if (chatsOpen) {
      chatsOpen = false;
      return;
    }
    chatsOpen = true;
    chatSearch = "";
    try {
      await invoke("agent_chats");
    } catch (error) {
      problem = String(error);
    }
  }

  async function openChat(chat: ChatView) {
    if (working) {
      return;
    }
    if (chat.path === openChatPath) {
      chatsOpen = false;
      return;
    }
    try {
      await invoke("agent_open_chat", { path: chat.path });
    } catch (error) {
      problem = String(error);
    }
  }

  function chatTime(chat: ChatView): string {
    return new Date(chat.modified).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function openSignIn() {
    modelsOpen = false;
    signInOpen = true;
  }

  async function startSignIn(provider: ProviderView, method: string) {
    signIn = { provider: provider.name, prompt: null, info: null, failed: null };
    try {
      await invoke("agent_sign_in", { provider: provider.id, method });
    } catch (error) {
      if (signIn !== null) {
        signIn.failed = String(error);
      }
    }
  }

  function answerSignIn(text: string) {
    if (signIn === null) {
      return;
    }
    signIn.prompt = null;
    signInAnswer = "";
    invoke("agent_sign_in_answer", { text });
  }

  function cancelSignIn() {
    invoke("agent_cancel_sign_in");
    signIn = null;
  }

  function closeSignIn() {
    signIn = null;
    signInOpen = false;
  }

  function switchThought(place: string) {
    if (openThoughts.includes(place)) {
      openThoughts = openThoughts.filter((open) => open !== place);
    } else {
      openThoughts = [...openThoughts, place];
    }
  }

  // Return sends; Shift+Return starts a new line. Neither reaches the keys.
  function onFieldKey(event: KeyboardEvent) {
    if (event.code !== "Enter" && event.code !== "NumpadEnter") {
      return;
    }
    event.stopPropagation();
    if (event.shiftKey) {
      return;
    }
    event.preventDefault();
    (event.currentTarget as HTMLElement).blur();
    send(draft);
  }

  function seconds(ms: number | null): string {
    if (ms === null) {
      return "";
    }
    if (ms < 1000) {
      return `${ms} ms`;
    }
    return `${(ms / 1000).toFixed(1)} s`;
  }

  const STARTERS: Record<Job, { label: string; text: string }[]> = {
    tutor: [
      { label: "First melody", text: "Teach me a first melody, one small step at a time." },
      { label: "Chords that fit", text: "Show me chords that work together, and play them for me." },
      { label: "Check my timing", text: "Look at what I just played and tell me how my timing was." },
    ],
    composer: [
      { label: "Start a song", text: "Help me start a song: suggest a key and a few chords, and play them." },
      { label: "What comes next", text: "Play me three chords that could come next in my song." },
      { label: "Find a sound", text: "Give the song a soft, warm pad sound." },
    ],
    producer: [
      { label: "Check the loop", text: "Listen to my loop and tell me what the measures say." },
      { label: "Balance layers", text: "Balance my loop layers' volumes." },
    ],
  };
</script>

<section class="agent-panel" class:is-hidden={!visible} aria-label="Agent">
  <header class="agent-top">
    <h2>{jobName(job)}</h2>
    <div class="agent-top-buttons">
      {#if status !== null && status.model !== null}
        <button
          type="button"
          class="agent-button-plain agent-model-button"
          class:is-open={modelsOpen}
          title="The model the agent uses"
          onclick={() => (modelsOpen = !modelsOpen)}
        >
          {status.model.name}
        </button>
      {/if}
      <button
        type="button"
        class="agent-button-plain"
        class:is-open={chatsOpen}
        disabled={status === null || working}
        onclick={switchChats}>Chats</button
      >
      <button
        type="button"
        class="agent-button-plain"
        disabled={working || turns.length === 0}
        onclick={newChat}>New</button
      >
    </div>
    {#if modelsOpen && status !== null}
      <div class="agent-models">
        {#each status.models as model (model.provider + model.id)}
          <button
            type="button"
            class:is-picked={status.model !== null &&
              status.model.provider === model.provider &&
              status.model.id === model.id}
            onclick={() => pickModel(model)}
          >
            <span>{model.name}</span>
            <span class="agent-quiet">{model.provider}</span>
          </button>
        {/each}
        <button type="button" class="agent-models-more" onclick={openSignIn}>
          Sign in to another provider
        </button>
      </div>
    {/if}
  </header>

  <div class="agent-body" bind:this={list} aria-live="polite">
    {#if status === null && problem === ""}
      <p class="agent-quiet agent-starting">Starting the agent…</p>
    {:else if needsSignIn && status !== null}
      <div class="agent-sign-in">
        <h1>Sign in</h1>
        <p class="agent-quiet">
          Pick where the agent's model comes from. The login stays on this
          computer, in Jello's settings.
        </p>
        {#if signIn === null}
          {#each providersShown as provider (provider.id)}
            <div class="agent-provider">
              <span class="agent-provider-name">{provider.name}</span>
              <span class="agent-provider-ways">
                {#if provider.browser !== null}
                  <button
                    type="button"
                    class="agent-button-plain"
                    onclick={() => startSignIn(provider, "oauth")}
                    >{provider.browser}</button
                  >
                {/if}
                {#if provider.key}
                  <button
                    type="button"
                    class="agent-button-plain"
                    onclick={() => startSignIn(provider, "api_key")}
                    >API key</button
                  >
                {/if}
              </span>
            </div>
          {/each}
          {#if !allProviders && status.providers.length > FIRST_PROVIDERS_SHOWN}
            <button
              type="button"
              class="agent-button-plain"
              onclick={() => (allProviders = true)}>More providers</button
            >
          {/if}
          {#if status.model !== null}
            <button type="button" class="agent-button-plain" onclick={closeSignIn}
              >Back</button
            >
          {/if}
        {:else}
          <p class="agent-sign-in-name">Signing in to {signIn.provider}</p>
          {#if signIn.info !== null}
            <p>{signIn.info.message}</p>
            {#if signIn.info.code !== ""}
              <p class="agent-code">{signIn.info.code}</p>
            {/if}
            {#if signIn.info.url !== ""}
              <p class="agent-url">{signIn.info.url}</p>
            {/if}
          {/if}
          {#if signIn.prompt !== null}
            <p>{signIn.prompt.message}</p>
            {#if signIn.prompt.kind === "select"}
              {#each signIn.prompt.options as option (option.id)}
                <button
                  type="button"
                  class="agent-button-plain"
                  onclick={() => answerSignIn(option.id)}>{option.label}</button
                >
              {/each}
            {:else}
              <form
                class="agent-sign-in-form"
                onsubmit={(event) => {
                  event.preventDefault();
                  answerSignIn(signInAnswer);
                }}
              >
                <input
                  type={signIn.prompt.kind === "secret" ? "password" : "text"}
                  bind:value={signInAnswer}
                  placeholder={signIn.prompt.placeholder}
                  autocomplete="off"
                  onkeydown={(event) => {
                    if (event.code === "Enter") {
                      event.stopPropagation();
                    }
                  }}
                />
                <button type="submit" class="agent-button-plain">Done</button>
              </form>
            {/if}
          {/if}
          {#if signIn.failed !== null}
            <p class="agent-error">{signIn.failed}</p>
            <button type="button" class="agent-button-plain" onclick={() => (signIn = null)}
              >Back</button
            >
          {:else}
            <button type="button" class="agent-button-plain" onclick={cancelSignIn}
              >Cancel</button
            >
          {/if}
        {/if}
      </div>
    {:else if chatsOpen && status !== null}
      <div class="agent-chats">
        <h1>Chats</h1>
        <input
          class="agent-chat-search"
          type="search"
          bind:value={chatSearch}
          placeholder="Search chats"
          aria-label="Search chats"
          autocomplete="off"
        />
        {#if chats === null}
          <p class="agent-quiet">Reading the chats…</p>
        {:else if chatGroups.length === 0}
          <p class="agent-quiet">
            {#if chats.length === 0}
              No chats yet. Every chat is kept in Music/Jello/Chats.
            {:else}
              No chat has "{chatSearch.trim()}".
            {/if}
          </p>
        {/if}
        {#each chatGroups as group (group.day)}
          <h3 class="agent-chat-day">{group.day}</h3>
          {#each group.chats as chat (chat.path)}
            <button
              type="button"
              class="agent-chat"
              class:is-picked={chat.path === openChatPath}
              disabled={working}
              onclick={() => openChat(chat)}
            >
              <span class="agent-chat-jobs">
                {#each chat.jobs as chatJob (chatJob)}
                  <span title={jobName(chatJob)}>{jobLetter(chatJob)}</span>
                {/each}
              </span>
              <span class="agent-chat-title">{chat.title}</span>
              <span class="agent-chat-song">{chat.song}</span>
              <span class="agent-step-time">{chatTime(chat)}</span>
            </button>
          {/each}
        {/each}
      </div>
    {:else if turns.length === 0 && status !== null}
      <div class="agent-empty">
        <h1>{jobName(job)}</h1>
        {#each STARTERS[job] as starter (starter.label)}
          <button
            type="button"
            class="agent-button-plain"
            onclick={() => send(starter.text)}>{starter.label}</button
          >
        {/each}
      </div>
    {:else}
      {#each turns as turn, turnPlace (turnPlace)}
        {#if turn.jobChanged && turnPlace > 0}
          <p class="agent-job-line">Now {jobName(turn.job)}</p>
        {/if}
        <p class="agent-said">{turn.said}</p>
        <div class="agent-run">
          {#each turn.steps as step, stepPlace (stepPlace)}
            {#if step.kind === "thinking"}
              {@const place = `${turnPlace}-${stepPlace}`}
              <button
                type="button"
                class="agent-step agent-thought"
                class:is-live={step.endedAt === null}
                onclick={() => switchThought(place)}
              >
                <span class="agent-dot"></span>
                <span class="agent-step-title">
                  {#if step.endedAt === null}
                    Thinking {thoughtSeconds(step, now)} s
                  {:else if turn.replayed}
                    Thought
                  {:else}
                    Thought for {thoughtSeconds(step, now)} s
                  {/if}
                </span>
              </button>
              {#if openThoughts.includes(place)}
                <p class="agent-thought-text">{step.text}</p>
              {/if}
            {:else if step.kind === "tool"}
              <div
                class="agent-step"
                class:is-live={!step.done}
                class:is-failed={step.failed}
              >
                <span class="agent-dot"></span>
                <span class="agent-step-title">{toolTitle(step)}</span>
                <span class="agent-step-detail">{toolDetail(step)}</span>
                <span class="agent-step-time">{seconds(step.ms)}</span>
              </div>
              {#if step.changes.length > 0}
                <ul class="agent-changes">
                  {#each step.changes as line, linePlace (linePlace)}
                    <li>{line}</li>
                  {/each}
                </ul>
              {/if}
              {#if step.failed}
                <p class="agent-error agent-step-why">{step.answer}</p>
              {:else if step.card !== null && step.card.kind === "options"}
                <AgentOptions card={step.card} {working} {onSongChanged} />
              {:else if step.card !== null}
                {@const place = `${turnPlace}-${stepPlace}`}
                <AgentCard card={step.card} playhead={playheadOf(place, step.card)} />
              {/if}
            {:else}
              <div class="agent-text"><Markdown text={step.text} /></div>
            {/if}
          {/each}
          {#if turn.running && turn.steps.length === 0}
            <div class="agent-step is-live">
              <span class="agent-dot"></span>
              <span class="agent-step-title">Starting</span>
            </div>
          {/if}
          {#each turn.questions as question (question.number)}
            <div class="agent-question">
              <p>{question.question}</p>
              {#if question.answer === null}
                <button
                  type="button"
                  class="agent-button-plain is-lit"
                  onclick={() => answerQuestion(question.number, true)}>Yes</button
                >
                <button
                  type="button"
                  class="agent-button-plain"
                  onclick={() => answerQuestion(question.number, false)}>No</button
                >
              {:else}
                <span class="agent-quiet">{question.answer ? "Yes" : "No"}</span>
              {/if}
            </div>
          {/each}
          {#each turn.errors as error, errorPlace (errorPlace)}
            <p class="agent-error">{error}</p>
          {/each}
          {#if !turn.running}
            <div class="agent-run-end">
              {#if turn.stopped}
                <span class="agent-quiet">Stopped</span>
              {/if}
              {#if cachedPercent(turn) !== null}
                <span class="agent-quiet">Cached {cachedPercent(turn)}%</span>
              {/if}
              {#if changedSomething(turn) && turn.reply !== null}
                {#if turn.undone}
                  <span class="agent-quiet">Undone</span>
                {:else}
                  <button
                    type="button"
                    class="agent-button-plain"
                    disabled={working}
                    onclick={() => undo(turn)}>Undo</button
                  >
                {/if}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
    {#if problem !== ""}
      <p class="agent-error">{problem}</p>
    {/if}
  </div>

  <footer class="agent-bottom">
    {#if working}
      <button type="button" class="agent-stop" onclick={stop}>
        <span>Stop</span><kbd>esc</kbd>
      </button>
    {/if}
    <form
      class="agent-field"
      onsubmit={(event) => {
        event.preventDefault();
        send(draft);
      }}
    >
      <textarea
        bind:value={draft}
        rows="2"
        maxlength="12000"
        placeholder={`Ask your ${jobName(job)}…`}
        aria-label={`Ask your ${jobName(job)}`}
        disabled={needsSignIn || status === null}
        onkeydown={onFieldKey}
      ></textarea>
      <button
        type="submit"
        class="agent-button-plain is-lit"
        disabled={working || draft.trim() === "" || needsSignIn}>Send</button
      >
    </form>
  </footer>
</section>
