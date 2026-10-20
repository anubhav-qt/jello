// The start page: the laptop keyboard drawn on a stage of dots (ui/stage.ts).
// Keys play the synth: Rust decides what each key does.

import { invoke } from "@tauri-apps/api/core";
import { findElement, makeElement } from "./dom";
import { detectPlatform, type KeyLayout } from "./keyboard";
import { createStage, type Stage } from "./stage";

const page = findElement<HTMLElement>("#app");

// The page, once Rust has said what each key does.
let screen: Stage | null = null;
function showWave(wave: string) {
  if (screen !== null) {
    screen.showWave(wave);
  }
}

function showStatus(message: string) {
  if (screen !== null) {
    screen.showStatus(message);
  }
}

// Messages go to the engine one at a time, in order: each one waits for the
// one before it to finish. Otherwise, on a quick tap, letting go could
// overtake pressing and leave the note stuck on.
let lastMessage: Promise<void> = Promise.resolve();

function sendInOrder(send: () => Promise<void>) {
  lastMessage = lastMessage.then(send);
}

// The next six catch their own errors, so one failed message never stops
// the ones after it.

async function askForWave() {
  try {
    const newWave = await invoke<string>("current_wave");
    showWave(newWave);
  } catch (error) {
    showStatus(`Could not get the wave: ${error}`);
  }
}

// `key` is the key's place on the keyboard, like "KeyQ". Rust decides what it
// does, and answers with the wave new notes play.
async function pressKey(key: string) {
  try {
    const newWave = await invoke<string>("press_key", { key });
    showWave(newWave);
  } catch (error) {
    showStatus(`Could not play: ${error}`);
  }
}

async function releaseKey(key: string) {
  try {
    await invoke("release_key", { key });
  } catch (error) {
    showStatus(`Could not let go: ${error}`);
  }
}

async function releaseAllNotes() {
  try {
    await invoke("release_all_notes");
  } catch (error) {
    showStatus(`Could not let go: ${error}`);
  }
}

// The top bar's wave button does what the ` key does.
async function switchWave() {
  try {
    const newWave = await invoke<string>("switch_wave");
    showWave(newWave);
  } catch (error) {
    showStatus(`Could not switch the wave: ${error}`);
  }
}

async function renderWav() {
  try {
    const path = await invoke<string>("render_test_note");
    showStatus(`Saved ${path}`);
  } catch (error) {
    showStatus(`Could not render: ${error}`);
  }
}

// Lets go of every key at once, on screen and in the engine.
function letGoOfEverything() {
  if (screen !== null) {
    screen.releaseAll();
  }
  sendInOrder(releaseAllNotes);
}

window.addEventListener("keydown", (event) => {
  // While Cmd, Ctrl or Option/Alt is held, the key is a shortcut like Cmd+Q:
  // it plays nothing and is left alone. Every note is let go too, because on
  // a Mac a key let go while Cmd is held may never say so, and its note would
  // stay stuck on.
  if (event.metaKey || event.ctrlKey || event.altKey) {
    letGoOfEverything();
    return;
  }

  // Keep the webview from acting on the key as well: a Mac beeps at keys
  // nothing on the page uses, and Space or Enter would press the focused
  // button.
  event.preventDefault();

  // Holding a key down makes the system send it again and again, but its
  // note is already playing.
  if (event.repeat) {
    return;
  }

  // `event.code` is the key's place on the keyboard, like "KeyQ", whatever is
  // printed on it.
  const key = event.code;
  if (screen !== null) {
    screen.keyDown(key);
  }
  sendInOrder(() => pressKey(key));
});

// Every let-go goes to Rust, even while Cmd is held. Rust ignores keys
// that did not start a note.
window.addEventListener("keyup", (event) => {
  const key = event.code;
  if (screen !== null) {
    screen.keyUp(key);
  }
  sendInOrder(() => releaseKey(key));
});

// If the window loses focus (say, Cmd+Tab), let-gos may never reach it, so
// let go of every key right away.
window.addEventListener("blur", () => {
  letGoOfEverything();
});

// Asks Rust what each key does, then shows the page.
async function start() {
  let layout: KeyLayout;
  try {
    layout = await invoke<KeyLayout>("key_layout");
  } catch (error) {
    page.append(makeElement("p", "start-error", `Could not start: ${error}`));
    return;
  }

  screen = createStage({
    platform: detectPlatform(),
    layout,
    onWaveClick: () => sendInOrder(switchWave),
    onRenderClick: renderWav,
  });
  screen.show(page);
  sendInOrder(askForWave);
}

start();
