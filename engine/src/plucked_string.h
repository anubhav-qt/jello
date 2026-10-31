#pragma once

#include <cstdint>

#include "biquad.h"

// A plucked string (docs/design.md#guitar): the string oscillator kind. A
// period's worth of sound runs round a delay line and loses a little on each
// trip, the way a wave runs up and down a string, so no recording is needed.
// Its pitch comes from the delay's length, so vibrato and glide bend it.

// The shortest and longest a string can take to ring away, in seconds.
constexpr float string_shortest_decay_seconds = 0.1f;
constexpr float string_longest_decay_seconds = 30.0f;

// How a string sounds, each the song's own to change.
struct StringSettings {
    // How long it takes to fall 60 dB.
    float decay_seconds = 4.0f;
    // How much of its top it keeps on each trip, from 0 (dull) to 1 (bright).
    float tone = 0.6f;
    // Where along the string it is plucked, as a share from the bridge, from
    // 0.02 to 0.5. Near the bridge is thin, at the middle round.
    float pick = 0.2f;
    // Where a pickup hears it, as a share from the bridge, or 0 for no pickup.
    float pickup = 0.0f;
    // How much top the pickup hears, from 0 (a dark, thick coil) to 1 (a thin,
    // glassy one). Only with a pickup.
    float pickup_tone = 0.6f;
    // How much of it comes through an acoustic guitar's wooden body, from 0 to 1.
    float body = 0.0f;
};

class PluckedString {
public:
    // How many samples of memory one string needs at `sample_rate`, a power
    // of two.
    static int memory_size(int sample_rate);

    // Gives the string its `memory`, memory_size samples long, unwritten
    // until its first pluck. It is never freed or moved by the string.
    void attach(float* memory, int size, int sample_rate);

    // Plucks it, stopping what it was playing. `cycle_step` is how far one
    // sample moves through a cycle, hz over the sample rate, and `hardness`
    // from 0 to 1 makes a harder pluck brighter. `press_number` and `place`
    // give it its own noise, so no two plucks are alike.
    void pluck(double cycle_step, double hardness, const StringSettings& settings, std::uint64_t press_number,
               std::uint64_t place);

    // How high it is right now, about -1 to 1.
    double height() const;

    // Moves one sample on, at this pitch.
    void advance(double cycle_step);

private:
    // One of the body's resonances: a two-pole filter.
    struct Resonance {
        double a1 = 0.0;
        double a2 = 0.0;
        double gain = 0.0;
        double y1 = 0.0;
        double y2 = 0.0;

        void tune(double hz, double q, double level, int sample_rate);
        double next(double in);
    };

    // The delay line `back` samples behind where it writes next, between
    // samples.
    double read_back(double back) const;

    void write_pluck(int period, double hardness, const StringSettings& settings, std::uint64_t press_number,
                     std::uint64_t place);

    float* memory_ = nullptr;
    int size_ = 0;
    int mask_ = 0;
    int sample_rate_ = 48000;
    int write_ = 0;

    // What each trip costs: how far it falls, and how much it is averaged
    // with the sample before, which takes off the top.
    double loop_gain_ = 0.0;
    double smoothing_ = 0.0;
    double last_read_ = 0.0;

    // A pickup's place, as a share of the string, and the body's share.
    double pickup_ = 0.0;
    Biquad pickup_coil_;
    double body_ = 0.0;
    static constexpr int resonance_count = 3;
    Resonance resonances_[resonance_count];

    // Takes off any slow drift from the middle line.
    double blocker_in_ = 0.0;
    double blocker_out_ = 0.0;

    double height_ = 0.0;
};
