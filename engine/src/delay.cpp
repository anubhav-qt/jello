#include "delay.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// How long a new time takes to slide there, in ms, like tape.
constexpr int time_slide_ms = 100;

// How far the wobble swings the time at full wobble, in ms, always later.
constexpr double most_wobble_ms = 5.0;

// How hard full grit drives the echoes into saturation, in dB.
constexpr double most_grit_db = 24.0;

// How fast ducking follows the dry sound up and down, in ms, and how loud
// the dry sound must be to duck the echoes fully, in dB. Quieter dry sound
// ducks them less.
constexpr double duck_rise_ms = 5.0;
constexpr double duck_fall_ms = 250.0;
constexpr double duck_full_db = -24.0;

// Past this the sound going round is rounded off, so even full feedback
// on a held note never runs away. Music never gets this loud.
constexpr double runaway = 2.0;

// The cut filters' sharpness: a plain 12 dB an octave cut.
constexpr double cut_q = 0.7071;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// How far a one-pole follower moves toward its target each sample, to get
// two thirds of the way there in `ms`.
double share_per_sample(double ms, int sample_rate) {
    double samples = ms * sample_rate / 1000.0;
    return 1.0 - std::exp(-1.0 / samples);
}

// `sound` as it goes back round, rounded off past runaway.
double kept_from_running_away(double sound) {
    double size = std::fabs(sound);
    if (size <= runaway) {
        return sound;
    }
    double kept = runaway + std::tanh(size - runaway);
    if (sound < 0.0) {
        return -kept;
    }
    return kept;
}

}  // namespace

ValueRange delay_value_range(int value) {
    if (value == delay_left_ms || value == delay_right_ms) {
        return number_range(delay_shortest_ms, delay_longest_ms);
    }
    if (value == delay_left_beats || value == delay_right_beats) {
        return number_range(delay_shortest_beats, delay_longest_beats);
    }
    if (value == delay_on_beat || value == delay_linked || value == delay_ping_pong) {
        return whole_range(0, 1);
    }
    if (value == delay_low_cut || value == delay_high_cut) {
        return number_range(delay_lowest_hz, delay_highest_hz);
    }
    if (value == delay_wobble_speed) {
        return number_range(delay_slowest_wobble_hz, delay_fastest_wobble_hz);
    }
    if (value == delay_feedback || value == delay_wobble || value == delay_grit) {
        return number_range(0.0f, 1.0f);
    }
    if (value == delay_duck || value == delay_mix) {
        return number_range(0.0f, 1.0f);
    }
    return ValueRange{};
}

bool is_valid_delay_value(int value, float number) {
    return is_in_range(delay_value_range(value), number);
}

Delay::Delay(int sample_rate, bool on_bus)
    : Effect(EffectKind::delay),
      sample_rate_(sample_rate),
      longest_samples_(delay_longest_ms * sample_rate / 1000.0),
      mix_(start_mix(on_bus)),
      heard_left_(samples_in_ms(time_slide_ms, sample_rate)),
      heard_right_(samples_in_ms(time_slide_ms, sample_rate)),
      heard_feedback_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_low_cut_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_high_cut_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_wobble_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_wobble_hz_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_grit_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_duck_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_ping_pong_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      left_line_(static_cast<int>((delay_longest_ms + most_wobble_ms) * sample_rate / 1000.0) + 1),
      right_line_(static_cast<int>((delay_longest_ms + most_wobble_ms) * sample_rate / 1000.0) + 1),
      duck_rise_(share_per_sample(duck_rise_ms, sample_rate)),
      duck_fall_(share_per_sample(duck_fall_ms, sample_rate)) {
    settle();
}

void Delay::settle() {
    // The times jump on the next piece, once the tempo is known.
    started_ = false;
    heard_feedback_.jump_to(feedback_);
    heard_low_cut_hz_.jump_to(low_cut_hz_);
    heard_high_cut_hz_.jump_to(high_cut_hz_);
    heard_wobble_.jump_to(wobble_);
    heard_wobble_hz_.jump_to(wobble_hz_);
    heard_grit_.jump_to(grit_);
    heard_duck_.jump_to(duck_);
    heard_mix_.jump_to(mix_);
    if (ping_pong_) {
        heard_ping_pong_.jump_to(1.0);
    } else {
        heard_ping_pong_.jump_to(0.0);
    }
    set_cuts(low_cut_hz_, high_cut_hz_);
}

void Delay::set_value(int value, float number) {
    if (!is_valid_delay_value(value, number)) {
        return;
    }
    if (value == delay_left_ms) {
        left_ms_ = number;
    } else if (value == delay_right_ms) {
        right_ms_ = number;
    } else if (value == delay_left_beats) {
        left_beats_ = number;
    } else if (value == delay_right_beats) {
        right_beats_ = number;
    } else if (value == delay_on_beat) {
        on_beat_ = number == 1.0f;
    } else if (value == delay_linked) {
        linked_ = number == 1.0f;
    } else if (value == delay_ping_pong) {
        ping_pong_ = number == 1.0f;
    } else if (value == delay_feedback) {
        feedback_ = number;
    } else if (value == delay_low_cut) {
        low_cut_hz_ = number;
    } else if (value == delay_high_cut) {
        high_cut_hz_ = number;
    } else if (value == delay_wobble) {
        wobble_ = number;
    } else if (value == delay_wobble_speed) {
        wobble_hz_ = number;
    } else if (value == delay_grit) {
        grit_ = number;
    } else if (value == delay_duck) {
        duck_ = number;
    } else if (value == delay_mix) {
        mix_ = number;
    }
}

float Delay::value(int value) const {
    if (value == delay_left_ms) {
        return static_cast<float>(left_ms_);
    }
    if (value == delay_right_ms) {
        return static_cast<float>(right_ms_);
    }
    if (value == delay_left_beats) {
        return static_cast<float>(left_beats_);
    }
    if (value == delay_right_beats) {
        return static_cast<float>(right_beats_);
    }
    if (value == delay_on_beat) {
        return switch_number(on_beat_);
    }
    if (value == delay_linked) {
        return switch_number(linked_);
    }
    if (value == delay_ping_pong) {
        return switch_number(ping_pong_);
    }
    if (value == delay_feedback) {
        return static_cast<float>(feedback_);
    }
    if (value == delay_low_cut) {
        return static_cast<float>(low_cut_hz_);
    }
    if (value == delay_high_cut) {
        return static_cast<float>(high_cut_hz_);
    }
    if (value == delay_wobble) {
        return static_cast<float>(wobble_);
    }
    if (value == delay_wobble_speed) {
        return static_cast<float>(wobble_hz_);
    }
    if (value == delay_grit) {
        return static_cast<float>(grit_);
    }
    if (value == delay_duck) {
        return static_cast<float>(duck_);
    }
    if (value == delay_mix) {
        return static_cast<float>(mix_);
    }
    return 0.0f;
}

double Delay::time_in_samples(double ms, double beats, double beats_per_sample) const {
    double samples = ms * sample_rate_ / 1000.0;
    if (on_beat_) {
        if (beats_per_sample > 0.0) {
            samples = beats / beats_per_sample;
        }
    }
    return std::min(samples, longest_samples_);
}

double Delay::gritty(double echo, double grit) {
    if (grit == 0.0) {
        return echo;
    }
    // Driven harder the more grit, and turned back down by as much, so
    // quiet echoes stay as loud and only loud ones round off.
    double drive = gain_of_db(grit * most_grit_db);
    double saturated = std::tanh(echo * drive) / drive;
    return echo + (saturated - echo) * grit;
}

void Delay::set_cuts(double low_cut_hz, double high_cut_hz) {
    low_cut_.set_high_pass(low_cut_hz, cut_q, sample_rate_);
    high_cut_.set_low_pass(high_cut_hz, cut_q, sample_rate_);
    set_low_cut_hz_ = low_cut_hz;
    set_high_cut_hz_ = high_cut_hz;
}

void Delay::process(float* stereo, int frames, const EffectContext& context) {
    double right_ms = right_ms_;
    double right_beats = right_beats_;
    if (linked_) {
        right_ms = left_ms_;
        right_beats = left_beats_;
    }
    double left_target = time_in_samples(left_ms_, left_beats_, context.beats_per_sample);
    double right_target = time_in_samples(right_ms, right_beats, context.beats_per_sample);

    // The times depend on the tempo, known only now, so the first piece
    // starts right on them.
    if (!started_) {
        heard_left_.jump_to(left_target);
        heard_right_.jump_to(right_target);
        started_ = true;
    }

    double ping_pong_target = 0.0;
    if (ping_pong_) {
        ping_pong_target = 1.0;
    }
    double most_wobble_samples = most_wobble_ms * sample_rate_ / 1000.0;
    double duck_full = gain_of_db(duck_full_db);

    for (int i = 0; i < frames; i++) {
        double dry_left = stereo[2 * i];
        double dry_right = stereo[2 * i + 1];

        // The wobble only ever makes the echoes later, so even the
        // shortest time stays reachable.
        wobble_turn_ += heard_wobble_hz_.next(wobble_hz_) / sample_rate_;
        wobble_turn_ -= std::floor(wobble_turn_);
        double swing = 0.5 * (1.0 - std::cos(2.0 * pi * wobble_turn_));
        double later = heard_wobble_.next(wobble_) * most_wobble_samples * swing;

        Stereo echo;
        echo.left = left_line_.read(heard_left_.next(left_target) + later);
        echo.right = right_line_.read(heard_right_.next(right_target) + later);

        double low_cut_hz = heard_low_cut_hz_.next(low_cut_hz_);
        double high_cut_hz = heard_high_cut_hz_.next(high_cut_hz_);
        if (low_cut_hz != set_low_cut_hz_ || high_cut_hz != set_high_cut_hz_) {
            set_cuts(low_cut_hz, high_cut_hz);
        }
        echo = high_cut_.next(low_cut_.next(echo));
        double grit = heard_grit_.next(grit_);
        echo.left = gritty(echo.left, grit);
        echo.right = gritty(echo.right, grit);

        // Straight, each side's echoes go round on their own side. Bouncing,
        // the dry sound starts on the left and each echo crosses over.
        double feedback = heard_feedback_.next(feedback_);
        double straight_left = dry_left + feedback * echo.left;
        double straight_right = dry_right + feedback * echo.right;
        double bounce_left = 0.5 * (dry_left + dry_right) + feedback * echo.right;
        double bounce_right = feedback * echo.left;
        double ping_pong = heard_ping_pong_.next(ping_pong_target);
        double into_left = straight_left + (bounce_left - straight_left) * ping_pong;
        double into_right = straight_right + (bounce_right - straight_right) * ping_pong;
        left_line_.write(kept_from_running_away(into_left));
        right_line_.write(kept_from_running_away(into_right));

        // Ducking: the louder the dry sound lately, the quieter the echoes.
        double dry_peak = std::max(std::fabs(dry_left), std::fabs(dry_right));
        if (dry_peak > dry_level_) {
            dry_level_ += (dry_peak - dry_level_) * duck_rise_;
        } else {
            dry_level_ += (dry_peak - dry_level_) * duck_fall_;
        }
        double playing = std::min(1.0, dry_level_ / duck_full);
        double echo_gain = 1.0 - heard_duck_.next(duck_) * playing;

        double mix = heard_mix_.next(mix_);
        double wet_left = echo.left * echo_gain;
        double wet_right = echo.right * echo_gain;
        stereo[2 * i] = static_cast<float>(dry_left + (wet_left - dry_left) * mix);
        stereo[2 * i + 1] = static_cast<float>(dry_right + (wet_right - dry_right) * mix);
    }
}
