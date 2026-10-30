#include "compressor.h"

#include <algorithm>
#include <cmath>

namespace {

// The attack and release when a compressor is new, in ms, and its ratio.
constexpr double start_attack_ms = 10.0;
constexpr double start_release_ms = 100.0;
constexpr double start_ratio = 4.0;

// The quietest peak it reads, in dB, so silence has a level.
constexpr double quietest_db = -120.0;

// A reduction this small is none.
constexpr double settled_db = 1e-6;

// The low cut's sharpness: a plain 12 dB an octave cut.
constexpr double low_cut_q = 0.7071;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// How far a one-pole follower moves toward its target each sample, to get
// two thirds of the way there in `ms`.
double share_per_sample(double ms, int sample_rate) {
    double samples = ms * sample_rate / 1000.0;
    return 1.0 - std::exp(-1.0 / samples);
}

// The slope for a ratio: how much of each dB past the threshold comes off.
double slope_of(double ratio) {
    if (ratio == compressor_limit) {
        return 1.0;
    }
    return 1.0 - 1.0 / ratio;
}

}  // namespace

ValueRange compressor_value_range(int value) {
    if (value == compressor_threshold) {
        return number_range(compressor_lowest_threshold_db, 0.0f);
    }
    if (value == compressor_ratio) {
        return number_range(1.0f, compressor_most_ratio);
    }
    if (value == compressor_attack) {
        return number_range(compressor_shortest_attack_ms, compressor_longest_attack_ms);
    }
    if (value == compressor_release) {
        return number_range(compressor_shortest_release_ms, compressor_longest_release_ms);
    }
    if (value == compressor_knee) {
        return number_range(0.0f, compressor_widest_knee_db);
    }
    if (value == compressor_makeup) {
        return number_range(0.0f, compressor_most_makeup_db);
    }
    if (value == compressor_mix) {
        return number_range(0.0f, 1.0f);
    }
    if (value == compressor_low_cut) {
        return number_range(compressor_lowest_cut_hz, compressor_highest_cut_hz);
    }
    return ValueRange{};
}

bool is_valid_compressor_value(int value, float number) {
    // The limit is outside the ratio's range, so connections never move it.
    if (value == compressor_ratio) {
        if (number == compressor_limit) {
            return true;
        }
    }
    if (value == compressor_listens_to) {
        return is_valid_listens_to(number);
    }
    return is_in_range(compressor_value_range(value), number);
}

Compressor::Compressor(int sample_rate)
    : Effect(EffectKind::compressor),
      sample_rate_(sample_rate),
      ratio_(start_ratio),
      attack_ms_(start_attack_ms),
      release_ms_(start_release_ms),
      slope_(slope_of(start_ratio)),
      attack_share_(share_per_sample(start_attack_ms, sample_rate)),
      release_share_(share_per_sample(start_release_ms, sample_rate)),
      heard_threshold_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_slope_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_knee_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_makeup_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_low_cut_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      listener_(sample_rate) {
    settle();
}

void Compressor::settle() {
    listener_.settle();
    heard_threshold_db_.jump_to(threshold_db_);
    heard_slope_.jump_to(slope_);
    heard_knee_db_.jump_to(knee_db_);
    heard_makeup_db_.jump_to(makeup_db_);
    heard_mix_.jump_to(mix_);
    heard_low_cut_hz_.jump_to(low_cut_hz_);
    set_low_cut(low_cut_hz_);
}

void Compressor::set_value(int value, float number) {
    if (!is_valid_compressor_value(value, number)) {
        return;
    }
    if (value == compressor_threshold) {
        threshold_db_ = number;
    } else if (value == compressor_ratio) {
        ratio_ = number;
        slope_ = slope_of(number);
    } else if (value == compressor_attack) {
        attack_ms_ = number;
        attack_share_ = share_per_sample(number, sample_rate_);
    } else if (value == compressor_release) {
        release_ms_ = number;
        release_share_ = share_per_sample(number, sample_rate_);
    } else if (value == compressor_knee) {
        knee_db_ = number;
    } else if (value == compressor_makeup) {
        makeup_db_ = number;
    } else if (value == compressor_mix) {
        mix_ = number;
    } else if (value == compressor_listens_to) {
        listener_.set(static_cast<int>(number));
    } else if (value == compressor_low_cut) {
        low_cut_hz_ = number;
    }
}

float Compressor::value(int value) const {
    if (value == compressor_threshold) {
        return static_cast<float>(threshold_db_);
    }
    if (value == compressor_ratio) {
        return static_cast<float>(ratio_);
    }
    if (value == compressor_attack) {
        return static_cast<float>(attack_ms_);
    }
    if (value == compressor_release) {
        return static_cast<float>(release_ms_);
    }
    if (value == compressor_knee) {
        return static_cast<float>(knee_db_);
    }
    if (value == compressor_makeup) {
        return static_cast<float>(makeup_db_);
    }
    if (value == compressor_mix) {
        return static_cast<float>(mix_);
    }
    if (value == compressor_low_cut) {
        return static_cast<float>(low_cut_hz_);
    }
    return 0.0f;
}

float Compressor::take_reduction_db() {
    float most = static_cast<float>(most_reduction_db_);
    most_reduction_db_ = 0.0;
    return most;
}

void Compressor::set_low_cut(double hz) {
    low_cut_.set_high_pass(hz, low_cut_q, sample_rate_);
    set_low_cut_hz_ = hz;
}

double Compressor::wanted_reduction_db(double level_db, double threshold_db, double slope, double knee_db) const {
    double over = level_db - threshold_db;
    double half_knee = knee_db / 2.0;

    // Under the knee: untouched.
    if (over <= -half_knee) {
        return 0.0;
    }

    // In the knee: the slope comes in gradually, so the reduction grows
    // with the square of how far into the knee the peak is.
    if (over < half_knee) {
        double into = over + half_knee;
        return slope * into * into / (2.0 * knee_db);
    }

    // Past it: each dB over loses `slope` dB.
    return slope * over;
}

void Compressor::process(float* stereo, int frames, const EffectContext& context) {
    listener_.start(stereo, context);

    for (int i = 0; i < frames; i++) {
        double low_cut_hz = heard_low_cut_hz_.next(low_cut_hz_);
        if (low_cut_hz != set_low_cut_hz_) {
            set_low_cut(low_cut_hz);
        }
        Stereo heard = low_cut_.next(listener_.next(i));
        double peak = std::max(std::fabs(heard.left), std::fabs(heard.right));
        double level_db = quietest_db;
        if (peak > 0.0) {
            level_db = std::max(quietest_db, 20.0 * std::log10(peak));
        }

        double threshold_db = heard_threshold_db_.next(threshold_db_);
        double slope = heard_slope_.next(slope_);
        double knee_db = heard_knee_db_.next(knee_db_);
        double wanted_db = wanted_reduction_db(level_db, threshold_db, slope, knee_db);

        // Held at each peak and falling over the release, then followed
        // over the attack, so it turns down smoothly and comes back up
        // without wobbling along with each wave.
        held_db_ += (wanted_db - held_db_) * release_share_;
        held_db_ = std::max(held_db_, wanted_db);
        reduction_db_ += (held_db_ - reduction_db_) * attack_share_;

        // Coming back up, it settles at exactly none instead of creeping
        // toward it forever.
        if (reduction_db_ < settled_db) {
            held_db_ = 0.0;
            reduction_db_ = 0.0;
        }
        most_reduction_db_ = std::max(most_reduction_db_, reduction_db_);

        double makeup_db = heard_makeup_db_.next(makeup_db_);
        double gain = gain_of_db(makeup_db - reduction_db_);
        double mix = heard_mix_.next(mix_);
        for (int speaker = 0; speaker < 2; speaker++) {
            double dry = stereo[2 * i + speaker];
            double wet = dry * gain;
            stereo[2 * i + speaker] = static_cast<float>(dry + (wet - dry) * mix);
        }
    }
}
