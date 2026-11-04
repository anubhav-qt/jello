// The start page: the laptop keyboard drawn on a stage of dots
// (ui/Stage.svelte). Keys play the synth: Rust decides what each key does.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mount } from "svelte";
import type { ComposeView, SectionEdit, SectionsView } from "./compose";
import { findElement, makeElement } from "./dom";
import { detectPlatform, type KeyLayout } from "./keyboard";
import type {
  ConnectionView,
  FilterEnvelope,
  InstrumentsView,
  LfoView,
  LoudnessEnvelope,
  Oscillator,
  PadView,
  PresetView,
  SoundSender,
  SoundView,
  Unison,
  Amp,
} from "./sound";
import type { SongSender, SongsView } from "./songs";
import type { PracticeMark, PracticeView } from "./practice";
import type { MappingSender, MappingView } from "./keys";
import type { Limiter, MixSender, MixView, Written } from "./mix";
import type { Tab } from "./tabs";
import type { PathView } from "./path";
import Stage from "./Stage.svelte";
import { JAM_PAUSE_MS } from "./jam";

const page = findElement<HTMLElement>("#app");

// What ui/Stage.svelte lets main.ts do.
interface StageControls {
  // A key went down or came back up. `code` is its place, like "KeyQ".
  keyDown(code: string): void;
  showLayout(layout: KeyLayout): void;
  closePanel(): boolean;
  showSoundModifier(held: boolean): void;
  keyUp(code: string): void;
  // Every key came up at once.
  releaseAll(): void;
  // The song's sound, the tempo and what the looper is doing.
  showSound(view: SoundView): void;
  // The song's mix, as Rust answered (docs/design.md#mix).
  showMix(view: MixView): void;
  // A message for the player, like where a file was saved.
  showStatus(message: string): void;
  // An arrow key steps the value picked in the sound panel, if any.
  stepPicked(steps: number): void;
  // Whether the Compose tab is open.
  isComposing(): boolean;
  wantsPerformance(): boolean;
  // What Compose shows, as Rust answered.
  showCompose(view: ComposeView): void;
  // The song's sections, as Rust answered.
  showSections(view: SectionsView): void;
  // The songs and the one open, as Rust answered.
  showSongs(view: SongsView): void;
  // The song's learning path, as Rust answered.
  showPath(view: PathView): void;
  // The agent pressed or let go of a key, playing a phrase.
  showAgentKey(code: string, down: boolean): void;
  // The agent marked keys with a keys card.
  showAgentMarks(keys: { key: string; label: string }[]): void;
  // Practice was set, changed or closed.
  showPractice(view: PracticeView): void;
  // The keys practice marks: the next to press and the ones after it.
  showPracticeMarks(marks: PracticeMark[]): void;
  // The keys for what the loop plays next.
  showComing(marks: PracticeMark[]): void;
  // Whether the mapping page takes the keys, and a key it took.
  keysPageWants(): boolean;
  keysPageKey(code: string): void;
  // The open take: right Command tapped flips it, Return keeps it, Esc
  // drops it. Each says whether there was one.
  flipTake(): boolean;
  keepTake(): boolean;
  dropTake(): boolean;
  // Right Command held talks into the rail.
  talkStarts(): void;
  // A pause in playing, for the jam card.
  jamPaused(): void;
  talkEnds(): void;
}

// The page, once Rust has said what each key does.
let screen: StageControls | null = null;
function showSound(view: SoundView) {
  if (screen !== null) {
    screen.showSound(view);
  }
  askForLayoutIfGuitarChanged(view);
  askForSectionsIfLoopChanged(view);
  // Rust sends a looper message once, like "There is no layer 7".
  if (view.looper.message !== null) {
    showStatus(view.looper.message);
  }
  keepWatchingLooper(view);
}

// Whether the picked track plays a string, as the sound said last. It makes the
// keys a guitar, so the page asks for the keys again when that changes.
let playsAString = false;

function askForLayoutIfGuitarChanged(view: SoundView) {
  let now = false;
  for (const oscillator of view.oscillators) {
    if (oscillator.kind === "string" && oscillator.level > 0) {
      now = true;
    }
  }
  if (now === playsAString) {
    return;
  }
  playsAString = now;
  void askForLayout();
}

// While the looper is busy, what it shows changes on the beat with no key
// pressed, so the page asks Rust again this often.
const LOOPER_LOOK_MS = 50;

// Whether the next look is already on its way.
let lookPlanned = false;

function keepWatchingLooper(view: SoundView) {
  if (!view.looper.busy) {
    clearComing();
    return;
  }
  if (screen?.isComposing()) {
    sendInOrder(askForComing);
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

// Whether any key is lit for what comes next, so stopping clears them once.
let comingShown = false;

function clearComing() {
  if (comingShown) {
    comingShown = false;
    screen?.showComing([]);
  }
}

async function askForComing() {
  try {
    const marks = await invoke<PracticeMark[]>("coming_keys");
    comingShown = marks.length > 0;
    screen?.showComing(marks);
  } catch (error) {
    showStatus(`Could not see what comes next: ${error}`);
  }
}

function showSongs(view: SongsView) {
  if (screen !== null) {
    screen.showSongs(view);
  }
}

function showStatus(message: string) {
  if (screen !== null) {
    screen.showStatus(message);
  }
}

// How many chords the song had at the last answer, so the path, which
// counts them, is asked again only when that changes.
let chordCountBefore = -1;

function showCompose(view: ComposeView) {
  if (screen !== null) {
    screen.showCompose(view);
  }
  keepWatchingSong(view);
  if (view.progression.length !== chordCountBefore) {
    chordCountBefore = view.progression.length;
    sendInOrder(askForPath);
  }
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
// does, and answers with the song's sound, the tempo and what the looper is
// doing.
// Every press starts the jam pause over; once it runs out, the rail's jam
// card shows (docs/design.md#ai-and-learning-rethought-2026-11-03).
let jamTimer = 0;

function startJamPause() {
  window.clearTimeout(jamTimer);
  jamTimer = window.setTimeout(() => {
    screen?.jamPaused();
  }, JAM_PAUSE_MS);
}

async function pressKey(key: string) {
  startJamPause();
  try {
    const view = await invoke<SoundView>("press_key", { key });
    showSound(view);
  } catch (error) {
    showStatus(`Could not play: ${error}`);
  }
  if (drumModeKeys.includes(key) || guitarShapeKeys.includes(key)) {
    await askForLayout();
  }
}

// The keys that switch drum mode on and off, as Rust said last. Switching
// changes what the letters do, so the page asks for the keys again.
let drumModeKeys: string[] = [];

// A guitar's shape keys, as Rust said last. Holding one changes what the
// strings play, so the page asks for the keys again as it goes down and up.
let guitarShapeKeys: string[] = [];

function showLayout(layout: KeyLayout) {
  drumModeKeys = layout.drum_mode_keys;
  guitarShapeKeys = layout.guitar_shapes;
  screen?.showLayout(layout);
}

async function askForLayout() {
  try {
    const layout = await invoke<KeyLayout>("key_layout");
    showLayout(layout);
  } catch (error) {
    showStatus(`Could not get the keys: ${error}`);
  }
}

async function releaseKey(key: string) {
  try {
    await invoke("release_key", { key });
  } catch (error) {
    showStatus(`Could not let go: ${error}`);
  }
  if (guitarShapeKeys.includes(key)) {
    await askForLayout();
  }
}

async function releaseAllNotes() {
  try {
    await invoke("release_all_notes");
  } catch (error) {
    showStatus(`Could not let go: ${error}`);
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

// A layer's button pauses it or plays it again. `layer` counts from 1.
async function setLayerPlaying(layer: number, playing: boolean) {
  try {
    const view = await invoke<SoundView>("set_layer_playing", {
      layer,
      playing,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not switch layer ${layer}: ${error}`);
  }
}

// The lane's loop button pauses the whole loop, or plays it from beat 1.
async function setLoopPlaying(playing: boolean) {
  let command = "pause_loop";
  if (playing) {
    command = "play_loop";
  }
  try {
    const view = await invoke<SoundView>(command);
    showSound(view);
  } catch (error) {
    showStatus(`Could not switch the loop: ${error}`);
  }
}

// The pointer transport follows the same ordered path as Right Shift.
async function recordLoop() {
  try {
    const current = await invoke<SoundView>("sound_view");
    if (current.looper.state === "paused") {
      await setLoopPlaying(true);
    }
    await pressKey("ShiftRight");
    await releaseKey("ShiftRight");
  } catch (error) {
    showStatus(`Could not record: ${error}`);
  }
}

async function setTempo(tempo: number) {
  try {
    showSound(await invoke<SoundView>("set_tempo", { tempo }));
  } catch (error) {
    showStatus(`Could not set tempo: ${error}`);
  }
}

async function setLayerBars(layer: number, bars: number) {
  try {
    showSound(await invoke<SoundView>("set_layer_bars", { layer, bars }));
  } catch (error) {
    showStatus(`Could not set layer length: ${error}`);
  }
}

// The sound panel's Oscillators page sets oscillator `index`, from 0, for
// the next notes.
async function setOscillator(index: number, oscillator: Oscillator) {
  try {
    const view = await invoke<SoundView>("set_oscillator", {
      index,
      kind: oscillator.kind,
      level: oscillator.level,
      octaves: oscillator.octaves,
      halfSteps: oscillator.halfSteps,
      cents: oscillator.cents,
      width: oscillator.width,
      touch: oscillator.touch,
      string: oscillator.string,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set oscillator ${index + 1}: ${error}`);
  }
}

// The sound panel sets the loudness envelope for the next notes.
async function setLoudnessEnvelope(envelope: LoudnessEnvelope) {
  try {
    const view = await invoke<SoundView>("set_loudness_envelope", {
      attackMs: envelope.attackMs,
      decayMs: envelope.decayMs,
      sustain: envelope.sustain,
      releaseMs: envelope.releaseMs,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the loudness envelope: ${error}`);
  }
}

// It moves the filter. `filterType` is "low-pass", "high-pass", "band-pass"
// or "notch", `cutoffHz` is middle C's cutoff, and `resonance` is in
// percent, from 0 to 100.
async function setFilter(
  filterType: string,
  cutoffHz: number,
  resonance: number,
) {
  try {
    const view = await invoke<SoundView>("set_filter", {
      filterType,
      cutoffHz,
      resonance,
    });
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

// It sets key tracking for the next notes, in percent, from 0 to 100.
async function setKeyTracking(keyTracking: number) {
  try {
    const view = await invoke<SoundView>("set_key_tracking", { keyTracking });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set key tracking: ${error}`);
  }
}

// It tunes the next notes from A4 at `a4Hz`.
async function setTuning(a4Hz: number) {
  try {
    const view = await invoke<SoundView>("set_tuning", { a4Hz });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the tuning: ${error}`);
  }
}

// And it sets the amp, at once.
async function setAmp(amp: Amp) {
  try {
    const view = await invoke<SoundView>("set_amp", { amp });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the amp: ${error}`);
  }
}

// And it sets unison for the next notes: how many copies, how far apart
// they are tuned in cents, and how far apart they spread in percent.
async function setUnison(unison: Unison) {
  try {
    const view = await invoke<SoundView>("set_unison", {
      copies: unison.copies,
      detuneCents: unison.detuneCents,
      spread: unison.spread,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set unison: ${error}`);
  }
}

// It sets how long the vibrato key takes to rise when held and fall when
// let go, in ms.
async function setVibratoKeyTimes(riseMs: number, fallMs: number) {
  try {
    const view = await invoke<SoundView>("set_vibrato_key_times", {
      riseMs,
      fallMs,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the vibrato key: ${error}`);
  }
}

// It sets how long each melody note takes to glide while the glide key is
// held, in ms.
async function setGlideTime(glideMs: number) {
  try {
    const view = await invoke<SoundView>("set_glide_time", { glideMs });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the glide time: ${error}`);
  }
}

// It sets LFO `index`, from 0, which moves notes already sounding.
async function setLfo(index: number, lfo: LfoView) {
  try {
    const view = await invoke<SoundView>("set_lfo", { index, lfo });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set LFO ${index + 1}: ${error}`);
  }
}

// And it sets connection `index` of the routing list, from 0.
async function setConnection(index: number, connection: ConnectionView) {
  try {
    const view = await invoke<SoundView>("set_connection", {
      index,
      connection,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the connection: ${error}`);
  }
}

// It lists the presets in Music/Jello/Sounds.
async function listPresets(): Promise<PresetView[]> {
  try {
    return await invoke<PresetView[]>("preset_list");
  } catch (error) {
    showStatus(`Could not list the presets: ${error}`);
    return [];
  }
}

// It copies preset `name` into the song.
async function loadPreset(name: string) {
  try {
    const view = await invoke<SoundView>("load_preset", { name });
    showSound(view);
  } catch (error) {
    showStatus(`Could not load ${name}: ${error}`);
  }
}

// It saves the song's sound as preset `name`.
async function savePreset(name: string) {
  try {
    const view = await invoke<SoundView>("save_preset", { name });
    showSound(view);
  } catch (error) {
    showStatus(`Could not save ${name}: ${error}`);
  }
}

async function presetNameProblem(name: string): Promise<string | null> {
  try {
    return await invoke<string | null>("preset_name_problem", { name });
  } catch (error) {
    return String(error);
  }
}

async function markStartingPreset(name: string | null) {
  try {
    await invoke("mark_starting_preset", { name });
  } catch (error) {
    showStatus(`Could not mark the starting sound: ${error}`);
  }
}

// It picks the instrument oscillator `index`, from 0, plays as a sample
// (docs/design.md#sampler).
async function setInstrument(index: number, name: string | null) {
  try {
    const view = await invoke<SoundView>("set_instrument", { index, name });
    showSound(view);
  } catch (error) {
    showStatus(`Could not pick the instrument: ${error}`);
  }
}

// The Drums page picks the kit...
async function setDrumKit(name: string | null) {
  try {
    const view = await invoke<SoundView>("set_drum_kit", { name });
    showSound(view);
  } catch (error) {
    showStatus(`Could not pick the drum kit: ${error}`);
  }
  // A kit makes the track a drums track, so the letters hit drums.
  await askForLayout();
}

// Picks track `track`, counted from 1, or makes the next one
// (docs/design.md#tracks-and-sections). Drum mode follows the track, so the
// page asks for the keys again.
async function pickTrack(track: number) {
  try {
    showSound(await invoke<SoundView>("pick_track", { track }));
  } catch (error) {
    showStatus(`Could not pick track ${track}: ${error}`);
  }
  await askForLayout();
}

// The track row's other changes, each counted from 1.
async function renameTrack(track: number, name: string) {
  try {
    showSound(await invoke<SoundView>("rename_track", { track, name }));
  } catch (error) {
    showStatus(`Could not rename track ${track}: ${error}`);
  }
}

async function setTrackVolume(track: number, volume: number) {
  try {
    showSound(await invoke<SoundView>("set_track_volume", { track, volume }));
  } catch (error) {
    showStatus(`Could not set track ${track}'s volume: ${error}`);
  }
}

async function removeTrack(track: number) {
  try {
    showSound(await invoke<SoundView>("remove_track", { track }));
  } catch (error) {
    showStatus(`Could not remove track ${track}: ${error}`);
  }
  await askForLayout();
}

// The track a number key picks with left Cmd (left Alt on Windows): Digit1
// picks track 1, up to Digit9. Null for any other key.
function trackNumberOf(key: string): number | null {
  const match = /^Digit([1-9])$/.exec(key);
  if (match === null) {
    return null;
  }
  return Number(match[1]);
}

// ...and sets pad `pad`, from 0.
async function setDrumPad(pad: number, settings: PadView) {
  try {
    const view = await invoke<SoundView>("set_drum_pad", {
      pad,
      level: settings.level,
      halfSteps: settings.half_steps,
      pan: settings.pan,
      group: settings.group,
    });
    showSound(view);
  } catch (error) {
    showStatus(`Could not set the ${settings.name}: ${error}`);
  }
}

// The instruments in Music/Jello/Instruments, and the starters.
async function listInstruments(): Promise<InstrumentsView | null> {
  try {
    return await invoke<InstrumentsView>("list_instruments");
  } catch (error) {
    showStatus(`Could not list the instruments: ${error}`);
    return null;
  }
}

async function openInstrumentsFolder() {
  try {
    await invoke("open_instruments_folder");
  } catch (error) {
    showStatus(`Could not open the Instruments folder: ${error}`);
  }
}

// Fetches starter `name`. It takes a while, so it is not sent in order
// with the keys, which keep playing.
async function downloadStarter(name: string) {
  try {
    await invoke("download_starter", { name });
    showStatus(`${name} is in the Instruments folder`);
  } catch (error) {
    showStatus(`Could not download ${name}: ${error}`);
  }
}

// The songs in Music/Jello/Songs and the one open
// (docs/adr/0038-song-files.md).
async function askForSongs(): Promise<SongsView | null> {
  try {
    const view = await invoke<SongsView>("song_list");
    showSongs(view);
    return view;
  } catch (error) {
    showStatus(`Could not list the songs: ${error}`);
    return null;
  }
}

// Opening or making a song changes everything, so the page asks again for
// the sound and Compose.
async function openSong(name: string) {
  try {
    const view = await invoke<SongsView>("open_song", { name });
    showSongs(view);
  } catch (error) {
    showStatus(`Could not open ${name}: ${error}`);
  }
  await askForSound();
  await askForCompose();
  await askForMix();
  await askForPath();
}

async function makeNewSong() {
  try {
    const view = await invoke<SongsView>("make_new_song");
    showSongs(view);
  } catch (error) {
    showStatus(`Could not make a new song: ${error}`);
  }
  await askForSound();
  await askForCompose();
  await askForMix();
  await askForPath();
}

async function renameSong(name: string) {
  try {
    const view = await invoke<SongsView>("rename_song", { name });
    showSongs(view);
  } catch (error) {
    showStatus(`Could not rename the song: ${error}`);
  }
  await askForPath();
}

async function songNameProblem(name: string): Promise<string | null> {
  try {
    return await invoke<string | null>("song_name_problem", { name });
  } catch (error) {
    return String(error);
  }
}

// Sends `send` in order with the keys, like `sendInOrder`, and is done once
// it has been sent and answered.
function sendInOrderAndWait(send: () => Promise<void>): Promise<void> {
  return new Promise((done) => {
    sendInOrder(async () => {
      await send();
      done();
    });
  });
}

async function askForCompose() {
  try {
    const view = await invoke<ComposeView>("compose_view");
    showCompose(view);
  } catch (error) {
    showStatus(`Could not open Compose: ${error}`);
  }
}

async function pickScale(tonic: number, kind: string) {
  try {
    const view = await invoke<ComposeView>("pick_song_scale", { tonic, kind });
    showCompose(view);
    await askForLayout();
  } catch (error) {
    showStatus(`Could not pick the scale: ${error}`);
  }
}

async function pickPlayScale(tonic: number, kind: string) {
  try {
    const layout = await invoke<KeyLayout>("pick_play_scale", { tonic, kind });
    showLayout(layout);
  } catch (error) {
    showStatus(`Could not pick the scale: ${error}`);
  }
}

// Play has keys of its own; every other tab plays the song's.
async function tabChanged(tab: Tab) {
  try {
    const layout = await invoke<KeyLayout>("keys_tab_changed", { onPlay: tab === "play" });
    showLayout(layout);
    await askForCompose();
  } catch (error) {
    showStatus(`Could not change the keys: ${error}`);
  }
}

// Asks Rust for a change to the mapping page, in order with the keys. A refusal
// shows in the top bar. The keys may play something else after.
function askMapping(command: string, args: Record<string, unknown>): Promise<MappingView | null> {
  return new Promise((done) => {
    sendInOrder(async () => {
      try {
        const view = await invoke<MappingView>(command, args);
        done(view);
        if (command !== "mapping_view") {
          await askForLayout();
          await askForCompose();
        }
      } catch (error) {
        showStatus(`Could not change the keys: ${error}`);
        done(null);
      }
    });
  });
}

const mappingSender: MappingSender = {
  look: (section) => askMapping("mapping_view", { section }),
  setNotes: (section, key, notes) => askMapping("set_key_notes", { section, key, notes }),
  setLocked: (section, key, locked) => askMapping("set_key_locked", { section, key, locked }),
  setGuitar: (section, key, strings) => askMapping("set_guitar_strings", { section, key, name: "", strings }),
  reset: (section) => askMapping("reset_keys", { section }),
  soundNote: (note, on) =>
    sendInOrder(async () => {
      try {
        await invoke("sound_mapped_note", { note, on });
      } catch (error) {
        showStatus(`Could not play that note: ${error}`);
      }
    }),
};

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
    const path = await invoke<string>("render_wav");
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
  if (!screen.wantsPerformance()) {
    return;
  }
  sendInOrder(askForCompose);
}

// With left Command (left Alt on Windows), arrows edit the picked sound
// value. Bare arrows shift every note and chord by whole octaves.
const ARROW_STEPS: Record<string, number> = {
  ArrowLeft: -1,
  ArrowRight: 1,
  ArrowDown: -10,
  ArrowUp: 10,
};

// An arrow key went down. Held, the system sends it again and again, and
// each time it steps again, but the drawn key only goes down once.
function stepWithArrow(key: string, steps: number, again: boolean) {
  if (screen === null) {
    return;
  }
  if (!again) {
    screen.keyDown(key);
  }
  screen.stepPicked(steps);
}

// Whether the key went to a text field, like the Presets page's name field.
function isTyping(event: KeyboardEvent): boolean {
  const target = event.target;
  if (target instanceof HTMLInputElement) {
    return true;
  }
  if (target instanceof HTMLTextAreaElement) {
    return true;
  }
  return false;
}

// While typing in a text field, keys go to the field and play nothing.
// Esc and Return only leave the field, so keys play again.
function typeInField(event: KeyboardEvent) {
  if (
    event.code === "Escape" ||
    event.code === "Enter" ||
    event.code === "NumpadEnter"
  ) {
    event.preventDefault();
    const field = event.target as HTMLElement;
    field.blur();
  }
}

// Right Command (docs/design.md#ai-and-learning-rethought-2026-11-03): a
// tap flips the open take, holding it talks into the rail. Windows has no
// free key for it yet.
const TALK_KEY = detectPlatform() === "mac" ? "MetaRight" : null;
// How long right Command is held before it talks, in ms.
const TALK_AFTER_MS = 350;
let talkKeyDown = false;
let talking = false;
// Another key went down with right Command, so it was a shortcut.
let talkKeyUsed = false;
let talkTimer = 0;

function talkKeyGoesDown() {
  talkKeyDown = true;
  talkKeyUsed = false;
  talking = false;
  letGoOfEverything();
  talkTimer = window.setTimeout(() => {
    if (talkKeyDown && !talkKeyUsed) {
      talking = true;
      screen?.talkStarts();
    }
  }, TALK_AFTER_MS);
}

function talkKeyComesUp() {
  window.clearTimeout(talkTimer);
  const wasTalking = talking;
  const wasUsed = talkKeyUsed;
  talkKeyDown = false;
  talking = false;
  if (wasTalking) {
    screen?.talkEnds();
    return;
  }
  if (!wasUsed) {
    screen?.flipTake();
  }
}

function isReturn(code: string): boolean {
  if (code === "Enter") {
    return true;
  }
  return code === "NumpadEnter";
}

const octaveSteps: Record<string, number> = {
  ArrowUp: 1,
  ArrowDown: -1,
  ArrowRight: 2,
  ArrowLeft: -2,
};
let soundModifierHeld = false;
const soundModifier = detectPlatform() === "mac" ? "MetaLeft" : "AltLeft";

async function shiftOctave(steps: number) {
  try {
    const layout = await invoke<KeyLayout>("shift_octave", { steps });
    showLayout(layout);
    await askForCompose();
  } catch (error) {
    showStatus(`Could not shift the octave: ${error}`);
  }
}

// The sections lane shows each track's layers, so the page asks for the
// sections again whenever the loop's shape changes: a layer recorded,
// removed, paused or played, another track picked, or the tempo.
let loopShapeBefore = "";

function askForSectionsIfLoopChanged(view: SoundView) {
  const playing = view.looper.layers.map((layer) => layer.playing);
  const parts = [
    view.picked_track,
    view.tracks.length,
    view.looper.state,
    view.looper.layers.length,
    playing.join(","),
    view.tempo,
  ];
  const shape = parts.join(" ");
  if (shape === loopShapeBefore) {
    return;
  }
  loopShapeBefore = shape;
  sendInOrder(askForSections);
}

async function askForSections() {
  try {
    const view = await invoke<SectionsView>("sections_view");
    screen?.showSections(view);
  } catch (error) {
    showStatus(`Could not get the sections: ${error}`);
  }
  // The path counts the song's sections and tracks, so it is asked with
  // them.
  await askForPath();
}

async function askForPath() {
  try {
    const view = await invoke<PathView>("path_view");
    screen?.showPath(view);
  } catch (error) {
    showStatus(`Could not get the learning path: ${error}`);
  }
}

async function editSections(edit: SectionEdit) {
  try {
    const view = await invoke<SectionsView>("edit_sections", { edit });
    screen?.showSections(view);
  } catch (error) {
    showStatus(`Could not change the sections: ${error}`);
  }
  // Opening a section or playing them changes the looper and the tempo.
  await askForSound();
}

// Left Cmd + Return saves the layers playing now as a section, and left
// Cmd + Delete takes the last one off (docs/design.md#tracks-and-sections).
function sectionEditOf(code: string): SectionEdit | null {
  if (code === "Enter") {
    return { action: "save" };
  }
  if (code === "NumpadEnter") {
    return { action: "save" };
  }
  if (code === "Backspace") {
    return { action: "remove_last" };
  }
  return null;
}

async function editComposition(action: string, place: number) {
  try {
    showCompose(
      await invoke<ComposeView>("edit_composition", { action, place }),
    );
  } catch (error) {
    showStatus(`Could not edit the song: ${error}`);
  }
}

window.addEventListener("keydown", (event) => {
  if (event.code === TALK_KEY) {
    event.preventDefault();
    if (!event.repeat) {
      talkKeyGoesDown();
    }
    return;
  }
  if (talkKeyDown) {
    talkKeyUsed = true;
  }
  if (event.code === soundModifier) {
    soundModifierHeld = true;
    if (soundModifier === "AltLeft" && !isTyping(event)) {
      event.preventDefault();
    }
    screen?.showSoundModifier(true);
    return;
  }
  if (
    soundModifierHeld &&
    ARROW_STEPS[event.code] !== undefined &&
    !isTyping(event)
  ) {
    event.preventDefault();
    stepWithArrow(event.code, ARROW_STEPS[event.code], event.repeat);
    return;
  }
  // Left Cmd and a number picks that track, or makes the next one.
  const trackNumber = trackNumberOf(event.code);
  if (soundModifierHeld && trackNumber !== null && !isTyping(event)) {
    event.preventDefault();
    if (!event.repeat) {
      sendInOrder(() => pickTrack(trackNumber));
    }
    return;
  }
  const sectionEdit = sectionEditOf(event.code);
  if (soundModifierHeld && sectionEdit !== null && !isTyping(event)) {
    event.preventDefault();
    if (!event.repeat) {
      sendInOrder(() => editSections(sectionEdit));
    }
    return;
  }
  // With a take open, Esc drops it and Return keeps it.
  if (!isTyping(event) && !event.metaKey && !event.repeat) {
    if (event.code === "Escape" && screen?.dropTake()) {
      event.preventDefault();
      return;
    }
    if (isReturn(event.code) && screen?.keepTake()) {
      event.preventDefault();
      return;
    }
  }
  if (event.code === "Escape" && screen?.closePanel()) {
    event.preventDefault();
    if (event.target instanceof HTMLElement) {
      event.target.blur();
    }
    return;
  }
  // While Cmd is held, the key is a shortcut like Cmd+Q: it plays nothing
  // and is left alone. Every note is let go too, because on a Mac a key let
  // go while Cmd is held may never say so, and its note would stay stuck
  // on. Left Control plays F#m and left Option holds the glide key, so
  // neither Ctrl nor Option/Alt is a shortcut key
  // (docs/adr/0032-the-vibrato-key.md, docs/adr/0036-glide.md).
  if (event.metaKey) {
    letGoOfEverything();
    return;
  }

  if (isTyping(event)) {
    typeInField(event);
    return;
  }

  // Keep the webview from acting on the key as well: a Mac beeps at keys
  // nothing on the page uses, and Space or Enter would press the focused
  // button.
  event.preventDefault();

  // The mapping page edits the key pressed instead of playing it.
  if (screen?.keysPageWants()) {
    if (!event.repeat) {
      screen.keysPageKey(event.code);
    }
    return;
  }

  // `event.code` is the key's place on the keyboard, like "KeyQ", whatever is
  // printed on it.
  const key = event.code;
  const octaves = octaveSteps[key];
  if (octaves !== undefined) {
    if (!event.repeat) {
      screen?.keyDown(key);
      sendInOrder(() => shiftOctave(octaves));
    }
    return;
  }

  // Holding a key down makes the system send it again and again, but its
  // note is already playing.
  if (event.repeat) {
    return;
  }

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
    if (screen.wantsPerformance()) {
      sendInOrder(askForCompose);
    }
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

// Every let-go but an arrow key's goes to Rust, even while Cmd is held. Rust
// ignores keys that did not start a note.
window.addEventListener("keyup", (event) => {
  const key = event.code;
  if (key === TALK_KEY) {
    talkKeyComesUp();
    return;
  }
  if (key === soundModifier) {
    soundModifierHeld = false;
    screen?.showSoundModifier(false);
    // macOS can omit other key-up events while Command is held. Keep
    // notes sounding during sound edits, then clear any missed releases.
    if (soundModifier === "MetaLeft") {
      letGoOfEverything();
    }
  }
  if (screen !== null) {
    screen.keyUp(key);
  }
  if (ARROW_STEPS[key] !== undefined) {
    return;
  }
  sendInOrder(() => releaseKey(key));
  askForComposeAfterLetGo();
});

// If the window loses focus (say, Cmd+Tab), let-gos may never reach it, so
// let go of every key right away.
window.addEventListener("blur", () => {
  if (talkKeyDown) {
    window.clearTimeout(talkTimer);
    talkKeyDown = false;
    if (talking) {
      talking = false;
      screen?.talkEnds();
    }
  }
  soundModifierHeld = false;
  screen?.showSoundModifier(false);
  letGoOfEverything();
});

// The Mix panel's messages (src-tauri/src/mix_commands.rs): each answers
// with the whole mix.

async function askForMix() {
  try {
    const view = await invoke<MixView>("mix_view");
    screen?.showMix(view);
  } catch (error) {
    showStatus(`Could not get the mix: ${error}`);
  }
}

// Sends `command` with `args` and shows the mix it answers with, or says
// why it couldn't.
async function changeMix(command: string, args: Record<string, unknown>) {
  try {
    const view = await invoke<MixView>(command, args);
    screen?.showMix(view);
  } catch (error) {
    showStatus(`Could not change the mix: ${error}`);
  }
}

// What the Mix panel sends, each in order with the keys.
const mixSender: MixSender = {
  setEffect: (place: string, slot: number, kind: string) =>
    sendInOrder(() => changeMix("set_mix_effect", { place, slot, kind })),
  moveEffect: (place: string, from: number, to: number) =>
    sendInOrder(() => changeMix("move_mix_effect", { place, from, to })),
  setEffectOn: (place: string, slot: number, on: boolean) =>
    sendInOrder(() => changeMix("set_mix_effect_on", { place, slot, on })),
  setEffectValue: (place: string, slot: number, word: string, value: Written) =>
    sendInOrder(() => changeMix("set_mix_effect_value", { place, slot, word, value })),
  setPan: (place: string, pan: number) =>
    sendInOrder(() => changeMix("set_mix_pan", { place, pan })),
  setSend: (track: number, bus: number, amount: number) =>
    sendInOrder(() => changeMix("set_mix_send", { track, bus, amount })),
  setBusVolume: (bus: number, volume: number) =>
    sendInOrder(() => changeMix("set_bus_volume", { bus, volume })),
  addBus: () => sendInOrder(() => changeMix("add_bus", {})),
  removeBus: (bus: number) => sendInOrder(() => changeMix("remove_bus", { bus })),
  renameBus: (bus: number, name: string) =>
    sendInOrder(() => changeMix("rename_bus", { bus, name })),
  setLimiter: (limiter: Limiter) =>
    sendInOrder(() => changeMix("set_limiter", { limiter })),
  refresh: () => sendInOrder(askForMix),
};

// What the sound panel sends, each in order with the keys.
const soundSender: SoundSender = {
  setOscillator: (index, oscillator) =>
    sendInOrder(() => setOscillator(index, oscillator)),
  setLoudnessEnvelope: (envelope) =>
    sendInOrder(() => setLoudnessEnvelope(envelope)),
  setFilter: (filterType, cutoffHz, resonance) =>
    sendInOrder(() => setFilter(filterType, cutoffHz, resonance)),
  setFilterEnvelope: (envelope) =>
    sendInOrder(() => setFilterEnvelope(envelope)),
  setKeyTracking: (keyTracking) =>
    sendInOrder(() => setKeyTracking(keyTracking)),
  setUnison: (unison) => sendInOrder(() => setUnison(unison)),
  setAmp: (amp) => sendInOrder(() => setAmp(amp)),
  setTuning: (a4Hz) => sendInOrder(() => setTuning(a4Hz)),
  setVibratoKeyTimes: (riseMs, fallMs) =>
    sendInOrder(() => setVibratoKeyTimes(riseMs, fallMs)),
  setGlideTime: (glideMs) => sendInOrder(() => setGlideTime(glideMs)),
  setLfo: (index, lfo) => sendInOrder(() => setLfo(index, lfo)),
  setConnection: (index, connection) =>
    sendInOrder(() => setConnection(index, connection)),
  listPresets: () => listPresets(),
  loadPreset: (name) => sendInOrder(() => loadPreset(name)),
  savePreset: (name) => sendInOrderAndWait(() => savePreset(name)),
  presetNameProblem: (name) => presetNameProblem(name),
  markStartingPreset: (name) => markStartingPreset(name),
  setInstrument: (index, name) => sendInOrder(() => setInstrument(index, name)),
  setDrumKit: (name) => sendInOrder(() => setDrumKit(name)),
  setDrumPad: (pad, settings) => sendInOrder(() => setDrumPad(pad, settings)),
  listInstruments: () => listInstruments(),
  openInstrumentsFolder: () => openInstrumentsFolder(),
  downloadStarter: (name) => downloadStarter(name),
  pickTrack: (track) => sendInOrder(() => pickTrack(track)),
  renameTrack: (track, name) => sendInOrder(() => renameTrack(track, name)),
  setTrackVolume: (track, volume) =>
    sendInOrder(() => setTrackVolume(track, volume)),
  removeTrack: (track) => sendInOrder(() => removeTrack(track)),
};

// What the Songs panel sends, each in order with the keys.
const songSender: SongSender = {
  listSongs: () => listSongsInOrder(),
  openSong: (name) => sendInOrderAndWait(() => openSong(name)),
  newSong: () => sendInOrderAndWait(makeNewSong),
  renameSong: (name) => sendInOrderAndWait(() => renameSong(name)),
  songNameProblem: (name) => songNameProblem(name),
};

// Lists the songs in order with the keys, and is done with the list.
function listSongsInOrder(): Promise<SongsView | null> {
  return new Promise((done) => {
    sendInOrder(async () => {
      const view = await askForSongs();
      done(view);
    });
  });
}

// The agent changed something, or its Undo put the song back: the page
// asks Rust again for everything it shows.
function redrawAfterAgent() {
  sendInOrder(askForSound);
  sendInOrder(askForMix);
  sendInOrder(askForCompose);
  sendInOrder(askForSections);
  sendInOrder(async () => {
    await askForSongs();
  });
  sendInOrder(askForLayout);
  sendInOrder(async () => {
    try {
      const practice = await invoke<PracticeView>("practice_view");
      screen?.showPractice(practice);
    } catch {
      // Practice shows once the Tutor sets a piece.
    }
  });
}

// The agent's tools tell the page as they act (src-tauri/src/tool_server.rs).
function hearTheAgent() {
  listen<{ key: string; down: boolean }>("agent-key", (event) => {
    screen?.showAgentKey(event.payload.key, event.payload.down);
  });
  listen<{ keys: { key: string; label: string }[] }>("agent-marks", (event) => {
    screen?.showAgentMarks(event.payload.keys);
  });
  listen("agent-acted", () => {
    redrawAfterAgent();
  });
  // Practice starts, ends or scores a try: a section's practice puts the
  // loop away and gives it back, so the looper is asked again too.
  listen<PracticeView>("practice", (event) => {
    screen?.showPractice(event.payload);
    sendInOrder(askForSound);
  });
  listen<{ marks: PracticeMark[] }>("practice-marks", (event) => {
    screen?.showPracticeMarks(event.payload.marks);
  });
}

// The sections tell the page as another starts and once they have played
// (src-tauri/src/lib.rs).
function hearTheSections() {
  listen("section-playing", () => {
    sendInOrder(askForSections);
  });
  listen("arrangement-stopped", () => {
    sendInOrder(askForSections);
    sendInOrder(askForSound);
  });
}

// An instrument finished loading, or one came or went in the folder: the
// sound says again whether each is ready (src-tauri/src/lib.rs).
function hearTheInstruments() {
  listen("instruments-changed", () => {
    sendInOrder(askForSound);
  });
}

// Asks Rust what each key does, then shows the page.
async function start() {
  let layout: KeyLayout;
  try {
    layout = await invoke<KeyLayout>("key_layout");
  } catch (error) {
    page.append(makeElement("p", "start-error", `Could not start: ${error}`));
    return;
  }
  drumModeKeys = layout.drum_mode_keys;
  guitarShapeKeys = layout.guitar_shapes;

  screen = mount(Stage, {
    target: page,
    props: {
      platform: detectPlatform(),
      layout,
      onRenderClick: renderWav,
      onSnapClick: () => sendInOrder(switchSnap),
      onRecordClick: () => sendInOrder(recordLoop),
      onExpressionChange: (key: string, held: boolean) => {
        if (held) {
          screen?.keyDown(key);
          sendInOrder(() => pressKey(key));
        } else {
          screen?.keyUp(key);
          sendInOrder(() => releaseKey(key));
        }
      },
      onTempoChange: (tempo: number) => sendInOrder(() => setTempo(tempo)),
      onLayerBarsChange: (layer: number, bars: number) => sendInOrder(() => setLayerBars(layer, bars)),
      onVolumeChange: (layer: number, volume: number) =>
        sendInOrder(() => setLayerVolume(layer, volume)),
      onLayerPlayingChange: (layer: number, playing: boolean) =>
        sendInOrder(() => setLayerPlaying(layer, playing)),
      onLoopPlayingChange: (playing: boolean) =>
        sendInOrder(() => setLoopPlaying(playing)),
      onScalePick: (tonic: number, kind: string) =>
        sendInOrder(() => pickScale(tonic, kind)),
      onPlayScalePick: (tonic: number, kind: string) =>
        sendInOrder(() => pickPlayScale(tonic, kind)),
      onTabChange: (tab: Tab) => sendInOrder(() => tabChanged(tab)),
      mappingSender,
      onChordClick: (place: number) =>
        sendInOrder(() => playChordInSong(place)),
      onPlayClick: () => sendInOrder(playSong),
      onComposeEdit: (action: string, place: number) =>
        sendInOrder(() => editComposition(action, place)),
      onSectionEdit: (edit: SectionEdit) =>
        sendInOrder(() => editSections(edit)),
      soundSender,
      mixSender,
      songSender,
      onSongChanged: redrawAfterAgent,
    },
  });
  hearTheAgent();
  hearTheInstruments();
  hearTheSections();
  sendInOrder(askForSound);
  sendInOrder(askForCompose);
  sendInOrder(askForMix);
  sendInOrder(async () => {
    await askForSongs();
  });
}

start();
