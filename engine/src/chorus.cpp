#include "chorus.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// The most of the delay the swing reaches either way, so the lateness never
// reaches 0.
constexpr double most_swing = 0.9;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// How many copies copy `index` is placed among: all of them while it is
// heard, and otherwise as if it were the last, so it fades in from where
// it would be added.
int placed_among(int index, int copies) {
    if (index < copies) {
        return copies;
    }
    return index + 1;
}

// Where copy `index` of `copies` sits: spread evenly from fully left to
// fully right at full spread, and a single copy in the middle.
double pan_of(int index, int copies, double spread) {
    if (copies == 1) {
        return 0.0;
    }
    double across = 2.0 * index / (copies - 1) - 1.0;
    return spread * across;
}

// How far round copy `index` of `copies` swings from the first, as a share
// of a turn.
double offset_of(int index, int copies) {
    return static_cast<double>(index) / copies;
}

}  // namespace

ValueRange chorus_value_range(int value) {
    if (value == chorus_copies) {
        return whole_range(1, chorus_most_copies);
    }
    if (value == chorus_speed) {
        return number_range(chorus_slowest_hz, chorus_fastest_hz);
    }
    if (value == chorus_cycle) {
        return number_range(chorus_shortest_cycle, chorus_longest_cycle);
    }
    if (value == chorus_on_beat) {
        return whole_range(0, 1);
    }
    if (value == chorus_depth) {
        return number_range(0.0f, 1.0f);
    }
    if (value == chorus_delay) {
        return number_range(chorus_shortest_delay_ms, chorus_longest_delay_ms);
    }
    if (value == chorus_feedback) {
        return number_range(0.0f, chorus_most_feedback);
    }
    if (value == chorus_spread) {
        return number_range(0.0f, 1.0f);
    }
    if (value == chorus_mix) {
        return number_range(0.0f, 1.0f);
    }
    return ValueRange{};
}

bool is_valid_chorus_value(int value, float number) {
    return is_in_range(chorus_value_range(value), number);
}

Chorus::Chorus(int sample_rate, bool on_bus)
    : Effect(EffectKind::chorus),
      sample_rate_(sample_rate),
      mix_(start_mix(on_bus)),
      heard_speed_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_depth_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_delay_ms_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_feedback_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_on_beat_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_cycle_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      line_(static_cast<int>(chorus_longest_delay_ms * (1.0 + most_swing) * sample_rate / 1000.0) + 1) {
    int glide_samples = samples_in_ms(effect_glide_ms, sample_rate);
    int crossfade_samples = samples_in_ms(effect_crossfade_ms, sample_rate);
    for (int i = 0; i < chorus_most_copies; i++) {
        Copy copy{Glide(crossfade_samples), Glide(glide_samples), Glide(glide_samples)};
        copy_.push_back(copy);
    }
    settle();
}

void Chorus::settle() {
    heard_speed_hz_.jump_to(speed_hz_);
    heard_depth_.jump_to(depth_);
    heard_delay_ms_.jump_to(delay_ms_);
    heard_feedback_.jump_to(feedback_);
    heard_mix_.jump_to(mix_);
    if (on_beat_) {
        heard_on_beat_.jump_to(1.0);
    } else {
        heard_on_beat_.jump_to(0.0);
    }
    heard_cycle_.jump_to(1.0);

    for (int i = 0; i < chorus_most_copies; i++) {
        Copy& copy = copy_[i];
        double level = 0.0;
        if (i < copies_) {
            level = 1.0;
        }
        copy.level.jump_to(level);
        int among = placed_among(i, copies_);
        copy.offset.jump_to(offset_of(i, among));
        copy.pan.jump_to(pan_of(i, among, spread_));
    }
}

void Chorus::set_value(int value, float number) {
    if (!is_valid_chorus_value(value, number)) {
        return;
    }
    if (value == chorus_copies) {
        copies_ = static_cast<int>(number);
    } else if (value == chorus_speed) {
        speed_hz_ = number;
    } else if (value == chorus_cycle) {
        if (number != cycle_) {
            leaving_cycle_ = cycle_;
            cycle_ = number;
            heard_cycle_.jump_to(0.0);
        }
    } else if (value == chorus_on_beat) {
        on_beat_ = number == 1.0f;
    } else if (value == chorus_depth) {
        depth_ = number;
    } else if (value == chorus_delay) {
        delay_ms_ = number;
    } else if (value == chorus_feedback) {
        feedback_ = number;
    } else if (value == chorus_spread) {
        spread_ = number;
    } else if (value == chorus_mix) {
        mix_ = number;
    }
}

float Chorus::value(int value) const {
    if (value == chorus_copies) {
        return static_cast<float>(copies_);
    }
    if (value == chorus_speed) {
        return static_cast<float>(speed_hz_);
    }
    if (value == chorus_cycle) {
        return static_cast<float>(cycle_);
    }
    if (value == chorus_on_beat) {
        return switch_number(on_beat_);
    }
    if (value == chorus_depth) {
        return static_cast<float>(depth_);
    }
    if (value == chorus_delay) {
        return static_cast<float>(delay_ms_);
    }
    if (value == chorus_feedback) {
        return static_cast<float>(feedback_);
    }
    if (value == chorus_spread) {
        return static_cast<float>(spread_);
    }
    if (value == chorus_mix) {
        return static_cast<float>(mix_);
    }
    return 0.0f;
}

double Chorus::swing_at(double turn) {
    return std::sin(2.0 * pi * turn);
}

void Chorus::process(float* stereo, int frames, const EffectContext& context) {
    double on_beat_target = 0.0;
    if (on_beat_) {
        on_beat_target = 1.0;
    }

    for (int i = 0; i < frames; i++) {
        double dry_left = stereo[2 * i];
        double dry_right = stereo[2 * i + 1];

        // Where the swing is: at the speed, or on the beat, once a cycle.
        turn_ += heard_speed_hz_.next(speed_hz_) / sample_rate_;
        turn_ -= std::floor(turn_);
        double beat = context.beat + i * context.beats_per_sample;
        double beat_turn = std::fmod(beat, cycle_) / cycle_;
        double leaving_turn = std::fmod(beat, leaving_cycle_) / leaving_cycle_;
        double on_beat = heard_on_beat_.next(on_beat_target);
        double cycle_share = heard_cycle_.next(1.0);

        double delay_samples = heard_delay_ms_.next(delay_ms_) * sample_rate_ / 1000.0;
        double swing_samples = heard_depth_.next(depth_) * most_swing * delay_samples;

        // Each copy reads the line at its own lateness and takes its place
        // between the speakers, at equal loudness.
        double wet_left = 0.0;
        double wet_right = 0.0;
        double wet_middle = 0.0;
        double levels = 0.0;
        for (int c = 0; c < chorus_most_copies; c++) {
            Copy& copy = copy_[c];
            double level_target = 0.0;
            if (c < copies_) {
                level_target = 1.0;
            }
            double level = copy.level.next(level_target);
            int among = placed_among(c, copies_);
            double offset = copy.offset.next(offset_of(c, among));
            double pan = copy.pan.next(pan_of(c, among, spread_));
            if (level == 0.0) {
                continue;
            }

            double free_swing = swing_at(turn_ + offset);
            double beat_swing = swing_at(beat_turn + offset);
            double leaving_swing = swing_at(leaving_turn + offset);
            beat_swing = leaving_swing + (beat_swing - leaving_swing) * cycle_share;
            double swing = free_swing + (beat_swing - free_swing) * on_beat;

            double heard = line_.read(delay_samples + swing * swing_samples) * level;
            wet_left += heard * std::sqrt(1.0 - pan);
            wet_right += heard * std::sqrt(1.0 + pan);
            wet_middle += heard;
            levels += level;
        }
        wet_left /= levels;
        wet_right /= levels;
        wet_middle /= levels;

        // Both speakers go into the line together, with the copies fed back.
        double feedback = heard_feedback_.next(feedback_);
        line_.write(0.5 * (dry_left + dry_right) + feedback * wet_middle);

        double mix = heard_mix_.next(mix_);
        stereo[2 * i] = static_cast<float>(dry_left + (wet_left - dry_left) * mix);
        stereo[2 * i + 1] = static_cast<float>(dry_right + (wet_right - dry_right) * mix);
    }
}
