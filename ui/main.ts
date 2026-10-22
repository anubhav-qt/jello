// The start page: the laptop keyboard drawn on a stage of dots
// (ui/Stage.svelte). Keys play the synth: Rust decides what each key does.

import { invoke } from "@tauri-apps/api/core";
import { mount } from "svelte";
import type { ComposeView } from "./compose";
import { findElement, makeElement } from "./dom";
import { detectPlatform, type KeyLayout } from "./keyboard";
import type { FilterEnvelope, SoundView } from "./sound";
import Stage from "./Stage.svelte";

const page = findElement<HTMLElement>("#app");

// What ui/Stage.svelte lets main.ts do.
interface StageControls {
  // A key went down or came back up. `code` is its place, like "KeyQ".
  keyDown(code: string): void;
  keyUp(code: string): void;
  // Every key came up at once.
  releaseAll(): void;
  // The wave new notes play, the tempo and what the looper is doing.
  showSound(view: SoundView): void;
  // A message for the player, like where a file was saved.
  showStatus(message: string): void;
  // Whether the Compose tab is open.
  isComposing(): boolean;
  // What Compose shows, as Rust answered.
  showCompose(view: ComposeView): void;
}

// The page, once Rust has said what each key does.
let screen: StageControls | null = null;
function showSound(view: SoundView) {
  if (screen !== null) {
    screen.showSound(view);
  }
  // Rust sends a looper message once, like "There is no layer 7".
  if (view.looper.message !== null) {
    showStatus(view.looper.message);
  }
  keepWatchingLooper(view);
}

// While the looper is busy, what it shows changes on the beat with no key
// pressed, so the page asks Rust again this often.
const LOOPER_LOOK_MS = 50;

// Whether the next look is already on its way.
let lookPlanned = false;

function keepWatchingLooper(view: SoundView) {
  if (!view.looper.busy) {
    return;
  }
  if (lookPlanned) {
    return;
  }
  lookPlanned = true;
  window.setTimeout(() => {
    lookPlanned = false;
    sendInOrder(askForSound);
  }, LOOPER_LOOK_MS);
}

function showStatus(message: string) {
  if (screen !== null) {
    screen.showStatus(message);
  }
}

function showCompose(view: ComposeView) {
  if (screen !== null) {
    screen.showCompose(view);
  }
  keepWatchingSong(view);
}

// While the song plays, the playhead moves with no key pressed, so the page
// asks Rust again this often. In between, the tape moves the playhead on by
// its own clock.
const SONG_LOOK_MS = 50;

// Whether the next look at the song is already on its way.
let songLookPlanned = false;

function keepWatchingSong(view: ComposeView) {
  if (view.playing === null) {
    return;
  }
  if (songLookPlanned) {
    return;
  }
  songLookPlanned = true;
  window.setTimeout(() => {
    songLookPlanned = false;
    sendInOrder(askForCompose);
  }, SONG_LOOK_MS);
}

// Messages go to the engine one at a time, in order: each one waits for the
// one before it to finish. Otherwise, on a quick tap, letting go could
// overtake pressing and leave the note stuck on.
let lastMessage: Promise<void> = Promise.resolve();

function sendInOrder(send: () => Promise<void>) {
  lastMessage = lastMessage.then(send);
}

// The functions below catch their own errors, so one failed message never
// stops the ones after it.

async function askForSound() {
  try {
    const view = await invoke<SoundView>("sound_view");
    showSound(view);
  } catch (error) {
    showStatus(`Could not get the sound: ${error}`);
  }
}

// `key` is the key's place on the keyboard, like "KeyQ". Rust decides what it
// does, and answers with the wave new notes play, the tempo and what the
// looper is doing.
async function pressKey(key: string) {
  try {
    const view = await invoke<SoundView>("press_key", { key });
    showSound(view);
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

// The top bar's wave button does what the \ key does.
async function switchWave() {
  try {
    const view = await invoke<SoundView>("switch_wave");
    showSound(view);
  } catch (error) {
    showStatus(`Could not switch the wave: ${error}`);
  }
}

// The lane's snap switch turns snap on or off.
async function switchSnap() {
  try {
    const view = await invoke<SoundView>("switch_snap");
    showSound(view);
  } catch (error) {
    showStatus(`Could not switch snap: ${error}`);
  }
}

// A layer's volume knob sets its volume. `layer` counts from 1, and
// `volume` is in percent, from 0 to 100.
async function setLayerVolume(layer: number, volume: number) {
  try {
    const view = await invoke<SoundView>("set_layer_volume", { layer, volume });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the volume: ${error}`);
  }
}

// The filter's dev panel moves the filter. `cutoffHz` is middle C's cutoff,
// and `resonance` is in percent, from 0 to 100.
async function setFilter(cutoffHz: number, resonance: number) {
  try {
    const view = await invoke<SoundView>("set_filter", { cutoffHz, resonance });
    showSound(view);
  } catch (error) {
    showStatus(`Could not move the filter: ${error}`);
  }
}

// It also sets the filter envelope for the next notes.
async function setFilterEnvelope(envelope: FilterEnvelope) {
  try {
    const view = await invoke<SoundView>("set_filter_envelope", {
      amount: envelope.amount,
      attackMs: envelope.attackMs,
      decayMs: envelope.decayMs,
      sustain: envelope.sustain,
      releaseMs: envelope.releaseMs,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the filter envelope: ${error}`);
  }
}

async function askForCompose() {
  try {
    const view = await invoke<ComposeView>("compose_view");
    showCompose(view);
  } catch (error) {
    showStatus(`Could not open Compose: ${error}`);
  }
}

async function pickKey(tonic: number, mode: "major" | "minor") {
  try {
    const view = await invoke<ComposeView>("pick_key", { tonic, mode });
    showCompose(view);
  } catch (error) {
    showStatus(`Could not pick the key: ${error}`);
  }
}

// Adds the chord played last to the song, leaving out the ones tried before.
async function addLastChord() {
  try {
    const view = await invoke<ComposeView>("add_last_chord");
    showCompose(view);
  } catch (error) {
    showStatus(`Could not add the chord: ${error}`);
  }
}

async function removeLastChord() {
  try {
    const view = await invoke<ComposeView>("remove_last_chord");
    showCompose(view);
  } catch (error) {
    showStatus(`Could not remove the chord: ${error}`);
  }
}

// Plays the chord at `place` in the song, from 0, for as long as it was held.
async function playChordInSong(place: number) {
  try {
    const view = await invoke<ComposeView>("play_chord_in_song", { place });
    showCompose(view);
  } catch (error) {
    showStatus(`Could not play the chord: ${error}`);
  }
}

// Plays the whole song from its first chord.
async function playSong() {
  try {
    const view = await invoke<ComposeView>("play_song");
    showCompose(view);
  } catch (error) {
    showStatus(`Could not play the song: ${error}`);
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
  askForComposeAfterLetGo();
}

// A held chord only gets its length when it is let go, in the song or
// growing at its end, so in Compose the page asks again after every let-go.
function askForComposeAfterLetGo() {
  if (screen === null) {
    return;
  }
  if (!screen.isComposing()) {
    return;
  }
  sendInOrder(askForCompose);
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
  sendComposeKey(key);
});

// In Compose, Return adds the chord played last to the song and Delete
// takes the last one off. They go after the key's press, so a chord played
// just before is already known to Rust. After any other key, the page asks
// for Compose again, so a chord held shows at once, growing at the end.
function sendComposeKey(key: string) {
  if (screen === null) {
    return;
  }
  if (!screen.isComposing()) {
    return;
  }
  if (key === "Enter") {
    sendInOrder(addLastChord);
  } else if (key === "Backspace") {
    sendInOrder(removeLastChord);
  } else {
    sendInOrder(askForCompose);
  }
}

// Every let-go goes to Rust, even while Cmd is held. Rust ignores keys
// that did not start a note.
window.addEventListener("keyup", (event) => {
  const key = event.code;
  if (screen !== null) {
    screen.keyUp(key);
  }
  sendInOrder(() => releaseKey(key));
  askForComposeAfterLetGo();
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

  screen = mount(Stage, {
    target: page,
    props: {
      platform: detectPlatform(),
      layout,
      onWaveClick: () => sendInOrder(switchWave),
      onRenderClick: renderWav,
      onSnapClick: () => sendInOrder(switchSnap),
      onVolumeChange: (layer: number, volume: number) => sendInOrder(() => setLayerVolume(layer, volume)),
      onKeyPick: (tonic: number, mode: "major" | "minor") => sendInOrder(() => pickKey(tonic, mode)),
      onChordClick: (place: number) => sendInOrder(() => playChordInSong(place)),
      onPlayClick: () => sendInOrder(playSong),
      onFilterChange: (cutoffHz: number, resonance: number) => sendInOrder(() => setFilter(cutoffHz, resonance)),
      onFilterEnvelopeChange: (envelope: FilterEnvelope) => sendInOrder(() => setFilterEnvelope(envelope)),
    },
  });
  sendInOrder(askForSound);
  sendInOrder(askForCompose);
}

start();
