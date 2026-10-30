#pragma once

#include "effect.h"
#include "filter.h"
#include "listener.h"

// The gate's values, numbered.
constexpr int gate_threshold = 0;   // dB, gate_lowest_threshold_db to 0
constexpr int gate_range = 1;       // dB, gate_silent_range_db to 0
constexpr int gate_attack = 2;      // ms, gate_shortest_attack_ms to gate_longest_attack_ms
constexpr int gate_hold = 3;        // ms, 0 to gate_longest_hold_ms
constexpr int gate_release = 4;     // ms, gate_shortest_release_ms to gate_longest_release_ms
constexpr int gate_listens_to = 5;  // a "listens to" (effect.h)
constexpr int gate_value_count = 6;

// At its lowest, the threshold never closes the gate.
constexpr float gate_lowest_threshold_db = -80.0f;
// At its lowest, the range closes the gate to silence.
constexpr float gate_silent_range_db = -80.0f;
constexpr float gate_shortest_attack_ms = 0.1f;
constexpr float gate_longest_attack_ms = 50.0f;
constexpr float gate_longest_hold_ms = 500.0f;
constexpr float gate_shortest_release_ms = 5.0f;
constexpr float gate_longest_release_ms = 2000.0f;

// True if the gate has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange gate_value_range(int value);

bool is_valid_gate_value(int value, float number);

// The gate (docs/design.md#mix-in-the-engine): turns the sound down by its
// range while what it listens to stays under the threshold, which cuts
// noise and tails. It opens over the attack as soon as a peak reaches the
// threshold, stays open for the hold after the last one, then closes over
// the release, each in a straight line, both speakers together. A new one
// has its threshold at the lowest, so it never closes.
class Gate : public Effect {
public:
    explicit Gate(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;
    float take_reduction_db() override;

private:
    // How many samples `ms` lasts.
    int samples_in(double ms) const;

    int sample_rate_;

    double threshold_db_ = gate_lowest_threshold_db;
    double range_db_ = gate_silent_range_db;
    // The attack, hold and release as set, in ms.
    double attack_ms_;
    double hold_ms_;
    double release_ms_;
    // How many samples the attack and release take, at least 1, and the
    // hold.
    int attack_samples_ = 1;
    int hold_samples_ = 0;
    int release_samples_ = 1;

    Glide heard_threshold_db_;
    Glide heard_range_db_;

    Listener listener_;

    // How open the gate is, from 0 (closed, down by the range) to 1 (open),
    // and how many samples it still holds open.
    double openness_ = 1.0;
    int hold_left_ = 0;
    double most_reduction_db_ = 0.0;
};
