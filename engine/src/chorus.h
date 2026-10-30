#pragma once

#include <vector>

#include "delay_line.h"
#include "effect.h"
#include "filter.h"

// The chorus's values, numbered.
constexpr int chorus_copies = 0;    // 1 to chorus_most_copies
constexpr int chorus_speed = 1;     // Hz, chorus_slowest_hz to chorus_fastest_hz
constexpr int chorus_cycle = 2;     // beats, chorus_shortest_cycle to chorus_longest_cycle
constexpr int chorus_on_beat = 3;   // 0 or 1: 1 moves once a cycle, on the beat, instead of at the speed
constexpr int chorus_depth = 4;     // 0 to 1
constexpr int chorus_delay = 5;     // ms, chorus_shortest_delay_ms to chorus_longest_delay_ms
constexpr int chorus_feedback = 6;  // 0 to chorus_most_feedback
constexpr int chorus_spread = 7;    // 0 to 1
constexpr int chorus_mix = 8;       // 0 to 1
constexpr int chorus_value_count = 9;

constexpr int chorus_most_copies = 4;
constexpr float chorus_slowest_hz = 0.01f;
constexpr float chorus_fastest_hz = 10.0f;
constexpr float chorus_shortest_cycle = 1.0f / 16.0f;
constexpr float chorus_longest_cycle = 32.0f;
constexpr float chorus_shortest_delay_ms = 1.0f;
constexpr float chorus_longest_delay_ms = 40.0f;
constexpr float chorus_most_feedback = 0.95f;

// True if the chorus has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange chorus_value_range(int value);

bool is_valid_chorus_value(int value, float number);

// The chorus (docs/design.md#mix-in-the-engine): copies of the sound, each
// a little late, the lateness swinging up and down so each copy's pitch
// wobbles a little, like several players never quite together. Each copy
// swings at the same speed, its own share of a turn apart, and the copies
// sit spread across the speakers. The swing is the depth's share of the
// delay, at most 90% of it. Short delays with feedback make a flanger. A
// new one: 2 copies, 0.8 Hz, a quarter deep, 7 ms, no feedback, fully
// spread, and the starting mix (start_mix).
class Chorus : public Effect {
public:
    Chorus(int sample_rate, bool on_bus);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    struct Copy {
        // How much of the copy is heard, 0 or 1, crossfading.
        Glide level;
        // How far round the swing it is from the first copy, as a share of
        // a turn, and where it sits, from -1 (left) to 1 (right).
        Glide offset;
        Glide pan;
    };

    // Where a copy's swing is, from -1 to 1, at `turn` of the way round.
    static double swing_at(double turn);

    int sample_rate_;

    int copies_ = 2;
    double speed_hz_ = 0.8;
    double cycle_ = 1.0;
    double leaving_cycle_ = 1.0;
    bool on_beat_ = false;
    double depth_ = 0.25;
    double delay_ms_ = 7.0;
    double feedback_ = 0.0;
    double spread_ = 1.0;
    double mix_;

    Glide heard_speed_hz_;
    Glide heard_depth_;
    Glide heard_delay_ms_;
    Glide heard_feedback_;
    Glide heard_mix_;
    // How much the swing follows the beat rather than the speed, and the
    // new cycle rather than the one it is leaving, both crossfading.
    Glide heard_on_beat_;
    Glide heard_cycle_;
    std::vector<Copy> copy_;

    // Where the swing at the speed is, as a share of a turn.
    double turn_ = 0.0;

    // The copies all read one line of the sound, both speakers together,
    // with the feedback added in.
    DelayLine line_;
};
