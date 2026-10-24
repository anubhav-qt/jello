#pragma once

#include <array>

#include "wave.h"

// Oscillators (docs/adr/0035-oscillators-with-kinds.md): every note is
// played by oscillator_count oscillators at once, added up before the
// filter. Each one picks a kind, plays at its own level, and is tuned its
// own distance from the note.

// How many oscillators play each note.
constexpr int oscillator_count = 3;

// What an oscillator plays. The first four are the waves (wave.h), in the
// same order; later kinds, like wavetables, come after the sample.
enum class Kind {
    sine,
    triangle,
    square,
    saw,
    noise,   // a new random height every sample: every pitch at once, a hiss
    sample,  // a sampled instrument's recording (docs/design.md#sampler)
};

// How many kinds there are.
constexpr int kind_count = 6;

// How many kinds the routing list steps through: the waves and noise. A
// sample needs its instrument, so it is only ever picked, never stepped to.
constexpr int stepped_kind_count = 5;

// How far an oscillator can be tuned from the note, either way: in whole
// octaves, then whole half steps, then cents (hundredths of a half step).
constexpr int oscillator_most_octaves = 3;
constexpr int oscillator_most_half_steps = 12;
constexpr float oscillator_most_cents = 100.0f;

// The narrowest and widest a square's pulse can be, as a share of each
// cycle (docs/design.md#pulse-width). At 0 or 1 the wave would never move,
// and below 5% the thinnest pulses on high notes get narrower than a sample.
constexpr float oscillator_narrowest_width = 0.05f;
constexpr float oscillator_widest_width = 0.95f;

// One oscillator's settings. Changing them changes the notes started after.
struct OscillatorSettings {
    Kind kind = Kind::triangle;
    // How loud it plays, from 0 (silent, and skipped) to 1.
    float level = 0.0f;
    // How far it is tuned from the note, each from minus to plus its most.
    // Noise has no pitch, so they do nothing to it.
    int octaves = 0;
    int half_steps = 0;
    float cents = 0.0f;
    // How much of each cycle a square spends up, from
    // oscillator_narrowest_width to oscillator_widest_width. At a half it is
    // the square; narrower or wider, a thinner pulse. Only the square uses
    // it.
    float width = 0.5f;
};

// Where a new song's oscillators start: the first a triangle at full level,
// the others silent, so a note sounds just as one wave did.
std::array<OscillatorSettings, oscillator_count> start_oscillators();

// How far `oscillator` is tuned from the note, in cents.
double offset_cents(const OscillatorSettings& oscillator);

// The wave `kind` plays. Only for the four waves, not noise or a sample.
Wave wave_of(Kind kind);
