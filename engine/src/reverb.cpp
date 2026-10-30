#include "reverb.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// Each delay's length at full size, in ms: all different and sharing no
// rhythm, so the echoes never line up into a buzz.
constexpr std::array<double, 8> full_lengths_ms = {53.1, 59.3, 64.7, 71.3, 76.9, 83.1, 89.7, 97.3};

// The share of those lengths at size 0: a small room.
constexpr double smallest_scale = 0.15;

// How fast each delay's swing goes, in Hz, and how far it reaches at full
// modulation, in ms.
constexpr std::array<double, 8> swing_hz = {0.31, 0.37, 0.43, 0.47, 0.53, 0.59, 0.67, 0.71};
constexpr double most_swing_ms = 0.8;

// The all-passes' lengths, in ms, a little different for each side, and
// how much each smears at full diffusion.
constexpr std::array<double, 4> left_smear_ms = {4.77, 3.59, 12.73, 9.31};
constexpr std::array<double, 4> right_smear_ms = {4.97, 3.71, 12.31, 9.67};
constexpr double most_smear = 0.7;

// How long the size takes to slide to a new one, in ms.
constexpr int size_slide_ms = 200;

// How loud the sound goes into the network and comes out of it, so the
// tail of a held chord is about as loud as the chord.
constexpr double input_gain = 0.5;
constexpr double output_gain = 0.32;

// How much a level falls over the decay: 60 dB, a thousandth.
constexpr double decay_db = 60.0;

// The low cut's sharpness: a plain 12 dB an octave cut.
constexpr double low_cut_q = 0.7071;

// Under this the damping's memory is kept as silence (denormals).
constexpr double tiniest = 1e-20;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

int samples_in(double ms, int sample_rate) {
    return static_cast<int>(ms * sample_rate / 1000.0);
}

// How far a one-pole filter moves toward its input each sample for a
// cutoff of `hz`.
double one_pole_share(double hz, int sample_rate) {
    return 1.0 - std::exp(-2.0 * pi * hz / sample_rate);
}

// Mixes the eight delays' sounds into each other, in place, so each comes
// out of all of them: a Hadamard mix, which keeps the loudness exactly.
void mix_together(std::array<double, 8>& sounds) {
    for (int half = 1; half < 8; half *= 2) {
        for (int start = 0; start < 8; start += 2 * half) {
            for (int i = start; i < start + half; i++) {
                double first = sounds[i];
                double second = sounds[i + half];
                sounds[i] = first + second;
                sounds[i + half] = first - second;
            }
        }
    }
    double keep_loudness = 1.0 / std::sqrt(8.0);
    for (double& sound : sounds) {
        sound *= keep_loudness;
    }
}

}  // namespace

ValueRange reverb_value_range(int value) {
    if (value == reverb_size) {
        return number_range(0.0f, 1.0f);
    }
    if (value == reverb_decay) {
        return number_range(reverb_shortest_decay, reverb_longest_decay);
    }
    if (value == reverb_pre_delay) {
        return number_range(0.0f, reverb_longest_pre_delay_ms);
    }
    if (value == reverb_damping) {
        return number_range(reverb_lowest_damping_hz, reverb_highest_damping_hz);
    }
    if (value == reverb_low_cut) {
        return number_range(reverb_lowest_cut_hz, reverb_highest_cut_hz);
    }
    if (value == reverb_freeze) {
        return whole_range(0, 1);
    }
    if (value == reverb_diffusion || value == reverb_modulation) {
        return number_range(0.0f, 1.0f);
    }
    if (value == reverb_width || value == reverb_mix) {
        return number_range(0.0f, 1.0f);
    }
    return ValueRange{};
}

bool is_valid_reverb_value(int value, float number) {
    return is_in_range(reverb_value_range(value), number);
}

Reverb::Reverb(int sample_rate, bool on_bus)
    : Effect(EffectKind::reverb),
      sample_rate_(sample_rate),
      mix_(start_mix(on_bus)),
      heard_size_(samples_in_ms(size_slide_ms, sample_rate)),
      heard_decay_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_pre_delay_ms_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_damping_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_low_cut_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_diffusion_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_modulation_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_width_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_freeze_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      pre_left_(samples_in(reverb_longest_pre_delay_ms, sample_rate) + 3),
      pre_right_(samples_in(reverb_longest_pre_delay_ms, sample_rate) + 3) {
    settle();

    for (int i = 0; i < smear_count; i++) {
        int left_length = samples_in(left_smear_ms[i], sample_rate);
        int right_length = samples_in(right_smear_ms[i], sample_rate);
        left_smears_.push_back(Smear{DelayLine(left_length), left_length});
        right_smears_.push_back(Smear{DelayLine(right_length), right_length});
    }

    double most_swing_samples = most_swing_ms * sample_rate / 1000.0;
    for (int i = 0; i < line_count; i++) {
        full_lengths_[i] = full_lengths_ms[i] * sample_rate / 1000.0;
        int longest = static_cast<int>(full_lengths_[i] + most_swing_samples) + 1;
        lines_.push_back(DelayLine(longest));
    }
    set_low_cut(low_cut_hz_);
}

void Reverb::settle() {
    heard_size_.jump_to(size_);
    heard_decay_.jump_to(decay_);
    heard_pre_delay_ms_.jump_to(pre_delay_ms_);
    heard_damping_hz_.jump_to(damping_hz_);
    heard_low_cut_hz_.jump_to(low_cut_hz_);
    heard_diffusion_.jump_to(diffusion_);
    heard_modulation_.jump_to(modulation_);
    heard_width_.jump_to(width_);
    if (freeze_) {
        heard_freeze_.jump_to(1.0);
    } else {
        heard_freeze_.jump_to(0.0);
    }
    heard_mix_.jump_to(mix_);
}

void Reverb::set_value(int value, float number) {
    if (!is_valid_reverb_value(value, number)) {
        return;
    }
    if (value == reverb_size) {
        size_ = number;
    } else if (value == reverb_decay) {
        decay_ = number;
    } else if (value == reverb_pre_delay) {
        pre_delay_ms_ = number;
    } else if (value == reverb_damping) {
        damping_hz_ = number;
    } else if (value == reverb_low_cut) {
        low_cut_hz_ = number;
    } else if (value == reverb_diffusion) {
        diffusion_ = number;
    } else if (value == reverb_modulation) {
        modulation_ = number;
    } else if (value == reverb_width) {
        width_ = number;
    } else if (value == reverb_freeze) {
        freeze_ = number == 1.0f;
    } else if (value == reverb_mix) {
        mix_ = number;
    }
}

float Reverb::value(int value) const {
    if (value == reverb_size) {
        return static_cast<float>(size_);
    }
    if (value == reverb_decay) {
        return static_cast<float>(decay_);
    }
    if (value == reverb_pre_delay) {
        return static_cast<float>(pre_delay_ms_);
    }
    if (value == reverb_damping) {
        return static_cast<float>(damping_hz_);
    }
    if (value == reverb_low_cut) {
        return static_cast<float>(low_cut_hz_);
    }
    if (value == reverb_diffusion) {
        return static_cast<float>(diffusion_);
    }
    if (value == reverb_modulation) {
        return static_cast<float>(modulation_);
    }
    if (value == reverb_width) {
        return static_cast<float>(width_);
    }
    if (value == reverb_freeze) {
        return switch_number(freeze_);
    }
    if (value == reverb_mix) {
        return static_cast<float>(mix_);
    }
    return 0.0f;
}

double Reverb::smeared(double sound, std::vector<Smear>& smears, double share) {
    for (Smear& smear : smears) {
        double delayed = smear.line.read(smear.length);
        double kept = sound + share * delayed;
        smear.line.write(kept);
        sound = delayed - share * kept;
    }
    return sound;
}

void Reverb::set_fades(double scale, double decay) {
    // A delay `seconds` long goes round decay / seconds times over the
    // decay, so each time round it loses that share of the 60 dB.
    for (int i = 0; i < line_count; i++) {
        double seconds = full_lengths_[i] * scale / sample_rate_;
        fades_[i] = gain_of_db(-decay_db * seconds / decay);
    }
    set_scale_ = scale;
    set_decay_ = decay;
}

void Reverb::set_low_cut(double hz) {
    low_cut_.set_high_pass(hz, low_cut_q, sample_rate_);
    set_low_cut_hz_ = hz;
}

void Reverb::process(float* stereo, int frames, const EffectContext& context) {
    // The reverb hears only its own sound.
    (void)context;

    double freeze_target = 0.0;
    if (freeze_) {
        freeze_target = 1.0;
    }
    double most_swing_samples = most_swing_ms * sample_rate_ / 1000.0;

    for (int i = 0; i < frames; i++) {
        double dry_left = stereo[2 * i];
        double dry_right = stereo[2 * i + 1];
        double freeze = heard_freeze_.next(freeze_target);

        // Held back by the pre-delay, on top of the line's shortest reach,
        // then smeared. Frozen, nothing new comes in.
        double pre_delay = heard_pre_delay_ms_.next(pre_delay_ms_) * sample_rate_ / 1000.0;
        pre_delay += DelayLine::shortest;
        double held_left = pre_left_.read(pre_delay);
        double held_right = pre_right_.read(pre_delay);
        pre_left_.write(dry_left);
        pre_right_.write(dry_right);
        double smear = heard_diffusion_.next(diffusion_) * most_smear;
        double into_left = smeared(held_left, left_smears_, smear) * input_gain * (1.0 - freeze);
        double into_right = smeared(held_right, right_smears_, smear) * input_gain * (1.0 - freeze);

        double scale = smallest_scale + (1.0 - smallest_scale) * heard_size_.next(size_);
        double decay = heard_decay_.next(decay_);
        if (scale != set_scale_ || decay != set_decay_) {
            set_fades(scale, decay);
        }
        double damping_hz = heard_damping_hz_.next(damping_hz_);
        if (damping_hz != set_damping_hz_) {
            damping_share_ = one_pole_share(damping_hz, sample_rate_);
            set_damping_hz_ = damping_hz;
        }
        double swing_samples = heard_modulation_.next(modulation_) * most_swing_samples;

        // Each delay's sound, damped and faded, then all mixed into each
        // other and back in. Frozen, nothing fades or darkens.
        std::array<double, line_count> taps{};
        std::array<double, line_count> round{};
        for (int d = 0; d < line_count; d++) {
            turns_[d] += swing_hz[d] / sample_rate_;
            turns_[d] -= std::floor(turns_[d]);
            double swing = std::sin(2.0 * pi * turns_[d]) * swing_samples;
            taps[d] = lines_[d].read(full_lengths_[d] * scale + swing);

            damped_[d] += (taps[d] - damped_[d]) * damping_share_;
            if (std::fabs(damped_[d]) < tiniest) {
                damped_[d] = 0.0;
            }
            double darkened = damped_[d] + (taps[d] - damped_[d]) * freeze;
            double fade = fades_[d] + (1.0 - fades_[d]) * freeze;
            round[d] = darkened * fade;
        }
        mix_together(round);
        for (int d = 0; d < line_count; d++) {
            double into = into_right;
            if (d % 2 == 0) {
                into = into_left;
            }
            lines_[d].write(round[d] + into);
        }

        // Two different mixes of the delays for the two sides, so they
        // sound apart, then narrowed toward their middle by the width.
        double wet_left = 0.0;
        double wet_right = 0.0;
        for (int d = 0; d < line_count; d++) {
            double left_sign = 1.0;
            if (d % 2 == 1) {
                left_sign = -1.0;
            }
            double right_sign = 1.0;
            if (d % 4 >= 2) {
                right_sign = -1.0;
            }
            wet_left += taps[d] * left_sign;
            wet_right += taps[d] * right_sign;
        }
        double middle = 0.5 * (wet_left + wet_right) * output_gain;
        double side = 0.5 * (wet_left - wet_right) * output_gain * heard_width_.next(width_);

        double low_cut_hz = heard_low_cut_hz_.next(low_cut_hz_);
        if (low_cut_hz != set_low_cut_hz_) {
            set_low_cut(low_cut_hz);
        }
        Stereo wet = low_cut_.next(Stereo{middle + side, middle - side});

        double mix = heard_mix_.next(mix_);
        stereo[2 * i] = static_cast<float>(dry_left + (wet.left - dry_left) * mix);
        stereo[2 * i + 1] = static_cast<float>(dry_right + (wet.right - dry_right) * mix);
    }
}
