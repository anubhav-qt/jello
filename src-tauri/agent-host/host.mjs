// Jello's agent host (docs/design.md#the-agent-mode). Jello starts it with
// Node and keeps it running. It holds one pi chat, hands pi Jello's tools,
// and tells Jello everything pi does as it happens.
//
// Jello and the host talk in lines of JSON. Jello writes to the host's
// input: send a message, stop, start a new chat, pick a model, sign in. The
// host writes to its output: the chat's steps as they happen (thinking,
// each tool starting and ending, text arriving), and what it knows about
// models and sign-in.
//
// The tools are Jello's own, on its tool server in Rust (tool_server.rs).
// The host only passes calls on: pi has no shell, file or web tools here.
//
// Every chat is saved as pi saves its sessions, one JSONL file each, in
// Music/Jello/Chats. Jello's own notes go in the file as custom entries,
// which the model is never sent: the song the chat began on, and each job
// it went through.

import { spawn } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";
import {
  createAgentSession,
  createExtensionRuntime,
  ModelRuntime,
  SessionManager,
  SettingsManager,
} from "@earendil-works/pi-coding-agent";
import { JOBS, jobLine, systemPromptWith } from "./instructions.mjs";

// How much of a tool's answer the panel is told. A look at the whole song
// can be long; the model still gets all of it.
const MOST_ANSWER_CHARACTERS = 50000;

// Providers shown first when signing in.
const FIRST_PROVIDERS = ["anthropic", "openai", "google"];

// The custom entries Jello keeps in a chat's file: how it began, and each
// job it was switched to.
const CHAT_ENTRY = "jello.chat";
const JOB_ENTRY = "jello.job";

// How much of a chat's text is kept for searching the Chats page.
const MOST_SEARCH_CHARACTERS = 20000;

// A client for Jello's tool server at `url`, sending `password`. It speaks
// MCP's JSON-RPC: one message posted, one answer back.
export function toolServer(url, password) {
  let lastId = 0;

  async function ask(method, params) {
    lastId += 1;
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${password}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: lastId, method, params }),
    });
    if (!response.ok) {
      throw new Error(`Jello's tool server answered ${response.status}`);
    }
    const answer = await response.json();
    if (answer.error !== undefined) {
      throw new Error(answer.error.message);
    }
    return answer.result;
  }

  return { ask };
}

// Jello's tools, as pi tools that pass each call on to `server`.
export async function jelloTools(server) {
  await server.ask("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: "jello-agent-host", version: "1" },
  });
  const listed = await server.ask("tools/list", {});
  const tools = [];
  for (const tool of listed.tools) {
    tools.push(passedOn(server, tool));
  }
  return tools;
}

// The pi tool for `tool` from the server's list. Its answer keeps the
// lines saying what changed, like "Filter cutoff 1200 -> 800 Hz", and the
// card Jello drew, for the panel.
function passedOn(server, tool) {
  return {
    name: tool.name,
    label: tool.name,
    description: tool.description,
    parameters: tool.inputSchema,
    async execute(_callId, params) {
      const result = await server.ask("tools/call", {
        name: tool.name,
        arguments: params,
      });
      let changes = [];
      let card = null;
      if (result._meta !== undefined) {
        if (result._meta["jello/changes"] !== undefined) {
          changes = result._meta["jello/changes"];
        }
        if (result._meta["jello/card"] !== undefined) {
          card = result._meta["jello/card"];
        }
      }
      return {
        content: result.content,
        details: { changes, card },
        isError: result.isError === true,
      };
    },
  };
}

// What pi loads for Jello: the instructions and nothing else. No skills,
// extensions, prompt templates or context files from anywhere on disk.
function jelloResources(systemPrompt) {
  return {
    getExtensions: () => ({ extensions: [], errors: [], runtime: createExtensionRuntime() }),
    getSkills: () => ({ skills: [], diagnostics: [] }),
    getPrompts: () => ({ prompts: [], diagnostics: [] }),
    getThemes: () => ({ themes: [], diagnostics: [] }),
    getAgentsFiles: () => ({ agentsFiles: [] }),
    getSystemPrompt: () => systemPrompt,
    getSystemPromptSource: () => undefined,
    getAppendSystemPrompt: () => [],
    getAppendSystemPromptSources: () => [],
    extendResources: () => {},
    reload: async () => {},
  };
}

// Opens `url` in the browser, for signing in. Only web pages.
function openInBrowser(url) {
  const isWebPage = url.startsWith("https://") || url.startsWith("http://");
  if (!isWebPage) {
    return;
  }
  let program = "xdg-open";
  let args = [url];
  if (process.platform === "darwin") {
    program = "open";
  } else if (process.platform === "win32") {
    program = "rundll32";
    args = ["url.dll,FileProtocolHandler", url];
  }
  const child = spawn(program, args, { detached: true, stdio: "ignore" });
  child.on("error", () => {});
  child.unref();
}

// The text of a tool's answer, as long as the panel is told.
function answerText(result) {
  if (result === undefined || !Array.isArray(result.content)) {
    return "";
  }
  let text = "";
  for (const part of result.content) {
    if (part.type === "text") {
      text += part.text;
    }
  }
  return text.slice(0, MOST_ANSWER_CHARACTERS);
}

// The lines a tool's answer says changed.
function changesOf(result) {
  if (result === undefined || result.details === undefined) {
    return [];
  }
  if (!Array.isArray(result.details.changes)) {
    return [];
  }
  return result.details.changes;
}

// The card a tool's answer carries, or null.
function cardOf(result) {
  if (result === undefined || result.details === undefined) {
    return null;
  }
  if (result.details.card === undefined) {
    return null;
  }
  return result.details.card;
}

// The text of a message's content: a string, or a list of parts.
function contentText(content) {
  if (typeof content === "string") {
    return content;
  }
  if (!Array.isArray(content)) {
    return "";
  }
  let text = "";
  for (const part of content) {
    if (part.type === "text") {
      text += part.text;
    }
  }
  return text;
}

// What the player said, without the job line, the used options and the
// attached files Jello put before it.
export function withoutJobLine(text) {
  let said = text;
  for (const job of JOBS) {
    const line = `${jobLine(job)}\n\n`;
    if (said.startsWith(line)) {
      said = said.slice(line.length);
    }
  }
  if (said.startsWith(USED_START)) {
    const end = said.indexOf(")\n\n");
    if (end !== -1) {
      said = said.slice(end + 3);
    }
  }
  if (said.startsWith(ATTACHED_START)) {
    const end = said.indexOf(ATTACHED_END);
    if (end !== -1) {
      said = said.slice(end + ATTACHED_END.length);
    }
  }
  return said;
}

// The files attached to a message: what the model reads about each, put
// before what the player said between two marks, so a saved chat can show
// them as files again.
const ATTACHED_START = "<jello-attachments>\n";
const ATTACHED_END = "\n</jello-attachments>\n\n";
const NO_PICTURES =
  "(The model in use can't see pictures, so the pictures of these files were left out. Say so if they matter.)";

// The block for `attachments` ({name, kind, text}). `seesPictures` says
// whether the model takes pictures.
export function attachedBlock(attachments, seesPictures) {
  const parts = [];
  for (const attachment of attachments) {
    parts.push(`[Attached ${attachment.kind}: ${attachment.name}]\n${attachment.text}`);
  }
  let hasPictures = false;
  for (const attachment of attachments) {
    if ((attachment.images ?? []).length > 0) {
      hasPictures = true;
    }
  }
  if (hasPictures && !seesPictures) {
    parts.push(NO_PICTURES);
  }
  return `${ATTACHED_START}${parts.join("\n\n")}${ATTACHED_END}`;
}

// The files a sent message had, as {name, kind}, from its block.
export function attachedFiles(text) {
  const files = [];
  const start = text.indexOf(ATTACHED_START);
  if (start === -1) {
    return files;
  }
  const end = text.indexOf(ATTACHED_END, start);
  if (end === -1) {
    return files;
  }
  const block = text.slice(start + ATTACHED_START.length, end);
  for (const line of block.split("\n")) {
    if (!line.startsWith("[Attached ") || !line.endsWith("]")) {
      continue;
    }
    const inside = line.slice("[Attached ".length, -1);
    const colon = inside.indexOf(": ");
    if (colon === -1) {
      continue;
    }
    files.push({ kind: inside.slice(0, colon), name: inside.slice(colon + 2) });
  }
  return files;
}

// An attachment's pictures, as pi takes them.
function picturesOf(attachment) {
  const pictures = [];
  for (const image of attachment.images ?? []) {
    pictures.push({ type: "image", data: image.data, mimeType: image.mimeType });
  }
  return pictures;
}

// The largest file kept beside a chat, in bytes: bigger ones stay where
// they are.
const LARGEST_KEPT_FILE = 200_000_000;

// Copies each attached file into the folder beside the chat file
// `chatPath` (its name without .jsonl), so the chat keeps what it was
// given. A file that can't be copied is left where it is.
export function keepAttached(chatPath, attachments) {
  if (attachments.length === 0) {
    return;
  }
  const folder = chatPath.replace(/\.jsonl$/, "");
  try {
    mkdirSync(folder, { recursive: true });
  } catch {
    return;
  }
  for (const attachment of attachments) {
    try {
      if (statSync(attachment.path).size > LARGEST_KEPT_FILE) {
        continue;
      }
      copyFileSync(attachment.path, join(folder, basename(attachment.path)));
    } catch {
      continue;
    }
  }
}

// The line that tells the agent which options the player used since its
// last message, like "(Jello: the player used option 2, Darker, of offer
// 3.)". The panel leaves it out of what the player said.
const USED_START = "(Jello: ";

export function usedLine(used) {
  return `${USED_START}${used.join("; ")}.)`;
}

// The song a chat began on and the jobs it went through, from the custom
// entries in its file at `path`.
export function chatFacts(path) {
  const facts = { song: "", jobs: [] };
  let lines = [];
  try {
    lines = readFileSync(path, "utf8").split("\n");
  } catch {
    return facts;
  }
  for (const line of lines) {
    if (!line.includes('"custom"')) {
      continue;
    }
    let entry;
    try {
      entry = JSON.parse(line);
    } catch {
      continue;
    }
    if (entry.type !== "custom" || entry.data === undefined) {
      continue;
    }
    if (entry.customType === CHAT_ENTRY) {
      facts.song = String(entry.data.song ?? "");
    } else if (entry.customType === JOB_ENTRY && !facts.jobs.includes(entry.data.job)) {
      facts.jobs.push(entry.data.job);
    }
  }
  return facts;
}

// A saved chat's turns, as the events the panel draws a live run from, so
// an opened chat looks as it did. `entries` are the chat's, root first.
export function replayedTurns(entries) {
  const turns = [];
  let job = null;
  let jobChanged = false;
  let turn = null;
  for (const entry of entries) {
    if (entry.type === "custom" && entry.customType === JOB_ENTRY) {
      job = entry.data.job;
      jobChanged = true;
      continue;
    }
    if (entry.type !== "message") {
      continue;
    }
    const message = entry.message;
    if (message.role === "user") {
      const text = contentText(message.content);
      turn = { said: withoutJobLine(text), attachments: attachedFiles(text), job, jobChanged, events: [] };
      turns.push(turn);
      jobChanged = false;
    } else if (turn !== null && message.role === "assistant") {
      replayAnswer(turn.events, message);
    } else if (turn !== null && message.role === "toolResult") {
      turn.events.push({
        type: "tool_end",
        id: message.toolCallId,
        tool: message.toolName,
        failed: message.isError === true,
        changes: changesOf(message),
        card: cardOf(message),
        answer: answerText(message),
        ms: null,
      });
    }
  }
  return turns;
}

function replayAnswer(events, message) {
  if (Array.isArray(message.content)) {
    for (const part of message.content) {
      if (part.type === "thinking") {
        events.push({ type: "thinking_start" });
        events.push({ type: "thinking", text: part.thinking });
        events.push({ type: "thinking_end" });
      } else if (part.type === "text") {
        events.push({ type: "text", text: part.text });
      } else if (part.type === "toolCall") {
        events.push({ type: "tool_start", id: part.id, tool: part.name, args: part.arguments });
      }
    }
  }
  if (message.stopReason === "error") {
    events.push({ type: "error", message: message.errorMessage ?? "The model failed." });
  }
  if (message.stopReason === "aborted") {
    events.push({ type: "stopped" });
  }
  if (message.usage !== undefined) {
    events.push({
      type: "usage",
      input: message.usage.input,
      output: message.usage.output,
      cacheRead: message.usage.cacheRead,
      cacheWrite: message.usage.cacheWrite,
    });
  }
}

export class Host {
  // `tell` writes one event out. `agentDir` keeps the login and the model
  // picked, and `chatsDir` the chats. `runtime` is pi's model runtime;
  // `tools` are Jello's.
  constructor({ tell, agentDir, chatsDir, runtime, tools, openUrl = openInBrowser }) {
    this.tell = tell;
    this.agentDir = agentDir;
    this.chatsDir = chatsDir;
    this.runtime = runtime;
    this.tools = tools;
    this.openUrl = openUrl;
    this.session = null;
    // The chat's saved file, kept by pi's session manager.
    this.chat = null;
    this.model = null;
    // The job the chat was told last, so a new one gets its line.
    this.job = null;
    this.answering = false;
    this.stopped = false;
    // A sign-in under way: its cancel switch and the prompt it waits on.
    this.signIn = null;
    this.waitingPrompt = null;
  }

  // The file keeping the model picked last.
  choiceFile() {
    return join(this.agentDir, "jello-agent.json");
  }

  savedChoice() {
    try {
      return JSON.parse(readFileSync(this.choiceFile(), "utf8"));
    } catch {
      return null;
    }
  }

  saveChoice(model) {
    const choice = { provider: model.provider, model: model.id };
    writeFileSync(this.choiceFile(), JSON.stringify(choice));
  }

  // Picks the model picked last if it can still be used, or the first
  // that can, and tells Jello the models and providers.
  async refresh() {
    const available = await this.runtime.getAvailable();
    this.model = null;
    const saved = this.savedChoice();
    if (saved !== null) {
      for (const model of available) {
        if (model.provider === saved.provider && model.id === saved.model) {
          this.model = model;
        }
      }
    }
    if (this.model === null && available.length > 0) {
      this.model = available[0];
    }
    this.tellStatus(available);
  }

  tellStatus(available) {
    const models = [];
    for (const model of available) {
      models.push(this.modelView(model));
    }
    let model = null;
    if (this.model !== null) {
      model = this.modelView(this.model);
    }
    this.tell({ type: "status", models, model, providers: this.providers() });
  }

  modelView(model) {
    let name = model.id;
    if (typeof model.name === "string") {
      name = model.name;
    }
    return { provider: model.provider, id: model.id, name };
  }

  // The providers the player can sign in to, by browser or with an API key.
  providers() {
    const providers = [];
    for (const provider of this.runtime.getProviders()) {
      let browser = null;
      if (provider.auth.oauth !== undefined) {
        browser = provider.auth.oauth.loginLabel ?? provider.auth.oauth.name;
      }
      let key = false;
      if (provider.auth.apiKey !== undefined && provider.auth.apiKey.login !== undefined) {
        key = true;
      }
      if (browser === null && !key) {
        continue;
      }
      providers.push({ id: provider.id, name: provider.name, browser, key });
    }
    providers.sort((first, second) => {
      const firstPlace = FIRST_PROVIDERS.indexOf(first.id);
      const secondPlace = FIRST_PROVIDERS.indexOf(second.id);
      if (firstPlace !== secondPlace) {
        if (firstPlace === -1) {
          return 1;
        }
        if (secondPlace === -1) {
          return -1;
        }
        return firstPlace - secondPlace;
      }
      return first.name.localeCompare(second.name);
    });
    return providers;
  }

  // One message from Jello.
  async handle(message) {
    if (message.type === "send") {
      // Not waited for, so Stop can come in while it answers.
      this.send(
        message.text,
        message.job,
        message.song ?? "",
        message.used ?? [],
        message.memory ?? "",
        message.attachments ?? [],
      );
      return;
    }
    if (message.type === "chats") {
      await this.tellChats();
      return;
    }
    if (message.type === "open_chat") {
      await this.openChat(message.path);
      return;
    }
    if (message.type === "status") {
      await this.refresh();
      return;
    }
    if (message.type === "stop") {
      await this.stop();
      return;
    }
    if (message.type === "new") {
      await this.newChat();
      return;
    }
    if (message.type === "pick") {
      await this.pick(message.provider, message.model);
      return;
    }
    if (message.type === "sign_in") {
      await this.startSignIn(message.provider, message.method);
      return;
    }
    if (message.type === "answer") {
      this.answerPrompt(message.text);
      return;
    }
    if (message.type === "cancel_sign_in") {
      this.cancelSignIn();
      return;
    }
    this.tell({ type: "error", message: `The agent host doesn't know "${message.type}"` });
  }

  // The session for the chat in `chat`, pi's session manager, begun with
  // `memory`, the notes about the player as they were when it began.
  async makeSession(chat, memory) {
    const { session } = await createAgentSession({
      cwd: this.agentDir,
      agentDir: this.agentDir,
      modelRuntime: this.runtime,
      model: this.model,
      thinkingLevel: "medium",
      noTools: "builtin",
      customTools: this.tools,
      resourceLoader: jelloResources(systemPromptWith(memory)),
      sessionManager: chat,
      settingsManager: SettingsManager.inMemory({}),
    });
    session.subscribe((event) => this.passOn(event));
    this.session = session;
    this.chat = chat;
  }

  // A new chat, saved in chatsDir, begun on the song `song` with the
  // notes `memory`. Both are kept in the chat, so opening it again gives
  // the same instructions and the cache holds.
  async startChat(song, memory) {
    const chat = SessionManager.create(this.agentDir, this.chatsDir);
    chat.appendCustomEntry(CHAT_ENTRY, { song, memory });
    await this.makeSession(chat, memory);
  }

  // Sends the player's `text` to the chat, as `job`: "tutor", "composer"
  // or "producer". A new chat begins on the song `song`, with the notes
  // `memory`. `attachments` are the files attached ({name, path, kind,
  // text, images}).
  async send(text, job, song, used = [], memory = "", attachments = []) {
    if (this.answering) {
      this.tell({ type: "error", message: "The agent is still answering. Stop it first." });
      return;
    }
    if (this.model === null) {
      this.tell({ type: "error", message: "Sign in to a model first." });
      return;
    }
    this.answering = true;
    this.stopped = false;
    this.tell({ type: "run_start" });
    try {
      if (this.session === null) {
        await this.startChat(song, memory);
        this.tell({ type: "chat", path: this.chat.getSessionFile() });
      }
      let prompt = text;
      const images = [];
      if (attachments.length > 0) {
        const seesPictures = (this.model.input ?? []).includes("image");
        prompt = `${attachedBlock(attachments, seesPictures)}${prompt}`;
        if (seesPictures) {
          for (const attachment of attachments) {
            images.push(...picturesOf(attachment));
          }
        }
        keepAttached(this.chat.getSessionFile(), attachments);
      }
      if (used.length > 0) {
        prompt = `${usedLine(used)}\n\n${prompt}`;
      }
      if (job !== this.job) {
        prompt = `${jobLine(job)}\n\n${prompt}`;
        this.job = job;
        this.chat.appendCustomEntry(JOB_ENTRY, { job });
      }
      await this.session.prompt(prompt, { images });
    } catch (error) {
      this.tell({ type: "error", message: String(error.message ?? error) });
    } finally {
      this.answering = false;
      this.tell({ type: "run_end", stopped: this.stopped });
    }
  }

  // Tells Jello what pi just did, in Jello's own few kinds of event.
  passOn(event) {
    if (event.type === "message_update") {
      this.passOnStream(event.assistantMessageEvent);
      return;
    }
    if (event.type === "tool_execution_start") {
      this.tell({
        type: "tool_start",
        id: event.toolCallId,
        tool: event.toolName,
        args: event.args,
      });
      return;
    }
    if (event.type === "tool_execution_end") {
      this.tell({
        type: "tool_end",
        id: event.toolCallId,
        tool: event.toolName,
        failed: event.isError,
        changes: changesOf(event.result),
        card: cardOf(event.result),
        answer: answerText(event.result),
        ms: event.durationMs ?? null,
      });
      return;
    }
    if (event.type === "message_end") {
      this.passOnMessageEnd(event.message);
      return;
    }
    if (event.type === "auto_retry_start") {
      this.tell({
        type: "retry",
        attempt: event.attempt,
        attempts: event.maxAttempts,
        why: event.errorMessage,
      });
    }
  }

  passOnStream(streamed) {
    if (streamed.type === "thinking_start") {
      this.tell({ type: "thinking_start" });
    } else if (streamed.type === "thinking_delta") {
      this.tell({ type: "thinking", text: streamed.delta });
    } else if (streamed.type === "thinking_end") {
      this.tell({ type: "thinking_end" });
    } else if (streamed.type === "text_delta") {
      this.tell({ type: "text", text: streamed.delta });
    }
  }

  // An answer from the model ended: how many tokens it used and how many
  // came from the cache, or why it failed.
  passOnMessageEnd(message) {
    if (message.role !== "assistant") {
      return;
    }
    if (message.stopReason === "error") {
      this.tell({ type: "error", message: message.errorMessage ?? "The model failed." });
    }
    const usage = message.usage;
    if (usage !== undefined) {
      this.tell({
        type: "usage",
        input: usage.input,
        output: usage.output,
        cacheRead: usage.cacheRead,
        cacheWrite: usage.cacheWrite,
      });
    }
  }

  async stop() {
    if (this.session === null) {
      return;
    }
    this.stopped = true;
    await this.session.abort();
  }

  async newChat() {
    await this.closeChat();
    this.tell({ type: "new_chat" });
  }

  async closeChat() {
    if (this.session !== null) {
      await this.session.abort();
      this.session.dispose();
    }
    this.session = null;
    this.chat = null;
    this.job = null;
  }

  // Tells Jello every saved chat, newest first: its first message, the
  // song it began on, its jobs, when it last changed, and its text to
  // search.
  async tellChats() {
    const listed = await SessionManager.list(this.agentDir, this.chatsDir);
    const chats = [];
    for (const info of listed) {
      const facts = chatFacts(info.path);
      chats.push({
        path: info.path,
        title: withoutJobLine(info.firstMessage).slice(0, 200),
        song: facts.song,
        jobs: facts.jobs,
        modified: info.modified.getTime(),
        text: info.allMessagesText.slice(0, MOST_SEARCH_CHARACTERS),
      });
    }
    chats.sort((first, second) => second.modified - first.modified);
    let path = null;
    if (this.chat !== null) {
      path = this.chat.getSessionFile();
    }
    this.tell({ type: "chats", chats, open: path });
  }

  // Whether `path` is a chat file in chatsDir, and nothing else.
  isChatFile(path) {
    if (typeof path !== "string" || !path.endsWith(".jsonl")) {
      return false;
    }
    const whole = resolve(path);
    return dirname(whole) === resolve(this.chatsDir) && basename(whole) === basename(path);
  }

  // Opens the saved chat at `path` to carry on where it stopped, and tells
  // Jello its turns to draw.
  async openChat(path) {
    if (this.answering) {
      this.tell({ type: "error", message: "The agent is still answering. Stop it first." });
      return;
    }
    if (!this.isChatFile(path)) {
      this.tell({ type: "error", message: "That isn't one of Jello's chats." });
      return;
    }
    if (this.model === null) {
      this.tell({ type: "error", message: "Sign in to a model first." });
      return;
    }
    await this.closeChat();
    const chat = SessionManager.open(path, this.chatsDir);
    const entries = chat.getBranch();
    let job = null;
    let memory = "";
    for (const entry of entries) {
      if (entry.type !== "custom") {
        continue;
      }
      if (entry.customType === JOB_ENTRY) {
        job = entry.data.job;
      }
      if (entry.customType === CHAT_ENTRY) {
        memory = String(entry.data.memory ?? "");
      }
    }
    await this.makeSession(chat, memory);
    this.job = job;
    this.tell({ type: "chat_opened", path, job, turns: replayedTurns(entries) });
  }

  async pick(providerId, modelId) {
    const available = await this.runtime.getAvailable();
    let picked = null;
    for (const model of available) {
      if (model.provider === providerId && model.id === modelId) {
        picked = model;
      }
    }
    if (picked === null) {
      this.tell({ type: "error", message: `${modelId} can't be used: sign in to ${providerId} first.` });
      return;
    }
    this.model = picked;
    this.saveChoice(picked);
    if (this.session !== null) {
      await this.session.setModel(picked);
    }
    this.tellStatus(available);
  }

  // Signs in to `providerId`, by "oauth" (the browser) or "api_key". pi
  // asks its questions through Jello's panel and keeps the login in
  // agentDir.
  async startSignIn(providerId, method) {
    if (this.signIn !== null) {
      this.cancelSignIn();
    }
    const controller = new AbortController();
    this.signIn = controller;
    const interaction = {
      signal: controller.signal,
      prompt: (asked) => this.askPlayer(asked),
      notify: (event) => this.notifyPlayer(event),
    };
    try {
      await this.runtime.login(providerId, method, interaction);
      this.tell({ type: "sign_in_done", ok: true });
      await this.refresh();
    } catch (error) {
      this.tell({ type: "sign_in_done", ok: false, why: String(error.message ?? error) });
    } finally {
      this.signIn = null;
      this.waitingPrompt = null;
    }
  }

  // pi asks the player something while signing in, like an API key or a
  // code. The panel shows it, and the answer comes back as "answer".
  askPlayer(asked) {
    return new Promise((resolve, reject) => {
      this.waitingPrompt = { resolve, reject };
      let options = [];
      if (asked.type === "select") {
        options = asked.options;
      }
      this.tell({
        type: "sign_in_prompt",
        kind: asked.type,
        message: asked.message,
        placeholder: asked.placeholder ?? "",
        options,
      });
      if (asked.signal !== undefined) {
        asked.signal.addEventListener("abort", () => reject(new Error("The question was withdrawn.")));
      }
    });
  }

  notifyPlayer(event) {
    if (event.type === "auth_url") {
      this.openUrl(event.url);
      const message = event.instructions ?? "Finish signing in in your browser.";
      this.tell({ type: "sign_in_info", message, url: event.url });
    } else if (event.type === "device_code") {
      this.openUrl(event.verificationUri);
      this.tell({
        type: "sign_in_info",
        message: `Enter this code in your browser: ${event.userCode}`,
        url: event.verificationUri,
        code: event.userCode,
      });
    } else {
      this.tell({ type: "sign_in_info", message: event.message });
    }
  }

  answerPrompt(text) {
    if (this.waitingPrompt === null) {
      return;
    }
    const waiting = this.waitingPrompt;
    this.waitingPrompt = null;
    waiting.resolve(text);
  }

  cancelSignIn() {
    if (this.signIn === null) {
      return;
    }
    this.signIn.abort();
    if (this.waitingPrompt !== null) {
      this.waitingPrompt.reject(new Error("Signing in was cancelled."));
      this.waitingPrompt = null;
    }
  }
}

// Started by Jello: JELLO_TOOLS_URL and JELLO_TOOLS_PASSWORD reach the tool
// server, JELLO_AGENT_DIR keeps the login, readable by the player's
// account only, and JELLO_CHATS_DIR the chats.
async function main() {
  const tell = (event) => process.stdout.write(`${JSON.stringify(event)}\n`);
  const agentDir = process.env.JELLO_AGENT_DIR;
  const chatsDir = process.env.JELLO_CHATS_DIR;
  mkdirSync(agentDir, { recursive: true, mode: 0o700 });
  mkdirSync(chatsDir, { recursive: true });
  // The provider keeps the start of the prompt cached as long as it
  // offers, since it stays the same through a chat.
  process.env.PI_CACHE_RETENTION = "long";
  let host;
  try {
    const runtime = await ModelRuntime.create({
      authPath: join(agentDir, "auth.json"),
      modelsPath: join(agentDir, "models.json"),
    });
    const server = toolServer(process.env.JELLO_TOOLS_URL, process.env.JELLO_TOOLS_PASSWORD);
    const tools = await jelloTools(server);
    host = new Host({ tell, agentDir, chatsDir, runtime, tools });
    await host.refresh();
  } catch (error) {
    tell({ type: "error", message: `The agent couldn't start: ${error.message ?? error}` });
    process.exit(1);
  }

  // Jello closing the host's input means Jello quit.
  const lines = createInterface({ input: process.stdin });
  lines.on("line", (line) => {
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      tell({ type: "error", message: "The agent host got a line that isn't JSON." });
      return;
    }
    host.handle(message).catch((error) => {
      tell({ type: "error", message: String(error.message ?? error) });
    });
  });
  lines.on("close", () => process.exit(0));
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
