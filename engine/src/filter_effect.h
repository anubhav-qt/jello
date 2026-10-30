#pragma once

#include "effect.h"
#include "filter.h"
#include "listener.h"

// The filter effect's values, numbered.
constexpr int filter_effect_type = 0;            // a FilterType
constexpr int filter_effect_slope = 1;           // dB an octave: 12 or 24
constexpr int filter_effect_cutoff = 2;          // Hz, filter_lowest_hz to filter_open_hz
constexpr int filter_effect_resonance = 3;       // 0 to 1
constexpr int filter_effect_drive = 4;           // 0 to 1
constexpr int filter_effect_follow = 5;          // octaves, -filter_most_octaves to filter_most_octaves
constexpr int filter_effect_follow_attack = 6;   // ms, follow_shortest_attack_ms to follow_longest_attack_ms
constexpr int filter_effect_follow_release = 7;  // ms, follow_shortest_release_ms to follow_longest_release_ms
constexpr int filter_effect_mix = 8;             // 0 to 1
constexpr int filter_effect_listens_to = 9;      // a "listens to" (effect.h), for the follower
constexpr int filter_effect_value_count = 10;

constexpr float follow_shortest_attack_ms = 0.1f;
constexpr float follow_longest_attack_ms = 100.0f;
constexpr float follow_shortest_release_ms = 5.0f;
constexpr float follow_longest_release_ms = 2000.0f;

// True if the filter effect has value `value` and `number` is within its
// range.
// The range of value `value`.
ValueRange filter_effect_value_range(int value);

bool is_valid_filter_effect_value(int value, float number);

// The filter effect (docs/design.md#mix-in-the-engine): the voices' state
// variable filter, here on a whole track, bus or the master. A drive
// before it rounds off loud peaks. Two in a row make a 24 dB slope; only
// the first rings with the resonance, so the peak stays the same size. An
// envelope follower moves the cutoff by how loud what it listens to is: at
// full loudness by the follow amount, in octaves, up or down. A new one is
// an open low-pass, which changes nothing.
class FilterEffect : public Effect {
public:
    explicit FilterEffect(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    // The follower's loudness after `peak`, rising over the attack and
    // falling over the release.
    double follow(double peak);

    // How much should go through the filters, 0 while open, and how much
    // of the second filter should be heard.
    double share_target() const;
    double steep_target() const;

    int sample_rate_;
    double lowest_octave_;
    double highest_octave_;

    FilterType type_ = FilterType::low_pass;
    bool steep_ = false;
    // The cutoff as an octave number (log2 of its Hz), and as set, in Hz.
    double cutoff_;
    double cutoff_hz_;
    double resonance_ = 0.0;
    double drive_ = 0.0;
    double follow_ = 0.0;
    // The follower's attack and release as set, in ms.
    double attack_ms_;
    double release_ms_;
    double attack_share_ = 0.0;
    double release_share_ = 0.0;
    double mix_ = 1.0;
    Listener listener_;

    Glide heard_cutoff_;
    Glide heard_resonance_;
    Glide heard_drive_;
    Glide heard_follow_;
    Glide heard_mix_;
    // How much of the second filter is heard: 0 at 12 dB, 1 at 24 dB.
    Glide heard_steep_;
    // How much goes through the filters at all: 0 while open.
    Glide heard_share_;
    // The type's parts, each crossfading to a new type's mix.
    Glide heard_low_;
    Glide heard_band_;
    Glide heard_high_;

    StateVariableFilter first_;
    StateVariableFilter second_;
    double loudness_ = 0.0;

    // The cutoff, in octaves, and resonance the filters were last set to.
    bool filters_ready_ = false;
    double set_cutoff_ = 0.0;
    double set_resonance_ = 0.0;
};
