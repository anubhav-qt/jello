// The agent host against a fake tool server and pi's fake model, so no
// sign-in or network is needed (src-tauri/agent-host/host.mjs).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ModelRuntime } from "@earendil-works/pi-coding-agent";
import {
  fauxAssistantMessage,
  fauxProvider,
  fauxText,
  fauxThinking,
  fauxToolCall,
} from "@earendil-works/pi-ai";
import { attachedBlock, attachedFiles, Host, jelloTools, toolServer, withoutJobLine } from "../../src-tauri/agent-host/host.mjs";
import { INSTRUCTIONS } from "../../src-tauri/agent-host/instructions.mjs";

const PASSWORD = "secret";
// A red picture, 8 by 8, as a JPEG in base64.
const RED_JPEG =
  "/9j/4AAQSkZJRgABAgAAAQABAAD//gAPTGF2YzYzLjEuMTAyAP/bAEMACAQEBAQEBQUFBQUFBgYGBgYGBgYGBgYGBgcHBwgICAcHBwYGBwcICAgICQkJCAgICAkJCgoKDAwLCw4ODhERFP/EAEwAAQEAAAAAAAAAAAAAAAAAAAAGAQEBAAAAAAAAAAAAAAAAAAAGBxABAAAAAAAAAAAAAAAAAAAAABEBAAAAAAAAAAAAAAAAAAAAAP/AABEIAAgACAMBIgACEQADEQD/2gAMAwEAAhEDEQA/AIsATX9//9k=";

// A tool server like Jello's with one tool, jello_sound. It keeps every
// call it was asked, and every request's headers.
async function fakeToolServer() {
  const calls = [];
  const headers = [];
  const server = createServer((request, response) => {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      headers.push(request.headers);
      if (request.headers.authorization !== `Bearer ${PASSWORD}`) {
        response.writeHead(401);
        response.end();
        return;
      }
      const message = JSON.parse(body);
      let result = {};
      if (message.method === "tools/list") {
        result = {
          tools: [
            {
              name: "jello_sound",
              description: "Changes sound values.",
              inputSchema: {
                type: "object",
                properties: { changes: { type: "object" } },
                required: ["changes"],
              },
            },
          ],
        };
      } else if (message.method === "tools/call") {
        calls.push(message.params);
        result = {
          content: [{ type: "text", text: "{\"filter\":{\"cutoff_hz\":800}}" }],
          isError: false,
          _meta: {
            "jello/changes": ["Filter cutoff 1200 -> 800 Hz"],
            "jello/card": { kind: "sound", title: "" },
          },
        };
      }
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }));
    });
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const url = `http://127.0.0.1:${server.address().port}/mcp`;
  return { server, url, calls, headers };
}

// A host on the fake model, with the fake model picked, and every event it
// told kept in `events`.
async function fakeHost(tools) {
  const agentDir = mkdtempSync(join(tmpdir(), "jello-agent-host-"));
  const runtime = await ModelRuntime.create({
    authPath: join(agentDir, "auth.json"),
    modelsPath: null,
    refreshOnCreate: false,
  });
  const faux = fauxProvider({ tokensPerSecond: 1000 });
  runtime.registerNativeProvider(faux.provider);
  await runtime.refresh({ allowNetwork: false });
  const model = faux.getModel();
  writeFileSync(
    join(agentDir, "jello-agent.json"),
    JSON.stringify({ provider: model.provider, model: model.id }),
  );
  const events = [];
  const opened = [];
  const host = new Host({
    tell: (event) => events.push(event),
    agentDir,
    chatsDir: join(agentDir, "Chats"),
    runtime,
    tools,
    openUrl: (url) => opened.push(url),
  });
  await host.refresh();
  return { host, faux, model, events, opened, agentDir };
}

// The player's messages in what the model was sent.
function playerMessages(context) {
  const said = [];
  for (const message of context.messages) {
    if (message.role === "user") {
      said.push(message);
    }
  }
  return said;
}

function typesOf(events) {
  const types = [];
  for (const event of events) {
    types.push(event.type);
  }
  return types;
}

test("a message runs live: thinking, the tool call with its changes, text, the end", async () => {
  const tools = await fakeToolServer();
  const made = await fakeHost(await jelloTools(toolServer(tools.url, PASSWORD)));
  const seen = [];
  try {
    made.faux.setResponses([
      (context) => {
        seen.push(context);
        return fauxAssistantMessage([
          fauxThinking("Darker means a lower cutoff."),
          fauxToolCall("jello_sound", { changes: { filter: { cutoff_hz: 800 } } }),
        ], { stopReason: "toolUse" });
      },
      fauxAssistantMessage([fauxText("Darker now.")]),
      (context) => {
        seen.push(context);
        return fauxAssistantMessage([fauxText("Still Composer.")]);
      },
    ]);

    assert.equal(made.events[0].type, "status");
    assert.equal(made.events[0].model.id, made.model.id);

    made.events.length = 0;
    await made.host.send("Make it darker", "composer");
    const types = typesOf(made.events);
    assert.equal(types[0], "run_start");
    assert.ok(types.indexOf("thinking_start") < types.indexOf("tool_start"));
    assert.ok(types.indexOf("tool_start") < types.indexOf("tool_end"));
    assert.ok(types.indexOf("tool_end") < types.indexOf("text"));
    assert.equal(types[types.length - 1], "run_end");

    const ended = made.events.find((event) => event.type === "tool_end");
    assert.equal(ended.tool, "jello_sound");
    assert.equal(ended.failed, false);
    assert.deepEqual(ended.changes, ["Filter cutoff 1200 -> 800 Hz"]);
    assert.deepEqual(ended.card, { kind: "sound", title: "" });
    assert.deepEqual(tools.calls[0], {
      name: "jello_sound",
      arguments: { changes: { filter: { cutoff_hz: 800 } } },
    });

    let text = "";
    for (const event of made.events) {
      if (event.type === "text") {
        text += event.text;
      }
    }
    assert.equal(text, "Darker now.");

    // The model gets Jello's instructions, only Jello's tools, and the job.
    assert.ok(JSON.stringify(seen[0]).includes(JSON.stringify(INSTRUCTIONS).slice(1, -1)));
    assert.deepEqual(made.host.session.getActiveToolNames(), ["jello_sound"]);
    const first = JSON.stringify(playerMessages(seen[0])[0]);
    assert.ok(first.includes("Now Composer, in the Compose tab."));
    assert.ok(first.includes("Make it darker"));

    // The same job again gets no new line, so the start stays cached.
    await made.host.send("Thanks", "composer");
    const said = playerMessages(seen[1]);
    const last = JSON.stringify(said[said.length - 1]);
    assert.ok(last.includes("Thanks"));
    assert.ok(!last.includes("Now Composer"));

    // Fetch sends no Origin header, which Jello's server would refuse.
    for (const header of tools.headers) {
      assert.equal(header.origin, undefined);
    }
  } finally {
    tools.server.close();
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("Stop ends a run part way", async () => {
  const made = await fakeHost([]);
  try {
    made.faux.setResponses([fauxAssistantMessage([fauxText("word ".repeat(400))])]);
    const running = made.host.send("Talk a lot", "tutor");
    // Stop once the text starts coming.
    await new Promise((done) => {
      const timer = setInterval(() => {
        if (made.events.some((event) => event.type === "text")) {
          clearInterval(timer);
          done();
        }
      }, 5);
    });
    await made.host.stop();
    await running;
    const end = made.events[made.events.length - 1];
    assert.equal(end.type, "run_end");
    assert.equal(end.stopped, true);
  } finally {
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("one run at a time, a model that can't be used is refused, and a new chat forgets the job", async () => {
  const made = await fakeHost([]);
  try {
    made.faux.setResponses([
      fauxAssistantMessage([fauxText("one")]),
      fauxAssistantMessage([fauxText("two")]),
    ]);
    const first = made.host.send("One", "tutor");
    await made.host.send("Two", "tutor");
    await first;
    assert.ok(made.events.some((event) => event.message === "The agent is still answering. Stop it first."));

    await made.host.pick("anthropic", "not-a-model");
    assert.match(made.events[made.events.length - 1].message, /sign in to anthropic first/);
    assert.equal(made.host.model.id, made.model.id);

    await made.host.newChat();
    assert.equal(made.host.session, null);
    assert.equal(made.host.job, null);
    assert.equal(made.events[made.events.length - 1].type, "new_chat");
  } finally {
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("signing in asks through the panel and opens only web pages", async () => {
  const made = await fakeHost([]);
  try {
    made.host.notifyPlayer({ type: "auth_url", url: "https://example.com/login" });
    assert.deepEqual(made.opened, ["https://example.com/login"]);
    const info = made.events[made.events.length - 1];
    assert.equal(info.type, "sign_in_info");
    assert.equal(info.url, "https://example.com/login");

    const answer = made.host.askPlayer({ type: "secret", message: "API key" });
    const asked = made.events[made.events.length - 1];
    assert.equal(asked.type, "sign_in_prompt");
    assert.equal(asked.kind, "secret");
    made.host.answerPrompt("typed");
    assert.equal(await answer, "typed");

    const providers = made.host.providers();
    const first = providers.slice(0, 3).map((provider) => provider.id);
    assert.deepEqual(first, ["anthropic", "openai", "google"]);
    assert.equal(providers[1].browser, "Sign in with ChatGPT");
  } finally {
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("chats are saved with their song, jobs and memory, listed, and carried on once opened", async () => {
  const tools = await fakeToolServer();
  const made = await fakeHost(await jelloTools(toolServer(tools.url, PASSWORD)));
  const seen = [];
  try {
    made.faux.setResponses([
      fauxAssistantMessage([
        fauxThinking("Lower cutoff."),
        fauxToolCall("jello_sound", { changes: { filter: { cutoff_hz: 800 } } }),
      ], { stopReason: "toolUse" }),
      fauxAssistantMessage([fauxText("Darker now.")]),
      fauxAssistantMessage([fauxText("Play C, then G.")]),
      (context) => {
        seen.push(context);
        return fauxAssistantMessage([fauxText("Again, slower.")]);
      },
    ]);
    await made.host.send("Make it darker", "composer", "Prism Drift", [], "Habits:\n- [1] Rushes bar 3 (2026-10-24)");
    await made.host.send("Teach me the chorus", "tutor", "Other song", ["the player used option 2, Darker, of offer 1"]);
    const path = made.events.find((event) => event.type === "chat").path;

    await made.host.tellChats();
    const listed = made.events[made.events.length - 1];
    assert.equal(listed.type, "chats");
    assert.equal(listed.chats.length, 1);
    assert.equal(listed.open, path);
    const chat = listed.chats[0];
    assert.equal(chat.path, path);
    assert.equal(chat.title, "Make it darker");
    assert.equal(chat.song, "Prism Drift");
    assert.deepEqual(chat.jobs, ["composer", "tutor"]);
    assert.ok(chat.text.includes("Teach me the chorus"));

    await made.host.newChat();
    await made.host.openChat(path);
    const opened = made.events[made.events.length - 1];
    assert.equal(opened.type, "chat_opened");
    assert.equal(opened.job, "tutor");
    assert.equal(made.host.job, "tutor");
    assert.equal(opened.turns.length, 2);
    const [first, second] = opened.turns;
    assert.equal(first.said, "Make it darker");
    assert.equal(first.job, "composer");
    // The player's message drops the job line and the options used.
    assert.equal(second.said, "Teach me the chorus");
    assert.equal(second.job, "tutor");
    assert.equal(second.jobChanged, true);
    const ended = first.events.find((event) => event.type === "tool_end");
    assert.deepEqual(ended.changes, ["Filter cutoff 1200 -> 800 Hz"]);
    assert.deepEqual(ended.card, { kind: "sound", title: "" });
    assert.ok(first.events.some((event) => event.type === "thinking" && event.text === "Lower cutoff."));
    assert.ok(first.events.some((event) => event.type === "text" && event.text === "Darker now."));

    // Carrying on: the model gets the whole chat, the same job needs no
    // new line, and it stays one file.
    await made.host.send("Once more", "tutor", "Other song", [], "Goals:\n- [2] Play at 90 (2026-10-24)");
    const sent = JSON.stringify(seen[0]);
    // The chat keeps the memory it began with, so its start stays cached.
    assert.ok(sent.includes("Rushes bar 3"));
    assert.ok(!sent.includes("Play at 90"));
    assert.ok(sent.includes("Make it darker"));
    assert.ok(sent.includes("(Jello: the player used option 2, Darker, of offer 1.)"));
    const said = playerMessages(seen[0]);
    assert.ok(!JSON.stringify(said[said.length - 1]).includes("Now Tutor"));
    await made.host.tellChats();
    assert.equal(made.events[made.events.length - 1].chats.length, 1);
  } finally {
    tools.server.close();
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("attached files go with one message, are kept beside the chat and show again", async () => {
  const made = await fakeHost([]);
  const seen = [];
  try {
    const original = join(made.agentDir, "Score.pdf");
    writeFileSync(original, "pdf bytes");
    const attachments = [
      {
        name: "Score.pdf",
        path: original,
        kind: "pdf",
        text: "A PDF of 1 pages.\n\nPage 1:\nAllegro",
        images: [{ data: RED_JPEG, mimeType: "image/jpeg" }],
      },
    ];
    made.faux.setResponses([
      (context) => {
        seen.push(context);
        return fauxAssistantMessage([fauxText("It is in G.")]);
      },
      (context) => {
        seen.push(context);
        return fauxAssistantMessage([fauxText("Sure.")]);
      },
    ]);
    await made.host.send("What key is this?", "tutor", "Song", [], "", attachments);
    const path = made.events.find((event) => event.type === "chat").path;
    const sent = playerMessages(seen[0])[0];
    const text = JSON.stringify(sent.content);
    assert.ok(text.includes("[Attached pdf: Score.pdf]"));
    assert.ok(text.includes("Allegro"));
    assert.ok(text.includes("What key is this?"));
    assert.ok(sent.content.some((part) => part.type === "image"));
    const kept = join(path.replace(/\.jsonl$/, ""), "Score.pdf");
    assert.ok(existsSync(kept));
    assert.equal(readFileSync(kept, "utf8"), "pdf bytes");

    // The next message has none.
    await made.host.send("Thanks", "tutor", "Song");
    const later = playerMessages(seen[1]);
    assert.ok(!JSON.stringify(later[later.length - 1]).includes("Attached"));

    await made.host.newChat();
    await made.host.openChat(path);
    const [first, second] = made.events[made.events.length - 1].turns;
    assert.equal(first.said, "What key is this?");
    assert.deepEqual(first.attachments, [{ kind: "pdf", name: "Score.pdf" }]);
    assert.deepEqual(second.attachments, []);
  } finally {
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});

test("a model that can't see pictures is told they were left out", () => {
  const files = [{ name: "a.png", kind: "image", text: "A picture.", images: [{ data: "x", mimeType: "image/jpeg" }] }];
  const blind = attachedBlock(files, false);
  assert.ok(blind.includes("can't see pictures"));
  assert.ok(!attachedBlock(files, true).includes("can't see pictures"));
  const said = `${blind}Look at this`;
  assert.equal(withoutJobLine(said), "Look at this");
  assert.deepEqual(attachedFiles(said), [{ kind: "image", name: "a.png" }]);
  assert.deepEqual(attachedFiles("no files"), []);
});

test("only chat files in the chats folder can be opened", async () => {
  const made = await fakeHost([]);
  try {
    const outside = [
      "/etc/passwd",
      "/tmp/chat.jsonl",
      join(made.agentDir, "Chats", "..", "auth.jsonl"),
      join(made.agentDir, "Chats", "notes.json"),
    ];
    for (const path of outside) {
      await made.host.openChat(path);
      assert.equal(made.events[made.events.length - 1].message, "That isn't one of Jello's chats.");
    }
    assert.equal(made.host.session, null);
  } finally {
    rmSync(made.agentDir, { recursive: true, force: true });
  }
});
