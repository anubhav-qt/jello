#pragma once

#include "biquad.h"
#include "delay_line.h"
#include "effect.h"
#include "filter.h"

// The delay's values, numbered.
constexpr int delay_left_ms = 0;       // ms, delay_shortest_ms to delay_longest_ms
constexpr int delay_right_ms = 1;      // the same
constexpr int delay_left_beats = 2;    // beats, delay_shortest_beats to delay_longest_beats
constexpr int delay_right_beats = 3;   // the same
constexpr int delay_on_beat = 4;       // 0 or 1: 1 times both sides in beats instead of ms
constexpr int delay_linked = 5;        // 0 or 1: 1 gives the right side the left's time
constexpr int delay_ping_pong = 6;     // 0 or 1
constexpr int delay_feedback = 7;      // 0 to 1
constexpr int delay_low_cut = 8;       // Hz, delay_lowest_hz to delay_highest_hz
constexpr int delay_high_cut = 9;      // Hz, the same
constexpr int delay_wobble = 10;       // 0 to 1
constexpr int delay_wobble_speed = 11; // Hz, delay_slowest_wobble_hz to delay_fastest_wobble_hz
constexpr int delay_grit = 12;         // 0 to 1
constexpr int delay_duck = 13;         // 0 to 1
constexpr int delay_mix = 14;          // 0 to 1
constexpr int delay_value_count = 15;

constexpr float delay_shortest_ms = 1.0f;
constexpr float delay_longest_ms = 8000.0f;
constexpr float delay_shortest_beats = 1.0f / 16.0f;
constexpr float delay_longest_beats = 16.0f;
constexpr float delay_lowest_hz = 20.0f;
constexpr float delay_highest_hz = 20000.0f;
constexpr float delay_slowest_wobble_hz = 0.1f;
constexpr float delay_fastest_wobble_hz = 10.0f;

// True if the delay has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange delay_value_range(int value);

bool is_valid_delay_value(int value, float number);

// The delay (docs/design.md#mix-in-the-engine): echoes of the sound, each
// side after its own time, in ms or in beats, never longer than
// delay_longest_ms whatever the tempo. A new time slides there like tape
// over 100 ms, so the echoes bend in pitch on the way. Feedback sends each
// echo round again; ping-pong bounces them from side to side, starting on
// the left. Each echo goes through a low cut and a high cut, so each
// repeat comes back thinner or darker, then grit, which saturates it, and
// wobble, a slow swing in the time like a worn tape. Duck quietens the
// echoes while the dry sound plays. A new one: half a beat, linked, 35%
// feedback, nothing cut, no wobble, grit or duck, and the starting mix
// (start_mix).
class Delay : public Effect {
public:
    Delay(int sample_rate, bool on_bus);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    // A time asked for, in samples, at `beats_per_sample`: `ms`, or `beats`
    // while timed on the beat, never longer than the longest.
    double time_in_samples(double ms, double beats, double beats_per_sample) const;

    // An echo after grit at `grit`.
    static double gritty(double echo, double grit);

    // Sets the cut filters to the frequencies heard now.
    void set_cuts(double low_cut_hz, double high_cut_hz);

    int sample_rate_;
    double longest_samples_;

    double left_ms_ = 250.0;
    double right_ms_ = 250.0;
    double left_beats_ = 0.5;
    double right_beats_ = 0.5;
    bool on_beat_ = true;
    bool linked_ = true;
    bool ping_pong_ = false;
    double feedback_ = 0.35;
    double low_cut_hz_ = delay_lowest_hz;
    double high_cut_hz_ = delay_highest_hz;
    double wobble_ = 0.0;
    double wobble_hz_ = 0.5;
    double grit_ = 0.0;
    double duck_ = 0.0;
    double mix_;

    // Each side's time, in samples, sliding like tape, and whether it has
    // played yet: the first piece starts right on the times.
    bool started_ = false;
    Glide heard_left_;
    Glide heard_right_;
    Glide heard_feedback_;
    Glide heard_low_cut_hz_;
    Glide heard_high_cut_hz_;
    Glide heard_wobble_;
    Glide heard_wobble_hz_;
    Glide heard_grit_;
    Glide heard_duck_;
    Glide heard_mix_;
    // How much the echoes bounce from side to side, crossfading.
    Glide heard_ping_pong_;

    DelayLine left_line_;
    DelayLine right_line_;
    Biquad low_cut_;
    Biquad high_cut_;
    double set_low_cut_hz_ = 0.0;
    double set_high_cut_hz_ = 0.0;

    // Where the wobble's swing is, as a share of a turn.
    double wobble_turn_ = 0.0;

    // How loud the dry sound has been lately, for ducking, and the shares
    // it rises and falls by each sample.
    double dry_level_ = 0.0;
    double duck_rise_;
    double duck_fall_;
};
