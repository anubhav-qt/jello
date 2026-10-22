#pragma once

#include <array>
#include <cstdint>

// LFOs (docs/adr/0031-lfos-and-a-routing-list.md): low frequency
// oscillators, slow waves that move a setting up and down while a note
// plays. A vibrato is an LFO moving the pitch. Each song has lfo_count of
// them, and the routing list (routing.h) says what each one moves.

// How many LFOs a song has.
constexpr int lfo_count = 8;

// The most steps the steps shape can have.
constexpr int lfo_most_steps = 32;

// The slowest and fastest an LFO runs in Hz: once every 100 seconds, and
// 100 times a second.
constexpr double lfo_slowest_hz = 0.01;
constexpr double lfo_fastest_hz = 100.0;

// The shortest and longest one cycle can last in beats: a sixteenth of a
// beat, and 32 bars of 4 beats.
constexpr double lfo_shortest_beats = 1.0 / 16.0;
constexpr double lfo_longest_beats = 128.0;

// The most key presses one cycle can take.
constexpr int lfo_most_presses = 32;

// The longest an LFO's delay or fade-in can take, in ms and in beats.
constexpr int lfo_longest_ms = 10000;
constexpr double lfo_longest_delay_beats = 128.0;

// The shapes an LFO can have. Each goes from -1 to 1.
enum class LfoShape {
    sine,           // swings smoothly up to 1, down to -1, then back to 0
    triangle,       // the same in straight lines
    square,         // 1 for the first half of each cycle, -1 for the second
    saw_up,         // rises from -1 to 1, then jumps back down
    saw_down,       // falls from 1 to -1, then jumps back up
    random_steps,   // a new random height each cycle, held
    smooth_random,  // glides from one random height to the next each cycle
    steps,          // the heights you set, one after another, each held
};

// What an LFO's speed is counted in.
enum class LfoSpeed {
    hz,       // cycles a second
    beats,    // how many beats one cycle lasts, at the song's tempo
    presses,  // how many key presses one cycle takes: one step per press
};

// One LFO's settings. A new song starts each LFO here.
struct LfoSettings {
    LfoShape shape = LfoShape::sine;

    // For the steps shape: how many steps, from 1 to lfo_most_steps, and
    // each one's height, from -1 to 1. Only the first `step_count` count.
    int step_count = 8;
    std::array<float, lfo_most_steps> steps{};

    // Per note, every note starts its own copy of the LFO when it starts.
    // Shared, one LFO runs for every note: from the engine's start in Hz,
    // and on the beat in beats.
    bool per_note = true;

    LfoSpeed speed = LfoSpeed::hz;
    double hz = 5.5;               // from lfo_slowest_hz to lfo_fastest_hz
    double beats = 1.0;            // from lfo_shortest_beats to lfo_longest_beats
    int presses = 8;               // from 1 to lfo_most_presses

    // Where in its cycle it starts, from 0 to 1. And for a per-note LFO, how
    // far each note's start is moved at random on top, from 0 (none) to 1
    // (anywhere in the cycle).
    double start = 0.0;
    double spread = 1.0;

    // How long after each note starts the LFO waits before it moves the
    // note, then how long it takes to grow in a straight line to its full
    // size. In ms, or in beats while the speed is in beats.
    int delay_ms = 0;
    int fade_ms = 0;
    double delay_beats = 0.0;
    double fade_beats = 0.0;

    // How much of the time between jumps is spent sliding instead, from 0
    // (jumps at once) to 1 (always sliding). It only touches the shapes that
    // jump: square, both saws, random steps and steps. In presses, it slides
    // over that share of a beat.
    double smooth = 0.1;

    // For a per-note LFO: one cycle, then it holds where it ended, so it
    // works like an extra envelope.
    bool once = false;
};

// Where a moving value is: on its way from `from` to `to`, `done` of the way
// there, from 0 to 1. A value that is not sliding has the same `from` and
// `to`. Kept apart instead of as one number so a move snapped to half steps
// can snap both ends and still slide smoothly between them.
struct Slide {
    double from = 0.0;
    double to = 0.0;
    double done = 1.0;

    // A value that is not sliding.
    static Slide at(double value);

    // Where it is now.
    double value() const;
};

// How high an LFO with `settings` is `cycles` cycles after its cycle 0
// began: the whole number says which cycle, the rest how far into it. Its
// random shapes take their heights from `who` and `lfo`, so the same LFO of
// the same note always moves the same way.
Slide lfo_height(const LfoSettings& settings, double cycles, std::uint64_t who, int lfo);

// Turns a number into one that looks unrelated to it: SplitMix64's mixing
// step, by Sebastiano Vigna (public domain). Numbers next to each other come
// out far apart.
std::uint64_t mix(std::uint64_t number);

// A number from -1 to 1 that looks random but is always the same for the
// same three numbers, so a render always sounds the same.
double random_height(std::uint64_t who, std::uint64_t what, std::uint64_t when);
