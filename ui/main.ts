// Dev panel placeholder: a button that plays middle C while it is held, and
// one that renders a held middle C to a WAV file. Not a design.

import { invoke } from "@tauri-apps/api/core";

// The button plays middle C at velocity 100. Velocity is how hard a key is
// hit, from 1 to 127.
const MIDDLE_C = 60;
const VELOCITY = 100;

function findElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) {
    throw new Error(`Missing element: ${selector}`);
  }
  return element;
}

const noteButton = findElement<HTMLButtonElement>("#play-note");
const renderButton = findElement<HTMLButtonElement>("#render-wav");
const status = findElement<HTMLParagraphElement>("#status");

function showStatus(message: string) {
  status.textContent = message;
}

// Note messages go to the engine one at a time, in order: each one waits for
// the one before it to finish. Otherwise, on a quick tap, letting go could
// overtake pressing and leave the note stuck on.
let lastNoteMessage: Promise<void> = Promise.resolve();

function sendInOrder(send: () => Promise<void>) {
  lastNoteMessage = lastNoteMessage.then(send);
}

// The next three catch their own errors, so one failed message never stops
// the ones after it.

async function playMiddleC() {
  try {
    await invoke("play_note", { note: MIDDLE_C, velocity: VELOCITY });
    // The sound shows it worked, so just clear any old message.
    showStatus("");
  } catch (error) {
    showStatus(`Could not play: ${error}`);
  }
}

async function releaseMiddleC() {
  try {
    await invoke("release_note", { note: MIDDLE_C });
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

// True while the button is held. It makes sure each press is let go of
// exactly once, even when the window loses focus first.
let held = false;

noteButton.addEventListener("pointerdown", (event) => {
  // Only the main button, so a right-click does nothing.
  if (event.button !== 0) {
    return;
  }
  // Keep hearing about this pointer even after it leaves the button, so
  // letting go anywhere still counts.
  noteButton.setPointerCapture(event.pointerId);
  held = true;
  sendInOrder(playMiddleC);
});

function letGoOfButton() {
  if (!held) {
    return;
  }
  held = false;
  sendInOrder(releaseMiddleC);
}

noteButton.addEventListener("pointerup", letGoOfButton);
// The webview can take a press over, for example to scroll on a touch screen.
// Then the press ends with a cancel instead of a let-go.
noteButton.addEventListener("pointercancel", letGoOfButton);

// If the window loses focus mid-hold (say, Cmd+Tab), the let-go may never
// reach it, so let go of every note right away.
window.addEventListener("blur", () => {
  if (!held) {
    return;
  }
  held = false;
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
