#pragma once

#include <cstdint>

// How many pitches a loop holds: MIDI's notes, 0 to 127, then 128 for the
// vibrato key's presses (Synth::vibrato_key_note), 129 for the glide key's
// (Synth::glide_key_note), 130 to 145 for the drum pads' hits (drum_kit.h)
// and 146 for the pedal's presses (Synth::pedal_key_note), each recorded and
// played back like a note's.
constexpr int loop_pitch_count = 147;

// One note of a loop: its pitch, how hard it was hit, and the samples it
// starts and ends on, counted from the loop's start.
struct LoopNote {
    int note;  // 0 to 146
    int velocity;
    std::uint64_t start;
    std::uint64_t end;
};

// Works out where a loop plays its notes each time around
// (docs/adr/0021-a-looper-on-a-beat.md). `played` holds one layer's `count`
// notes as they were played, in any order (docs/adr/0026-layers.md).
// `planned` gets the same notes, in the order they start each time around,
// each where it was played. A note ends no later than the next start of the
// same key in its layer: the synth plays a layer's key on one voice, so the
// note's end would cut the next one off.
//
// It never locks, allocates or waits, so the audio thread can use it.
void plan_loop(const LoopNote* played, int count, LoopNote* planned);
