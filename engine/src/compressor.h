#pragma once

#include "biquad.h"
#include "effect.h"
#include "filter.h"
#include "listener.h"

// The compressor's values, numbered.
constexpr int compressor_threshold = 0;   // dB, compressor_lowest_threshold_db to 0
constexpr int compressor_ratio = 1;       // 1 to compressor_most_ratio, or compressor_limit
constexpr int compressor_attack = 2;      // ms, compressor_shortest_attack_ms to compressor_longest_attack_ms
constexpr int compressor_release = 3;     // ms, compressor_shortest_release_ms to compressor_longest_release_ms
constexpr int compressor_knee = 4;        // dB, 0 to compressor_widest_knee_db
constexpr int compressor_makeup = 5;      // dB, 0 to compressor_most_makeup_db
constexpr int compressor_mix = 6;         // 0 to 1
constexpr int compressor_listens_to = 7;  // a "listens to" (effect.h)
constexpr int compressor_low_cut = 8;     // Hz, compressor_lowest_cut_hz to compressor_highest_cut_hz
constexpr int compressor_value_count = 9;

constexpr float compressor_lowest_threshold_db = -60.0f;
constexpr float compressor_most_ratio = 20.0f;
// The ratio that makes it a limiter: nothing passes the threshold.
constexpr float compressor_limit = 0.0f;
constexpr float compressor_shortest_attack_ms = 0.1f;
constexpr float compressor_longest_attack_ms = 100.0f;
constexpr float compressor_shortest_release_ms = 5.0f;
constexpr float compressor_longest_release_ms = 2000.0f;
constexpr float compressor_widest_knee_db = 24.0f;
constexpr float compressor_most_makeup_db = 24.0f;
constexpr float compressor_lowest_cut_hz = 20.0f;
constexpr float compressor_highest_cut_hz = 500.0f;

// True if the compressor has value `value` and `number` is within its
// range.
// The range of value `value`.
ValueRange compressor_value_range(int value);

bool is_valid_compressor_value(int value, float number);

// The compressor (docs/design.md#mix-in-the-engine): turns the sound down
// once what it listens to goes over the threshold, so loud and quiet parts
// sit closer together. Past the threshold, each dB louder comes out only
// 1/ratio dB louder; the knee starts that gently, over its width around
// the threshold. It reads the peaks of what it listens to, after a low cut
// so deep bass doesn't make it pump, and turns down over the attack and
// back up over the release, both speakers together. Then the makeup gain,
// and a mix with the dry sound: under 1 is parallel compression. A new one
// has its threshold at 0 dB, so it changes nothing under 0 dB.
class Compressor : public Effect {
public:
    explicit Compressor(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;
    float take_reduction_db() override;

private:
    // How far to turn down a peak `level_db` loud, in dB, at the threshold,
    // slope and knee heard now.
    double wanted_reduction_db(double level_db, double threshold_db, double slope, double knee_db) const;

    // Sets the low cut's filter to `hz`.
    void set_low_cut(double hz);

    int sample_rate_;

    double threshold_db_ = 0.0;
    // The ratio, attack and release as set.
    double ratio_;
    double attack_ms_;
    double release_ms_;
    // How much of each dB past the threshold is taken off: 1 - 1/ratio,
    // from 0 (1:1) to 1 (limit).
    double slope_ = 0.75;
    double attack_share_ = 0.0;
    double release_share_ = 0.0;
    double knee_db_ = 0.0;
    double makeup_db_ = 0.0;
    double mix_ = 1.0;
    double low_cut_hz_ = compressor_lowest_cut_hz;

    Glide heard_threshold_db_;
    Glide heard_slope_;
    Glide heard_knee_db_;
    Glide heard_makeup_db_;
    Glide heard_mix_;
    Glide heard_low_cut_hz_;

    Listener listener_;
    Biquad low_cut_;
    double set_low_cut_hz_ = 0.0;

    // The reduction, in dB: held at each peak and falling over the release,
    // then followed over the attack.
    double held_db_ = 0.0;
    double reduction_db_ = 0.0;
    double most_reduction_db_ = 0.0;
};
