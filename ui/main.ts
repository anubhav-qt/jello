// Dev panel placeholder: plain buttons for the step 1 engine commands.
// Not a design.

import { invoke } from "@tauri-apps/api/core";

function findElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) {
    throw new Error(`Missing element: ${selector}`);
  }
  return element;
}

const playButton = findElement<HTMLButtonElement>("#play-tone");
const stopButton = findElement<HTMLButtonElement>("#stop-tone");
const renderButton = findElement<HTMLButtonElement>("#render-wav");
const status = findElement<HTMLParagraphElement>("#status");

function showStatus(message: string) {
  status.textContent = message;
}

playButton.addEventListener("click", async () => {
  try {
    await invoke("start_tone");
    showStatus("Playing the test tone.");
  } catch (error) {
    showStatus(`Could not play: ${error}`);
  }
});

stopButton.addEventListener("click", async () => {
  try {
    await invoke("stop_tone");
    showStatus("Stopped.");
  } catch (error) {
    showStatus(`Could not stop: ${error}`);
  }
});

renderButton.addEventListener("click", async () => {
  try {
    const path = await invoke<string>("render_test_tone");
    showStatus(`Saved ${path}`);
  } catch (error) {
    showStatus(`Could not render: ${error}`);
  }
});
