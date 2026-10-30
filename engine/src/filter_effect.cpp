#include "filter_effect.h"

#include <algorithm>
#include <cmath>

namespace {

// The cutoff never goes past this share of the sample rate, as in the
// voices' filter.
constexpr double highest_share_of_rate = 0.45;

// How much louder the drive makes the sound before rounding it off, at
// full drive: 16 times, 24 dB.
constexpr double most_drive_gain = 16.0;

// The follower's attack and release when a filter is new, in ms.
constexpr double start_attack_ms = 10.0;
constexpr double start_release_ms = 200.0;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// How far a one-pole follower moves toward its target each sample, to get
// two thirds of the way there in `ms`.
double share_per_sample(double ms, int sample_rate) {
    double samples = ms * sample_rate / 1000.0;
    return 1.0 - std::exp(-1.0 / samples);
}

}  // namespace

ValueRange filter_effect_value_range(int value) {
    if (value == filter_effect_type) {
        return whole_range(0, filter_type_count - 1);
    }
    if (value == filter_effect_cutoff) {
        return number_range(filter_lowest_hz, filter_open_hz);
    }
    if (value == filter_effect_resonance) {
        return number_range(0.0f, 1.0f);
    }
    if (value == filter_effect_drive) {
        return number_range(0.0f, 1.0f);
    }
    if (value == filter_effect_follow) {
        return number_range(-filter_most_octaves, filter_most_octaves);
    }
    if (value == filter_effect_follow_attack) {
        return number_range(follow_shortest_attack_ms, follow_longest_attack_ms);
    }
    if (value == filter_effect_follow_release) {
        return number_range(follow_shortest_release_ms, follow_longest_release_ms);
    }
    if (value == filter_effect_mix) {
        return number_range(0.0f, 1.0f);
    }
    return ValueRange{};
}

bool is_valid_filter_effect_value(int value, float number) {
    if (value == filter_effect_slope) {
        if (number == 12.0f) {
            return true;
        }
        return number == 24.0f;
    }
    if (value == filter_effect_listens_to) {
        return is_valid_listens_to(number);
    }
    return is_in_range(filter_effect_value_range(value), number);
}

FilterEffect::FilterEffect(int sample_rate)
    : Effect(EffectKind::filter),
      sample_rate_(sample_rate),
      lowest_octave_(std::log2(static_cast<double>(filter_lowest_hz))),
      highest_octave_(std::log2(highest_share_of_rate * sample_rate)),
      cutoff_(std::log2(static_cast<double>(filter_open_hz))),
      cutoff_hz_(filter_open_hz),
      attack_ms_(start_attack_ms),
      release_ms_(start_release_ms),
      attack_share_(share_per_sample(start_attack_ms, sample_rate)),
      release_share_(share_per_sample(start_release_ms, sample_rate)),
      listener_(sample_rate),
      heard_cutoff_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_resonance_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_drive_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_follow_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_steep_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_share_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_low_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_band_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_high_(samples_in_ms(effect_crossfade_ms, sample_rate)) {
    settle();
}

void FilterEffect::settle() {
    listener_.settle();
    heard_cutoff_.jump_to(cutoff_);
    heard_resonance_.jump_to(resonance_);
    heard_drive_.jump_to(drive_);
    heard_follow_.jump_to(follow_);
    heard_mix_.jump_to(mix_);
    heard_steep_.jump_to(steep_target());
    heard_share_.jump_to(share_target());
    PartMix parts = part_mix_of(type_);
    heard_low_.jump_to(parts.low);
    heard_band_.jump_to(parts.band);
    heard_high_.jump_to(parts.high);
}

void FilterEffect::set_value(int value, float number) {
    if (!is_valid_filter_effect_value(value, number)) {
        return;
    }
    if (value == filter_effect_type) {
        type_ = static_cast<FilterType>(static_cast<int>(number));
    } else if (value == filter_effect_slope) {
        steep_ = number == 24.0f;
    } else if (value == filter_effect_cutoff) {
        cutoff_ = std::log2(static_cast<double>(number));
        cutoff_hz_ = number;
    } else if (value == filter_effect_resonance) {
        resonance_ = number;
    } else if (value == filter_effect_drive) {
        drive_ = number;
    } else if (value == filter_effect_follow) {
        follow_ = number;
    } else if (value == filter_effect_follow_attack) {
        attack_ms_ = number;
        attack_share_ = share_per_sample(number, sample_rate_);
    } else if (value == filter_effect_follow_release) {
        release_ms_ = number;
        release_share_ = share_per_sample(number, sample_rate_);
    } else if (value == filter_effect_mix) {
        mix_ = number;
    } else if (value == filter_effect_listens_to) {
        listener_.set(static_cast<int>(number));
    }
}

float FilterEffect::value(int value) const {
    if (value == filter_effect_type) {
        return static_cast<float>(type_);
    }
    if (value == filter_effect_cutoff) {
        return static_cast<float>(cutoff_hz_);
    }
    if (value == filter_effect_resonance) {
        return static_cast<float>(resonance_);
    }
    if (value == filter_effect_drive) {
        return static_cast<float>(drive_);
    }
    if (value == filter_effect_follow) {
        return static_cast<float>(follow_);
    }
    if (value == filter_effect_follow_attack) {
        return static_cast<float>(attack_ms_);
    }
    if (value == filter_effect_follow_release) {
        return static_cast<float>(release_ms_);
    }
    if (value == filter_effect_mix) {
        return static_cast<float>(mix_);
    }
    return 0.0f;
}

double FilterEffect::follow(double peak) {
    if (peak > loudness_) {
        loudness_ += (peak - loudness_) * attack_share_;
    } else {
        loudness_ += (peak - loudness_) * release_share_;
    }
    return loudness_;
}

double FilterEffect::share_target() const {
    // Open: a low-pass at the highest cutoff that loudness can only open
    // further lets everything through, so the sound is exactly as it was.
    if (type_ == FilterType::low_pass) {
        if (cutoff_ >= std::log2(static_cast<double>(filter_open_hz))) {
            if (follow_ >= 0.0) {
                return 0.0;
            }
        }
    }
    return 1.0;
}

double FilterEffect::steep_target() const {
    if (steep_) {
        return 1.0;
    }
    return 0.0;
}

void FilterEffect::process(float* stereo, int frames, const EffectContext& context) {
    listener_.start(stereo, context);

    PartMix type_parts = part_mix_of(type_);

    for (int i = 0; i < frames; i++) {
        Stereo input;
        input.left = stereo[2 * i];
        input.right = stereo[2 * i + 1];
        Stereo heard = listener_.next(i);
        double loudness = follow(std::max(std::fabs(heard.left), std::fabs(heard.right)));

        // The drive: louder, rounded off, and back down, so quiet sounds
        // keep their level and loud peaks are softened.
        Stereo driven = input;
        double drive = heard_drive_.next(drive_);
        if (drive > 0.0) {
            double gain = 1.0 + drive * (most_drive_gain - 1.0);
            double left = std::tanh(input.left * gain) / gain;
            double right = std::tanh(input.right * gain) / gain;
            driven.left = input.left + (left - input.left) * drive;
            driven.right = input.right + (right - input.right) * drive;
        }

        double cutoff = heard_cutoff_.next(cutoff_) + heard_follow_.next(follow_) * loudness;
        cutoff = std::clamp(cutoff, lowest_octave_, highest_octave_);
        double resonance = heard_resonance_.next(resonance_);
        bool moved = !filters_ready_;
        if (cutoff != set_cutoff_) {
            moved = true;
        }
        if (resonance != set_resonance_) {
            moved = true;
        }
        if (moved) {
            first_.set(std::exp2(cutoff), resonance, sample_rate_);
            second_.set(std::exp2(cutoff), 0.0, sample_rate_);
            filters_ready_ = true;
            set_cutoff_ = cutoff;
            set_resonance_ = resonance;
        }

        PartMix parts;
        parts.low = heard_low_.next(type_parts.low);
        parts.band = heard_band_.next(type_parts.band);
        parts.high = heard_high_.next(type_parts.high);
        Stereo gentle = first_.next(driven, parts);
        Stereo steep = second_.next(gentle, parts);

        double steepness = heard_steep_.next(steep_target());
        double share = heard_share_.next(share_target());
        double mix = heard_mix_.next(mix_);
        for (int speaker = 0; speaker < 2; speaker++) {
            double dry = input.left;
            double before = driven.left;
            double first = gentle.left;
            double second = steep.left;
            if (speaker == 1) {
                dry = input.right;
                before = driven.right;
                first = gentle.right;
                second = steep.right;
            }
            double filtered = first + (second - first) * steepness;
            double wet = before + (filtered - before) * share;
            stereo[2 * i + speaker] = static_cast<float>(dry + (wet - dry) * mix);
        }
    }
}
