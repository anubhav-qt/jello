#include "gate.h"

#include <algorithm>
#include <cmath>

namespace {

// The attack, hold and release when a gate is new, in ms. The hold covers
// a whole wave of 50 Hz, so a held tone never flickers it.
constexpr double start_attack_ms = 1.0;
constexpr double start_hold_ms = 20.0;
constexpr double start_release_ms = 100.0;

// How far down a gate closed to silence reads, in dB.
constexpr double silent_reduction_db = -gate_silent_range_db;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

}  // namespace

ValueRange gate_value_range(int value) {
    if (value == gate_threshold) {
        return number_range(gate_lowest_threshold_db, 0.0f);
    }
    if (value == gate_range) {
        return number_range(gate_silent_range_db, 0.0f);
    }
    if (value == gate_attack) {
        return number_range(gate_shortest_attack_ms, gate_longest_attack_ms);
    }
    if (value == gate_hold) {
        return number_range(0.0f, gate_longest_hold_ms);
    }
    if (value == gate_release) {
        return number_range(gate_shortest_release_ms, gate_longest_release_ms);
    }
    return ValueRange{};
}

bool is_valid_gate_value(int value, float number) {
    if (value == gate_listens_to) {
        return is_valid_listens_to(number);
    }
    return is_in_range(gate_value_range(value), number);
}

Gate::Gate(int sample_rate)
    : Effect(EffectKind::gate),
      sample_rate_(sample_rate),
      heard_threshold_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_range_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      listener_(sample_rate) {
    settle();
    attack_ms_ = start_attack_ms;
    hold_ms_ = start_hold_ms;
    release_ms_ = start_release_ms;
    attack_samples_ = std::max(1, samples_in(attack_ms_));
    hold_samples_ = samples_in(hold_ms_);
    release_samples_ = std::max(1, samples_in(release_ms_));
}

void Gate::settle() {
    listener_.settle();
    heard_threshold_db_.jump_to(threshold_db_);
    heard_range_db_.jump_to(range_db_);
}

int Gate::samples_in(double ms) const {
    return static_cast<int>(std::lround(ms * sample_rate_ / 1000.0));
}

void Gate::set_value(int value, float number) {
    if (!is_valid_gate_value(value, number)) {
        return;
    }
    if (value == gate_threshold) {
        threshold_db_ = number;
    } else if (value == gate_range) {
        range_db_ = number;
    } else if (value == gate_attack) {
        attack_ms_ = number;
        attack_samples_ = std::max(1, samples_in(number));
    } else if (value == gate_hold) {
        hold_ms_ = number;
        hold_samples_ = samples_in(number);
    } else if (value == gate_release) {
        release_ms_ = number;
        release_samples_ = std::max(1, samples_in(number));
    } else if (value == gate_listens_to) {
        listener_.set(static_cast<int>(number));
    }
}

float Gate::value(int value) const {
    if (value == gate_threshold) {
        return static_cast<float>(threshold_db_);
    }
    if (value == gate_range) {
        return static_cast<float>(range_db_);
    }
    if (value == gate_attack) {
        return static_cast<float>(attack_ms_);
    }
    if (value == gate_hold) {
        return static_cast<float>(hold_ms_);
    }
    if (value == gate_release) {
        return static_cast<float>(release_ms_);
    }
    return 0.0f;
}

float Gate::take_reduction_db() {
    float most = static_cast<float>(most_reduction_db_);
    most_reduction_db_ = 0.0;
    return most;
}

void Gate::process(float* stereo, int frames, const EffectContext& context) {
    listener_.start(stereo, context);

    for (int i = 0; i < frames; i++) {
        Stereo heard = listener_.next(i);
        double peak = std::max(std::fabs(heard.left), std::fabs(heard.right));
        double threshold_db = heard_threshold_db_.next(threshold_db_);

        // Open while peaks reach the threshold and through the hold after
        // the last one. At the lowest threshold it never closes.
        bool open = false;
        if (threshold_db_ <= gate_lowest_threshold_db) {
            open = true;
        } else if (peak >= gain_of_db(threshold_db)) {
            open = true;
            hold_left_ = hold_samples_;
        } else if (hold_left_ > 0) {
            open = true;
            hold_left_ -= 1;
        }

        if (open) {
            openness_ = std::min(1.0, openness_ + 1.0 / attack_samples_);
        } else {
            openness_ = std::max(0.0, openness_ - 1.0 / release_samples_);
        }

        // Closed is down by the range, or silent at its lowest.
        double range_db = heard_range_db_.next(range_db_);
        double closed = 0.0;
        if (range_db > gate_silent_range_db) {
            closed = gain_of_db(range_db);
        }
        double gain = closed + (1.0 - closed) * openness_;

        double reduction_db = silent_reduction_db;
        if (gain > 0.0) {
            reduction_db = std::min(silent_reduction_db, -20.0 * std::log10(gain));
        }
        most_reduction_db_ = std::max(most_reduction_db_, reduction_db);

        stereo[2 * i] = static_cast<float>(stereo[2 * i] * gain);
        stereo[2 * i + 1] = static_cast<float>(stereo[2 * i + 1] * gain);
    }
}
