// Tests for the UI's own small rules: which keys the loop lights, and how a
// delay's length in beats is written. Node runs the TypeScript files as they
// are, so they may only import types.
import assert from "node:assert/strict";
import test from "node:test";

import { keysByNotes, keysPressedNow, layerPresses } from "../../ui/loop-keys.ts";
import { beatWords } from "../../ui/mix.ts";

// A layout with a melody key, two keys for one chord and a drum key.
function layout() {
  return {
    notes: { KeyJ: 60, KeyK: 62 },
    chords: {
      KeyX: { notes: [64, 60, 67] },
      KeyA: { notes: [60, 64, 67] },
      KeyQ: { notes: [57, 60, 64] },
    },
    drums: { KeyF: "snare" },
  };
}

const PADS = ["kick", "snare", "rim"];

function note(pitch, start, end) {
  return { note: pitch, start, end };
}

function layer(notes, extra = {}) {
  return { bars: 1, notes, volume: 100, playing: true, recording: false, ...extra };
}

const LOOPING = { state: "looping", number: 1 };

test("each set of notes finds the first key by name that plays it", () => {
  const keys = keysByNotes(layout(), PADS);
  assert.equal(keys.get("60"), "KeyJ");
  assert.equal(keys.get("60,64,67"), "KeyA");
  assert.equal(keys.get("57,60,64"), "KeyQ");
  assert.equal(keys.get("131"), "KeyF");
  assert.equal(keys.get("130"), undefined);
});

test("a drum key whose pad the kit lacks plays nothing", () => {
  const keys = keysByNotes(layout(), ["kick"]);
  assert.equal(keys.get("131"), undefined);
  assert.equal(keys.size, 4);
});

test("notes that start and end together are one press", () => {
  const keys = keysByNotes(layout(), PADS);
  const notes = [note(67, 0, 1), note(60, 0, 1), note(64, 0, 1), note(62, 1, 1.5), note(64, 2, 2.5)];
  const [presses] = layerPresses([layer(notes)], keys);
  assert.deepEqual(presses.presses, [
    { key: "KeyA", start: 0, end: 1 },
    { key: "KeyK", start: 1, end: 1.5 },
  ]);
});

test("a chord's notes that end apart are not that chord", () => {
  const keys = keysByNotes(layout(), PADS);
  const notes = [note(60, 0, 1), note(64, 0, 1), note(67, 0, 2)];
  const [presses] = layerPresses([layer(notes)], keys);
  assert.deepEqual(presses.presses, []);
});

test("a note still held is left out", () => {
  const keys = keysByNotes(layout(), PADS);
  const [presses] = layerPresses([layer([note(60, 0, null)])], keys);
  assert.deepEqual(presses.presses, []);
});

test("a key lights from its start up to, not at, its end", () => {
  const keys = keysByNotes(layout(), PADS);
  const layers = layerPresses([layer([note(60, 1, 2)])], keys);
  assert.deepEqual([...keysPressedNow(LOOPING, layers, 0.99)], []);
  assert.deepEqual([...keysPressedNow(LOOPING, layers, 1)], ["KeyJ"]);
  assert.deepEqual([...keysPressedNow(LOOPING, layers, 1.99)], ["KeyJ"]);
  assert.deepEqual([...keysPressedNow(LOOPING, layers, 2)], []);
});

test("a short layer goes round on its own inside a longer loop", () => {
  const keys = keysByNotes(layout(), PADS);
  const looper = { state: "looping", number: 2 };
  const layers = layerPresses([layer([note(60, 1, 2)], { bars: 1 })], keys);
  assert.deepEqual([...keysPressedNow(looper, layers, 5.5)], ["KeyJ"]);
  // A layer of 0 bars is as long as the loop.
  const whole = layerPresses([layer([note(60, 1, 2)], { bars: 0 })], keys);
  assert.deepEqual([...keysPressedNow(looper, whole, 5.5)], []);
  assert.deepEqual([...keysPressedNow(looper, whole, 1.5)], ["KeyJ"]);
});

test("paused layers, a layer being recorded and a stopped loop light nothing", () => {
  const keys = keysByNotes(layout(), PADS);
  const notes = [note(60, 0, 1)];
  const paused = layerPresses([layer(notes, { playing: false })], keys);
  assert.deepEqual([...keysPressedNow(LOOPING, paused, 0.5)], []);
  const recording = layerPresses([layer(notes, { recording: true })], keys);
  assert.deepEqual([...keysPressedNow(LOOPING, recording, 0.5)], []);
  const playing = layerPresses([layer(notes)], keys);
  for (const state of ["idle", "counting in", "recording", "paused"]) {
    assert.deepEqual([...keysPressedNow({ state, number: 1 }, playing, 0.5)], []);
  }
  assert.deepEqual([...keysPressedNow({ state: "layering", number: 1 }, playing, 0.5)], ["KeyJ"]);
});

test("every beat step, as Rust sends it back, reads as its fraction", () => {
  const steps = {
    "1/16": 1 / 16, "1/12": 1 / 12, "3/32": 3 / 32, "1/8": 1 / 8, "1/6": 1 / 6, "3/16": 3 / 16,
    "1/4": 1 / 4, "1/3": 1 / 3, "3/8": 3 / 8, "1/2": 1 / 2, "2/3": 2 / 3, "3/4": 3 / 4, "1": 1,
    "4/3": 4 / 3, "3/2": 3 / 2, "2": 2, "3": 3, "4": 4, "6": 6, "8": 8, "12": 12, "16": 16,
    "24": 24, "32": 32,
  };
  for (const [words, beats] of Object.entries(steps)) {
    // Rust keeps 4 decimals of an f32.
    const fromRust = Number(Math.fround(beats).toFixed(4));
    assert.equal(beatWords(beats), words);
    assert.equal(beatWords(fromRust), words);
  }
  assert.equal(beatWords(0.55), "11/20");
  assert.equal(beatWords(0.123456), "0.123");
});
