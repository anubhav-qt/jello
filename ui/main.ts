// Dev panel placeholder: plays the synth from the laptop keyboard, shows the
// wave new notes play, and renders a held middle C to a WAV file. Not a
// design.

import { invoke } from "@tauri-apps/api/core";

function findElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) {
    throw new Error(`Missing element: ${selector}`);
  }
  return element;
}

const waveLine = findElement<HTMLParagraphElement>("#wave");
const renderButton = findElement<HTMLButtonElement>("#render-wav");
const status = findElement<HTMLParagraphElement>("#status");

function showStatus(message: string) {
  status.textContent = message;
}

function showWave(wave: string) {
  waveLine.textContent = `Wave: ${wave}`;
}

// Messages go to the engine one at a time, in order: each one waits for the
// one before it to finish. Otherwise, on a quick tap, letting go could
// overtake pressing and leave the note stuck on.
let lastMessage: Promise<void> = Promise.resolve();

function sendInOrder(send: () => Promise<void>) {
  lastMessage = lastMessage.then(send);
}

// The next four catch their own errors, so one failed message never stops
// the ones after it.

async function askForWave() {
  try {
    const wave = await invoke<string>("current_wave");
    showWave(wave);
  } catch (error) {
    showStatus(`Could not get the wave: ${error}`);
  }
}

// `key` is the key's place on the keyboard, like "KeyQ". Rust decides what it
// does, and answers with the wave new notes play.
async function pressKey(key: string) {
  try {
    const wave = await invoke<string>("press_key", { key });
    showWave(wave);
    // The sound shows it worked, so just clear any old message.
    showStatus("");
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

window.addEventListener("keydown", (event) => {
  // While Cmd, Ctrl or Option/Alt is held, the key is a shortcut like Cmd+Q:
  // it plays nothing and is left alone. Every note is let go too, because on
  // a Mac a key let go while Cmd is held may never say so, and its note would
  // stay stuck on.
  if (event.metaKey || event.ctrlKey || event.altKey) {
    sendInOrder(releaseAllNotes);
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
  sendInOrder(() => pressKey(key));
});

// Every let-go goes to Rust, even while Cmd is held. Rust ignores keys that
// did not start a note.
window.addEventListener("keyup", (event) => {
  const key = event.code;
  sendInOrder(() => releaseKey(key));
});

// If the window loses focus (say, Cmd+Tab), let-gos may never reach it, so
// let go of every note right away.
window.addEventListener("blur", () => {
  sendInOrder(releaseAllNotes);
});

renderButton.addEventListener("click", async () => {
  try {
    const path = await invoke<string>("render_test_note");
    showStatus(`Saved ${path}`);
  } catch (error) {
    showStatus(`Could not render: ${error}`);
  }
});

// Show the wave as soon as the panel opens.
sendInOrder(askForWave);
