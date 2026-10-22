#pragma once

#include <cstdint>

#include "clock.h"

// How many pitches a loop holds: MIDI's notes, 0 to 127, and two more,
// 128 for the vibrato key's presses (Synth::vibrato_key_note) and 129 for
// the glide key's (Synth::glide_key_note), recorded and played back like a
// note's.
constexpr int loop_pitch_count = 130;

// One note of a loop: its pitch, how hard it was hit, and the samples it
// starts and ends on, counted from the loop's start.
struct LoopNote {
    int note;  // 0 to 129
    int velocity;
    std::uint64_t start;
    std::uint64_t end;
};

// Works out where a loop plays its notes each time around
// (docs/adr/0021-a-looper-on-a-beat.md). `played` holds one layer's `count`
// notes as they were played, in any order, in a loop `length` samples long,
// recorded at `beat` (docs/adr/0026-layers.md). `planned` gets the same
// notes, in the order they start each time around.
//
// With `snap` off, each note plays as it was played. With it on, its start
// and its end move to the nearest half beat, and it lasts at least half a
// beat. A note within a quarter beat of the loop's end moves round to beat
// 1, for half a beat. Either way, a note ends no later than the next start
// of the same key in its layer: the synth plays a layer's key on one voice,
// so the note's end would cut the next one off.
//
// It never locks, allocates or waits, so the audio thread can use it.
void plan_loop(const LoopNote* played, int count, std::uint64_t length, BeatLength beat, bool snap,
               LoopNote* planned);
