<!-- The rail: Jello AI, docked on the right in every tab
     (docs/design.md#ai-and-learning-rethought-2026-11-03). One agent with
     no jobs; each message says what is open. Each run draws its steps live
     as rows: thinking, every tool with what it changed, the text as it
     comes. What an answer changes is one take, shown as a chip under it:
     flip (right Command), Keep (Return), Drop (Esc), or drop one part.
     Stop, or Escape, ends a run. Before a model is signed in to, the rail
     shows where to sign in. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open as openPicker } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import RailPractice from "./RailPractice.svelte";
  import RailDrill from "./RailDrill.svelte";
  import RailPartner from "./RailPartner.svelte";
  import AgentCard from "./AgentCard.svelte";
  import AgentMemory from "./AgentMemory.svelte";
  import AgentOptions from "./AgentOptions.svelte";
  import Markdown from "./Markdown.svelte";
  import { colorForNote } from "./note-colors";
  import {
    ATTACHABLE_ENDINGS,
    addToTurn,
    attachmentKindWord,
    cachedPercent,
    changedSomething,
    chatsByDay,
    chatsMatching,
    hostEndedWhy,
    newTurn,
    replayedTurn,
    thoughtSeconds,
    toolDetail,
    toolTitle,
    type AttachmentCard,
    type Card,
    type ChatView,
    type HostEvent,
    type ModelView,
    type ProviderView,
    type SavedTurn,
    type Turn,
  } from "./agent";
  import { takeStateWord, takeTitle, type TakeView } from "./takes";
  import {
    measuredMoments,
    momentMs,
    momentSeconds,
    pickedMoments,
    type JamView,
    type MomentPick,
    type ShownMoment,
  } from "./jam";

  interface Props {
    visible: boolean;
    // What is open, in words the agent reads, like "Compose tab, Sound
    // panel on the Filter page".
    place: string;
    // The song may have changed, by a take: redraw the sound, Compose and
    // the songs.
    onSongChanged: () => void;
    // The open take changed, or there is none now.
    onTake: (take: TakeView | null) => void;
    // The player sent a message, so the agent's marks on the keys go.
    onSent: () => void;
    // Whether the song's Learn switch is on, or null until Rust has said,
    // and what flipping it does (docs/design.md#ai-and-learning-rethought-2026-11-03).
    learn: boolean | null;
    onLearn: (on: boolean) => void;
  }
  let { visible, place, onSongChanged, onTake, onSent, learn, onLearn }: Props = $props();

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
  const FIRST_PROVIDERS_SHOWN = 3;

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
  // The Memory page: open or not.
  let memoryOpen = $state(false);
  // The thinking rows opened to their text, by turn and step.
  let openThoughts: string[] = $state([]);
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
    if (event.type === "moments_picked") {
      hearMomentsPicked(event);
      return;
    }
    const turn = turns[turns.length - 1];
    const inRun = turn !== undefined && turn.running;
    if (inRun) {
      addToTurn(turn, event, performance.now());
      showLatest();
      if (event.type === "run_end") {
        endTake(turn);
      }
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
    openChatPath = String(event.path);
    openThoughts = [];
    chatsOpen = false;
    memoryOpen = false;
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

  // The files waiting to go with the next message, while they're read,
  // and whether files are being dragged over the window.
  let waiting: AttachmentCard[] = $state([]);
  let attaching = $state(false);
  let dropping = $state(false);

  async function attach(paths: string[]) {
    if (paths.length === 0) {
      return;
    }
    attaching = true;
    try {
      waiting = await invoke<AttachmentCard[]>("agent_attach", { paths });
      problem = "";
    } catch (error) {
      problem = String(error);
    }
    attaching = false;
  }

  // The paperclip: the player picks files.
  async function pickFiles() {
    const picked = await openPicker({
      multiple: true,
      directory: false,
      filters: [{ name: "Pictures, PDFs, videos, sound and text", extensions: ATTACHABLE_ENDINGS }],
    });
    if (picked === null) {
      return;
    }
    if (Array.isArray(picked)) {
      await attach(picked);
    } else {
      await attach([picked]);
    }
  }

  async function unattach(card: AttachmentCard) {
    // Removing the hummed tune from the files ends the hum too.
    if (humRead !== null && humRead.card.id === card.id) {
      humRead = null;
      humStep = "off";
    }
    try {
      waiting = await invoke<AttachmentCard[]>("agent_unattach", { id: card.id });
    } catch (error) {
      problem = String(error);
    }
  }

  // Files dropped on the window while the panel shows go with the next
  // message.
  onMount(() => {
    const stop = getCurrentWebview().onDragDropEvent((event) => {
      if (!visible) {
        dropping = false;
        return;
      }
      const kind = event.payload.type;
      if (kind === "enter" || kind === "over") {
        dropping = true;
        return;
      }
      dropping = false;
      if (kind === "drop") {
        attach(event.payload.paths);
      }
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  });

  // How the player asks (docs/design.md#the-input-box): typing, speaking
  // or humming, one tab each at the top of the input box.
  type Mode = "type" | "speak" | "hum";
  let mode: Mode = $state("type");
  let field: HTMLTextAreaElement | undefined = $state();

  // Dictation (docs/design.md#voice): the player speaks, and the words heard
  // so far fill the message after what was typed before. "asking" is the
  // first time, before the speech model is downloaded; "starting" waits for
  // the mic to turn on.
  type Speech = "off" | "starting" | "asking" | "downloading" | "listening" | "finishing";
  let speech: Speech = $state("off");
  let speechModelMegabytes = $state(148);
  let downloaded = $state(0);
  let micLevel = $state(0);
  let micSeconds = $state(0);
  // What the message held before speaking began.
  let typedBefore = "";
  // Return while listening sends once the last words are in.
  let sendWhenHeard = false;

  function withSpoken(words: string): string {
    if (typedBefore === "") {
      return words;
    }
    if (words === "") {
      return typedBefore;
    }
    return `${typedBefore} ${words}`;
  }

  async function speak() {
    if (speech !== "off") {
      return;
    }
    speech = "starting";
    try {
      const status = await invoke<{ ready: boolean; downloading: boolean; megabytes: number }>(
        "dictation_status",
      );
      speechModelMegabytes = status.megabytes;
      if (status.downloading) {
        speech = "downloading";
        return;
      }
      if (!status.ready) {
        speech = "asking";
        return;
      }
      await startListening();
    } catch (error) {
      problem = String(error);
      speech = "off";
      mode = "type";
    }
  }

  async function downloadSpeechModel() {
    speech = "downloading";
    downloaded = 0;
    try {
      await invoke("download_dictation_model");
      problem = "";
    } catch (error) {
      problem = String(error);
      speech = "off";
      mode = "type";
      return;
    }
    // Back to the Speak tab's Start: the mic waits for the player.
    speech = "off";
  }

  function notNow() {
    speech = "off";
    pickType();
  }

  async function startListening() {
    typedBefore = draft.trim();
    sendWhenHeard = false;
    micLevel = 0;
    micSeconds = 0;
    try {
      await invoke("start_dictation", { input: chosenMic() });
      problem = "";
      speech = "listening";
      if (stopWhenListening) {
        stopWhenListening = false;
        sendWhenHeard = true;
        finishSpeaking();
        return;
      }
    } catch (error) {
      problem = String(error);
      speech = "off";
      mode = "type";
      return;
    }
    // Focused, so Return sends instead of reaching the keys.
    await tick();
    if (field !== undefined) {
      field.focus();
    }
  }

  // Done: the words stay in the message, and the Type tab opens to edit
  // them, unless Return asked to send them.
  async function finishSpeaking() {
    if (speech !== "listening") {
      return;
    }
    speech = "finishing";
    try {
      const words = await invoke<string>("stop_dictation");
      draft = withSpoken(words);
    } catch (error) {
      problem = String(error);
    }
    speech = "off";
    if (mode === "speak") {
      mode = "type";
    }
    if (!sendWhenHeard) {
      return;
    }
    sendWhenHeard = false;
    sendFromBox();
  }

  $effect(() => {
    const unlistenWords = listen<{ words: string }>("dictation-words", (event) => {
      if (speech === "listening") {
        draft = withSpoken(event.payload.words);
      }
    });
    const unlistenLevel = listen<{ peak: number; seconds: number }>("mic-level", (event) => {
      micLevel = Math.min(1, event.payload.peak);
      micSeconds = event.payload.seconds;
      if (humStep === "listening" && humStarted !== null && humSeconds() >= humStarted.most_seconds) {
        finishHum();
      }
    });
    const unlistenDownload = listen<{ received: number }>("dictation-download", (event) => {
      downloaded = event.payload.received;
    });
    const unlistenFull = listen("dictation-full", () => {
      finishSpeaking();
    });
    return () => {
      unlistenWords.then((unlisten) => unlisten());
      unlistenLevel.then((unlisten) => unlisten());
      unlistenDownload.then((unlisten) => unlisten());
      unlistenFull.then((unlisten) => unlisten());
    };
  });

  // How loud the mic is, as the meter's seven bars: each bar's share of the
  // level, so the row peaks in the middle.
  const BAR_SHARES = [0.3, 0.6, 1, 0.5, 0.8, 0.45, 0.2];
  const TALLEST_BAR_PX = 22;
  const SHORTEST_BAR_PX = 3;
  let barHeights = $derived.by(() => {
    const level = Math.sqrt(micLevel);
    const heights = [];
    for (const share of BAR_SHARES) {
      const height = Math.round(TALLEST_BAR_PX * level * share);
      heights.push(`${Math.max(SHORTEST_BAR_PX, height)}px`);
    }
    return heights;
  });

  // The mics there are, and the one picked: "" is the system's pick.
  let mics: string[] = $state([]);
  let micChoice = $state("");

  function chosenMic(): number | null {
    if (micChoice === "") {
      return null;
    }
    return Number(micChoice);
  }

  // Humming (docs/design.md#voice): Ready, Count in, Listening, Done. Jello
  // reads the notes on this computer, and the reading waits to go with the
  // next message as an attached file, with the player's description.
  type HumStep = "off" | "ready" | "counting" | "listening" | "reading" | "done";
  interface HumNote {
    note: number;
    name: string;
    start: number;
    hold: number;
  }
  interface HumStarted {
    tempo: number | null;
    count_in_beats: number;
    count_in_seconds: number;
    most_seconds: number;
  }
  interface HumRead {
    card: AttachmentCard;
    cards: AttachmentCard[];
    notes: HumNote[];
    reading: string;
  }
  const VIBES = ["happy", "sad", "dreamy", "dark", "bouncy", "calm", "epic"];
  let humStep: HumStep = $state("off");
  let humCountIn = $state(1);
  let humClick = $state(true);
  let humFree = $state(false);
  let humStarted: HumStarted | null = $state(null);
  let humBeat = $state(0);
  let humRead: HumRead | null = $state(null);
  let humProblem = $state("");
  let countInTimer: ReturnType<typeof setInterval> | null = null;

  async function openHum() {
    if (humStep !== "off") {
      return;
    }
    humStep = "ready";
    humProblem = "";
    try {
      mics = await invoke<string[]>("mic_inputs");
    } catch (error) {
      humProblem = String(error);
    }
  }

  async function startHum() {
    humProblem = "";
    micLevel = 0;
    micSeconds = 0;
    try {
      humStarted = await invoke<HumStarted>("start_hum", {
        input: chosenMic(),
        countInBars: humCountIn,
        click: humClick,
        free: humFree,
      });
    } catch (error) {
      humProblem = String(error);
      return;
    }
    if (humStarted.count_in_seconds === 0) {
      humStep = "listening";
      return;
    }
    humStep = "counting";
    humBeat = 1;
    const started = performance.now();
    countInTimer = setInterval(() => countIn(started), 20);
  }

  // Counts the beats of the count in, then listens.
  function countIn(started: number) {
    if (humStarted === null) {
      return;
    }
    const elapsed = (performance.now() - started) / 1000;
    if (elapsed >= humStarted.count_in_seconds) {
      stopCountIn();
      humStep = "listening";
      return;
    }
    const beatSeconds = humStarted.count_in_seconds / humStarted.count_in_beats;
    humBeat = Math.floor(elapsed / beatSeconds) + 1;
  }

  function stopCountIn() {
    if (countInTimer !== null) {
      clearInterval(countInTimer);
      countInTimer = null;
    }
  }

  // Seconds hummed, after the count in.
  function humSeconds(): number {
    if (humStarted === null) {
      return 0;
    }
    return Math.max(0, micSeconds - humStarted.count_in_seconds);
  }

  async function finishHum() {
    if (humStep !== "listening" && humStep !== "counting") {
      return;
    }
    stopCountIn();
    humStep = "reading";
    try {
      humRead = await invoke<HumRead>("stop_hum");
      waiting = humRead.cards;
      humStep = "done";
    } catch (error) {
      humProblem = String(error);
      humStep = "ready";
    }
  }

  // The hum read, taken off the next message.
  async function forgetHum() {
    if (humRead === null) {
      return;
    }
    const card = humRead.card;
    humRead = null;
    await unattach(card);
  }

  async function cancelHum() {
    stopCountIn();
    if (humStep === "reading") {
      return;
    }
    if (humStep === "counting" || humStep === "listening") {
      await invoke("cancel_hum");
    }
    await forgetHum();
    humStep = "off";
    pickType();
  }

  async function humAgain() {
    await forgetHum();
    humProblem = "";
    humStep = "ready";
  }

  // A vibe word goes on the end of the message, which describes the hum.
  function addVibe(word: string) {
    const now = draft.trim();
    if (now === "") {
      draft = word;
    } else {
      draft = `${now}, ${word}`;
    }
  }

  // Where each note heard sits on the little roll: across by time, up by
  // pitch.
  function rollOf(notes: HumNote[]) {
    let lowest = 127;
    let highest = 0;
    let end = 0;
    for (const heard of notes) {
      lowest = Math.min(lowest, heard.note);
      highest = Math.max(highest, heard.note);
      end = Math.max(end, heard.start + heard.hold);
    }
    const span = Math.max(1, highest - lowest);
    const drawn = [];
    for (const heard of notes) {
      drawn.push({
        name: heard.name,
        left: `${(heard.start / end) * 100}%`,
        width: `${Math.max(0.5, (heard.hold / end) * 100)}%`,
        bottom: `${((heard.note - lowest) / span) * 100}%`,
        color: `rgb(${colorForNote(heard.note).accent})`,
      });
    }
    return drawn;
  }

  function keyGuessOf(reading: string): string {
    for (const line of reading.split("\n")) {
      if (line.startsWith("Key guess")) {
        return line;
      }
    }
    return "";
  }

  // True from Send until the run starts, so a quick second click or
  // Return doesn't send the message twice.
  let sending = false;

  async function send(text: string) {
    const said = text.trim();
    if (working || attaching || sending) {
      return;
    }
    if (said === "" && waiting.length === 0) {
      return;
    }
    sending = true;
    // Before the agent acts, Jello starts a take, keeping the song as the
    // player has it. A take still open is kept, since the answer builds on
    // it.
    let takeNumber: number | null = null;
    try {
      takeNumber = await invoke<number>("take_starts");
    } catch {
      takeNumber = null;
    }
    keepOpenTakesInRail();
    onSent();
    const attached = [];
    for (const card of waiting) {
      attached.push({ name: card.name, kind: card.kind });
    }
    turns.push(newTurn(said, takeNumber, attached));
    sending = false;
    draft = "";
    waiting = [];
    // A hum that was read went with the files, so the Hum tab starts over.
    if (humStep === "done") {
      humRead = null;
      humStep = "off";
    }
    await tick();
    if (list !== undefined) {
      list.scrollTop = list.scrollHeight;
    }
    try {
      await invoke("agent_say", { text: said, place });
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

  // The jam card: the moments of the last jam, what each became, and the
  // press the next jam starts from.
  let jam: JamView | null = $state(null);
  let jamKept: Record<number, string> = $state({});
  let jamProblem = $state("");
  let jamSince = 0;
  // The moments the card shows, by place among the candidates. Jello's
  // best three at first; the AI's picks and names once they come.
  let jamShown: ShownMoment[] = $state([]);
  let jamPicking = $state(false);

  // A pause in playing: the card shows the moments worth keeping, if any.
  // It makes no sound and takes no key.
  export async function jamPaused() {
    let view: JamView;
    try {
      view = await invoke<JamView>("jam_moments", { since: jamSince });
    } catch {
      return;
    }
    jamSince = view.next;
    if (view.moments.length === 0) {
      return;
    }
    jam = view;
    jamKept = {};
    jamProblem = "";
    jamShown = measuredMoments(view);
    jamPicking = false;
    showLatest();
    try {
      jamPicking = await invoke<boolean>("pick_jam_moments", { next: view.next });
    } catch {
      jamPicking = false;
    }
  }

  // The AI's picks for the jam card. They replace Jello's own unless the
  // card is gone, is for another jam, or one of its moments was kept.
  function hearMomentsPicked(event: HostEvent) {
    if (jam === null || event.jam !== jam.next) {
      return;
    }
    jamPicking = false;
    if (event.picks === null || Object.keys(jamKept).length > 0) {
      return;
    }
    jamShown = pickedMoments(jam, event.picks as MomentPick[]);
  }

  // The moment playing now, so its Play waits until it ends, and whether
  // a Loop or Section is on its way, so a quick second click makes nothing.
  let jamPlaying: number | null = $state(null);
  let jamKeeping = false;
  let jamPlayTimer = 0;

  async function playMoment(place: number) {
    if (jam === null || jamPlaying !== null) {
      return;
    }
    jamProblem = "";
    jamPlaying = place;
    try {
      await invoke("play_jam_moment", { place });
    } catch (error) {
      jamProblem = String(error);
      jamPlaying = null;
      return;
    }
    window.clearTimeout(jamPlayTimer);
    jamPlayTimer = window.setTimeout(() => {
      jamPlaying = null;
    }, momentMs(jam.moments[place]));
  }

  // Loop or Section on a moment.
  async function keepMoment(place: number, how: string) {
    if (jamKeeping) {
      return;
    }
    jamKeeping = true;
    jamProblem = "";
    try {
      await invoke<string>("keep_jam_moment", { place, how });
      if (how === "loop") {
        jamKept[place] = "Looped";
      } else {
        jamKept[place] = "Saved as a section";
      }
    } catch (error) {
      jamProblem = String(error);
    }
    jamKeeping = false;
    onSongChanged();
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
    memoryOpen = false;
    try {
      await invoke("agent_new_chat");
    } catch (error) {
      problem = String(error);
    }
  }

  // The answer ended: its take opens, if it changed anything.
  async function endTake(turn: Turn) {
    if (turn.takeNumber === null) {
      return;
    }
    try {
      turn.take = await invoke<TakeView | null>("take_ends", { take: turn.takeNumber });
    } catch (error) {
      turn.errors.push(String(error));
      turn.take = null;
    }
    onTake(openTake());
  }

  // A take from a used option, on the turn whose card offered it.
  function tookOption(turn: Turn, take: TakeView | null) {
    keepOpenTakesInRail();
    turn.take = take;
    onTake(openTake());
    onSongChanged();
  }

  // Rust keeps an open take once a new one starts; the rail says so too.
  function keepOpenTakesInRail() {
    for (const turn of turns) {
      if (turn.take !== null && turn.take.state === "open") {
        turn.take.state = "kept";
      }
    }
  }

  // Another song opened. Rust keeps a take whose song was left, and the
  // rail says so too.
  export async function songOpened() {
    let stillOpen: TakeView | null = null;
    try {
      stillOpen = await invoke<TakeView | null>("open_take");
    } catch {
      return;
    }
    if (stillOpen !== null) {
      return;
    }
    keepOpenTakesInRail();
    onTake(null);
  }

  // The turn whose take is open, if there is one.
  function openTakeTurn(): Turn | null {
    for (const turn of turns) {
      if (turn.take !== null && turn.take.state === "open") {
        return turn;
      }
    }
    return null;
  }

  function openTake(): TakeView | null {
    const turn = openTakeTurn();
    if (turn === null) {
      return null;
    }
    return turn.take;
  }

  // "flip", "keep", "drop" or "drop part" on `turn`'s take.
  async function changeTake(turn: Turn, action: string, part: string | null = null) {
    if (turn.take === null) {
      return;
    }
    try {
      turn.take = await invoke<TakeView>("change_agent_take", {
        take: turn.take.number,
        action,
        part,
      });
    } catch (error) {
      turn.errors.push(String(error));
    }
    onTake(openTake());
    onSongChanged();
  }

  // Right Command tapped, Return and Esc: they act on the open take, and
  // say whether there was one.
  export function flipOpenTake(): boolean {
    return actOnOpenTake("flip");
  }

  export function keepOpenTake(): boolean {
    return actOnOpenTake("keep");
  }

  export function dropOpenTake(): boolean {
    return actOnOpenTake("drop");
  }

  function actOnOpenTake(action: string): boolean {
    if (working) {
      return false;
    }
    const turn = openTakeTurn();
    if (turn === null) {
      return false;
    }
    changeTake(turn, action);
    return true;
  }

  // Right Command held: talk into the rail. Let go, and what was heard is
  // sent.
  let stopWhenListening = false;

  export function talkStarts() {
    if (status === null || needsSignIn || working) {
      return;
    }
    if (speech !== "off" || mode === "hum") {
      return;
    }
    stopWhenListening = false;
    mode = "speak";
    speak();
  }

  export function talkEnds() {
    if (speech === "starting") {
      stopWhenListening = true;
      return;
    }
    if (speech !== "listening") {
      return;
    }
    sendWhenHeard = true;
    finishSpeaking();
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
    memoryOpen = false;
      return;
    }
    chatsOpen = true;
    memoryOpen = false;
    chatSearch = "";
    try {
      await invoke("agent_chats");
    } catch (error) {
      problem = String(error);
    }
  }

  function switchMemory() {
    modelsOpen = false;
    chatsOpen = false;
    memoryOpen = !memoryOpen;
  }

  async function openChat(chat: ChatView) {
    if (working) {
      return;
    }
    if (chat.path === openChatPath) {
      chatsOpen = false;
    memoryOpen = false;
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

  // While the mic counts in, listens or reads a hum, the other tabs wait
  // for Done or Cancel.
  let humBusy = $derived.by(() => {
    const busySteps: HumStep[] = ["counting", "listening", "reading"];
    return busySteps.includes(humStep);
  });

  // While the mic turns on or off, the tabs wait.
  let micSwitching = $derived.by(() => {
    const switchingSteps: Speech[] = ["starting", "finishing"];
    return switchingSteps.includes(speech);
  });

  async function pickType() {
    if (speech === "listening") {
      await finishSpeaking();
    }
    if (speech === "asking") {
      speech = "off";
    }
    mode = "type";
    await tick();
    if (field !== undefined) {
      field.focus();
    }
  }

  // Opening Speak never turns the mic on; its Start button does.
  function pickSpeak() {
    mode = "speak";
  }

  async function pickHum() {
    if (mode === "hum") {
      return;
    }
    if (speech === "listening") {
      await finishSpeaking();
    }
    if (speech === "asking") {
      speech = "off";
    }
    mode = "hum";
    await openHum();
  }

  // Send, or Return: while listening, once the last words are in; on the
  // Hum tab, only once the hum is read.
  function sendFromBox() {
    if (speech === "listening") {
      sendWhenHeard = true;
      finishSpeaking();
      return;
    }
    if (mode === "hum" && humStep !== "done") {
      return;
    }
    if (mode !== "type") {
      mode = "type";
    }
    send(draft);
  }

  // Send's state: off while there is nothing to send or something is
  // still being read or heard.
  let canSend = $derived.by(() => {
    if (status === null || needsSignIn || working || attaching) {
      return false;
    }
    if (speech === "starting" || speech === "finishing") {
      return false;
    }
    if (speech === "asking" || speech === "downloading") {
      return false;
    }
    if (mode === "hum" && humStep !== "done") {
      return false;
    }
    if (speech === "listening") {
      return true;
    }
    if (draft.trim() === "" && waiting.length === 0) {
      return false;
    }
    return true;
  });

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
    if (!canSend) {
      return;
    }
    (event.currentTarget as HTMLElement).blur();
    sendFromBox();
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

  const STARTERS: { label: string; text: string }[] = [
    { label: "Start a song", text: "Help me start a song: suggest a key and a few chords, and play them." },
    { label: "What comes next", text: "Play me three chords that could come next in my song." },
    { label: "Check my timing", text: "Look at what I just played and tell me how my timing was." },
    { label: "Find a sound", text: "Give the song a soft, warm pad sound." },
  ];
</script>

<section class="agent-panel" class:is-hidden={!visible} aria-label="Jello AI">
  <header class="agent-top">
    <h2>Jello AI</h2>
    <div class="agent-top-buttons">
      <button
        type="button"
        class="agent-learn"
        class:is-on={learn === true}
        role="switch"
        aria-checked={learn === true}
        disabled={learn === null}
        title="Learn: hints, checks and lessons on this song instead of takes"
        onclick={() => onLearn(learn !== true)}
      >
        Learn <span class="agent-learn-box" aria-hidden="true"></span>
      </button>
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
        class:is-open={memoryOpen}
        disabled={status === null || working}
        onclick={switchMemory}>Memory</button
      >
      <button
        type="button"
        class="agent-button-plain"
        disabled={working || turns.length === 0}
        onclick={newChat}>New</button
      >
    </div>
  </header>

  <div class="agent-body" bind:this={list} aria-live="polite">
    {#if status === null && problem === ""}
      <p class="agent-quiet agent-starting">Starting the agent…</p>
    {:else if memoryOpen && status !== null}
      <AgentMemory />
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
              <span class="agent-chat-title">{chat.title}</span>
              <span class="agent-chat-song">{chat.song}</span>
              <span class="agent-step-time">{chatTime(chat)}</span>
            </button>
          {/each}
        {/each}
      </div>
    {:else if turns.length === 0 && status !== null}
      <div class="agent-empty">
        {#each STARTERS as starter (starter.label)}
          <button
            type="button"
            class="agent-button-plain"
            onclick={() => send(starter.text)}>{starter.label}</button
          >
        {/each}
      </div>
    {:else}
      {#each turns as turn, turnPlace (turnPlace)}
        {#if turn.attachments.length > 0}
          <p class="agent-attached">
            {#each turn.attachments as file, filePlace (filePlace)}
              <span><b>{attachmentKindWord(file.kind)}</b> {file.name}</span>
            {/each}
          </p>
        {/if}
        {#if turn.said !== ""}
          <p class="agent-said">{turn.said}</p>
        {/if}
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
                <AgentOptions
                  card={step.card}
                  {working}
                  take={turn.take}
                  onTook={(take) => tookOption(turn, take)}
                  onDrop={() => changeTake(turn, "drop")}
                />
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
            </div>
            {#if turn.take !== null}
              {@const take = turn.take}
              <div class="agent-take" class:is-open={take.state === "open"}>
                <span class="agent-take-title">{takeTitle(take)}</span>
                {#if take.state === "open"}
                  <span class="agent-take-parts">
                    {#each take.parts as part (part)}
                      <button
                        type="button"
                        class="agent-take-part"
                        disabled={working}
                        title="Drop the {part} part of this take"
                        onclick={() => changeTake(turn, "drop part", part)}
                        >{part} <span aria-hidden="true">×</span></button
                      >
                    {/each}
                  </span>
                  <span class="agent-take-buttons">
                    <button
                      type="button"
                      class="agent-button-plain"
                      disabled={working}
                      title="Right ⌘ flips between your version and the take"
                      onclick={() => changeTake(turn, "flip")}
                      >{take.showingTake ? "Hear mine" : "Hear take"}</button
                    >
                    <button
                      type="button"
                      class="agent-button-plain is-lit"
                      disabled={working}
                      title="Return keeps it"
                      onclick={() => changeTake(turn, "keep")}>Keep</button
                    >
                    <button
                      type="button"
                      class="agent-button-plain"
                      disabled={working}
                      title="Esc drops it"
                      onclick={() => changeTake(turn, "drop")}>Drop</button
                    >
                  </span>
                {:else}
                  <span class="agent-quiet">{takeStateWord(take)}</span>
                {/if}
              </div>
            {:else if changedSomething(turn) && turn.takeNumber === null && !turn.replayed}
              <p class="agent-quiet">No take: this change can't be dropped.</p>
            {/if}
          {/if}
        </div>
      {/each}
    {/if}
    {#if !chatsOpen && !memoryOpen}
      <RailPractice onCoach={coach} {working} />
    {/if}
    {#if learn === true && !chatsOpen && !memoryOpen && !needsSignIn}
      <RailDrill />
    {/if}
    {#if status !== null && !chatsOpen && !memoryOpen && !needsSignIn}
      <RailPartner />
    {/if}
    {#if jam !== null && !chatsOpen && !memoryOpen && !needsSignIn}
      <div class="agent-jam">
        <div class="agent-jam-top">
          <span class="agent-jam-title">From your jam</span>
          {#if jamPicking}
            <span class="agent-quiet">Picking</span>
          {/if}
          <button
            type="button"
            class="agent-button-plain"
            title="Close the jam card"
            onclick={() => (jam = null)}>Close</button
          >
        </div>
        {#each jamShown as shown (shown.place)}
          {@const place = shown.place}
          {@const moment = jam.moments[place]}
          <div class="agent-moment">
            <span class="agent-moment-name">{shown.name}</span>
            {#if shown.picked}
              <span class="agent-quiet">{moment.name} · {momentSeconds(moment)}</span>
            {:else}
              <span class="agent-quiet">{momentSeconds(moment)}</span>
            {/if}
            <span class="agent-moment-notes">{moment.notes}</span>
            <span class="agent-moment-buttons">
              <button
                type="button"
                class="agent-button-plain agent-moment-play"
                class:is-open={jamPlaying === place}
                disabled={jamPlaying !== null && jamPlaying !== place}
                onclick={() => playMoment(place)}
              >
                {#if jamPlaying === place}
                  Playing
                {:else}
                  Play
                {/if}
              </button>
              {#if jamKept[place] === undefined}
                <button
                  type="button"
                  class="agent-button-plain"
                  onclick={() => keepMoment(place, "loop")}>Loop</button
                >
                <button
                  type="button"
                  class="agent-button-plain"
                  onclick={() => keepMoment(place, "section")}>Section</button
                >
              {:else}
                <span class="agent-quiet">{jamKept[place]}</span>
              {/if}
            </span>
          </div>
        {/each}
        {#if jamProblem !== ""}
          <p class="agent-error">{jamProblem}</p>
        {/if}
      </div>
    {/if}
    {#if problem !== ""}
      <p class="agent-error">{problem}</p>
    {/if}
  </div>

  <footer class="agent-bottom">
    <div class="agent-box" class:is-dropping={dropping}>
      <div class="agent-modes" role="tablist" aria-label="How to ask">
        <button
          type="button"
          role="tab"
          class="agent-mode"
          class:is-on={mode === "type"}
          aria-selected={mode === "type"}
          disabled={status === null || needsSignIn || humBusy || micSwitching}
          onclick={pickType}>Type</button
        >
        <button
          type="button"
          role="tab"
          class="agent-mode"
          class:is-on={mode === "speak"}
          aria-selected={mode === "speak"}
          disabled={status === null || needsSignIn || humBusy || micSwitching}
          onclick={pickSpeak}>Speak</button
        >
        <button
          type="button"
          role="tab"
          class="agent-mode"
          class:is-on={mode === "hum"}
          aria-selected={mode === "hum"}
          disabled={status === null || needsSignIn || micSwitching}
          onclick={pickHum}>Hum</button
        >
      </div>

      {#if waiting.length > 0 || attaching || dropping}
        <div class="agent-files">
          {#each waiting as card (card.id)}
            <div class="agent-file">
              <b>{attachmentKindWord(card.kind)}</b>
              <span class="agent-file-name">{card.name}</span>
              <span class="agent-quiet">{card.taken}</span>
              <button
                type="button"
                class="agent-button-plain agent-file-remove"
                aria-label={`Remove ${card.name}`}
                onclick={() => unattach(card)}>x</button
              >
            </div>
          {/each}
          {#if attaching}
            <p class="agent-quiet">Reading the files…</p>
          {:else if dropping}
            <p class="agent-quiet">Drop to attach to your next message</p>
          {/if}
        </div>
      {/if}

      {#if mode === "speak" && speech === "off"}
        <div class="agent-box-part agent-row">
          <span class="agent-quiet agent-grow">The mic turns on when you press Start.</span>
          <button type="button" class="agent-button-plain is-lit" onclick={speak}>Start</button>
        </div>
      {:else if mode === "speak" && speech === "starting"}
        <div class="agent-box-part">
          <p class="agent-quiet">Turning the mic on…</p>
        </div>
      {:else if mode === "speak" && speech === "asking"}
        <div class="agent-box-part">
          <p>
            Dictation turns your speech into words on this computer; the sound is never sent. It needs a
            speech model of {speechModelMegabytes} MB from Hugging Face, downloaded once.
          </p>
          <div class="agent-row">
            <button type="button" class="agent-button-plain is-lit" onclick={downloadSpeechModel}>Download</button>
            <button type="button" class="agent-button-plain" onclick={notNow}>Not now</button>
          </div>
        </div>
      {:else if mode === "speak" && speech === "downloading"}
        <div class="agent-box-part">
          <p class="agent-quiet">
            Downloading the speech model, {Math.floor(downloaded / 1_000_000)} of {speechModelMegabytes} MB…
          </p>
        </div>
      {:else if mode === "speak" && speech === "listening"}
        <div class="agent-box-part agent-row">
          {@render meter()}
          <span class="agent-quiet agent-grow">Listening, {micSeconds.toFixed(0)} s</span>
          <button type="button" class="agent-button-plain is-lit" onclick={finishSpeaking}>Done</button>
        </div>
      {:else if mode === "speak" && speech === "finishing"}
        <div class="agent-box-part">
          <p class="agent-quiet">Hearing the last words…</p>
        </div>
      {/if}

      {#if mode === "hum"}
        {@render humming()}
      {/if}

      {#if mode !== "hum" || humStep === "done"}
        <textarea
          class="agent-message"
          bind:this={field}
          bind:value={draft}
          rows="3"
          maxlength="12000"
          readonly={speech === "listening"}
          placeholder={mode === "hum" ? "Describe it" : "Ask, or hold right ⌘ and talk"}
          aria-label={mode === "hum" ? "Describe the hum" : "Ask Jello AI"}
          disabled={needsSignIn || status === null}
          onkeydown={onFieldKey}
        ></textarea>
      {/if}
      {#if mode === "hum" && humStep === "done"}
        <div class="agent-box-part agent-hum-vibes">
          {#each VIBES as vibe (vibe)}
            <button type="button" class="agent-button-plain" onclick={() => addVibe(vibe)}>{vibe}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="agent-under">
      <button
        type="button"
        class="agent-button-plain"
        title="Attach pictures, PDFs, videos, sound files or text"
        disabled={attaching || needsSignIn || status === null}
        onclick={pickFiles}>Attach</button
      >
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
      <span class="agent-grow"></span>
      {#if working}
        <button type="button" class="agent-button-plain is-lit agent-send" onclick={stop}>
          Stop <kbd>esc</kbd>
        </button>
      {:else}
        <button type="button" class="agent-button-plain is-lit agent-send" disabled={!canSend} onclick={sendFromBox}>
          Send <kbd>return</kbd>
        </button>
      {/if}
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
  </footer>
</section>

<!-- The mic's level as seven bars. -->
{#snippet meter()}
  <span class="agent-meter" aria-hidden="true">
    {#each barHeights as height, place (place)}
      <span style:height></span>
    {/each}
  </span>
{/snippet}

<!-- The Hum tab: Ready, Count in, Listening, Reading, then the notes read,
     described in the message below. -->
{#snippet humming()}
  <div class="agent-box-part agent-hum">
    {#if humStep === "ready"}
      <div class="agent-row">
        <b>Tempo</b>
        <button type="button" class="agent-button-plain" class:is-lit={!humFree} onclick={() => (humFree = false)}
          >The song's</button
        >
        <button type="button" class="agent-button-plain" class:is-lit={humFree} onclick={() => (humFree = true)}
          >Free</button
        >
      </div>
      {#if !humFree}
        <div class="agent-row">
          <b>Count in</b>
          {#each [0, 1, 2] as bars (bars)}
            <button
              type="button"
              class="agent-button-plain"
              class:is-lit={humCountIn === bars}
              onclick={() => (humCountIn = bars)}>{bars === 1 ? "1 bar" : `${bars} bars`}</button
            >
          {/each}
        </div>
        <div class="agent-row">
          <b>Click</b>
          <button type="button" class="agent-button-plain" class:is-lit={humClick} onclick={() => (humClick = true)}
            >On</button
          >
          <button type="button" class="agent-button-plain" class:is-lit={!humClick} onclick={() => (humClick = false)}
            >Off</button
          >
        </div>
      {/if}
      <div class="agent-row">
        <b>Mic</b>
        <select class="agent-mic-pick" bind:value={micChoice} aria-label="Mic">
          <option value="">The system's pick</option>
          {#each mics as name, place (place)}
            <option value={String(place)}>{name}</option>
          {/each}
        </select>
      </div>
      {#if humProblem !== ""}
        <p class="agent-error">{humProblem}</p>
      {/if}
      <div class="agent-row">
        <button type="button" class="agent-button-plain is-lit" onclick={startHum}>Start</button>
        <button type="button" class="agent-button-plain" onclick={cancelHum}>Cancel</button>
      </div>
    {:else if humStep === "counting" && humStarted !== null}
      <div class="agent-row">
        <b>Count in</b>
        <span class="agent-hum-beat">{humBeat}</span>
        <span class="agent-quiet agent-grow">of {humStarted.count_in_beats}, at {humStarted.tempo} BPM</span>
        <button type="button" class="agent-button-plain" onclick={cancelHum}>Cancel</button>
      </div>
    {:else if humStep === "listening"}
      <div class="agent-row">
        {@render meter()}
        <span class="agent-quiet agent-grow">Hum one note at a time, {humSeconds().toFixed(0)} s</span>
        <button type="button" class="agent-button-plain is-lit" onclick={finishHum}>Done</button>
        <button type="button" class="agent-button-plain" onclick={cancelHum}>Cancel</button>
      </div>
    {:else if humStep === "reading"}
      <p class="agent-quiet">Reading the notes…</p>
    {:else if humRead !== null}
      <div class="agent-hum-roll" aria-label="The notes heard">
        {#each rollOf(humRead.notes) as drawn, place (place)}
          <span
            title={drawn.name}
            style:left={drawn.left}
            style:width={drawn.width}
            style:bottom={drawn.bottom}
            style:background={drawn.color}
          ></span>
        {/each}
      </div>
      <div class="agent-row">
        <span class="agent-file-name">{humRead.card.taken}</span>
        <span class="agent-quiet">{keyGuessOf(humRead.reading)}</span>
      </div>
      <div class="agent-row">
        <button type="button" class="agent-button-plain" onclick={humAgain}>Hum again</button>
        <button type="button" class="agent-button-plain" onclick={cancelHum}>Cancel</button>
      </div>
    {/if}
  </div>
{/snippet}
