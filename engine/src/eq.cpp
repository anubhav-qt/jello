#include "eq.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// No frequency goes past this share of the sample rate, where the biquads'
// math stops behaving.
constexpr double highest_share_of_rate = 0.49;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// How many biquads make a cut of `slope` dB an octave: each gives 12.
int sections_of_slope(int slope) {
    return slope / 12;
}

// The Q of biquad `index` of `count` that together make a Butterworth cut,
// the steepest cut with no bump before it.
double butterworth_q(int index, int count) {
    double order = 2.0 * count;
    double angle = (2.0 * index + 1.0) * pi / (2.0 * order);
    return 1.0 / (2.0 * std::cos(angle));
}

}  // namespace

ValueRange eq_value_range(int value) {
    if (value == eq_output) {
        return number_range(-eq_most_gain_db, eq_most_gain_db);
    }
    if (value < 0) {
        return ValueRange{};
    }
    if (value > eq_output) {
        return ValueRange{};
    }

    int which = value % eq_band_values;
    if (which == eq_band_on) {
        return whole_range(0, 1);
    }
    if (which == eq_band_shape) {
        return whole_range(0, eq_shape_count - 1);
    }
    if (which == eq_band_frequency) {
        return number_range(eq_lowest_hz, eq_highest_hz);
    }
    if (which == eq_band_gain) {
        return number_range(-eq_most_gain_db, eq_most_gain_db);
    }
    if (which == eq_band_width) {
        return number_range(eq_narrowest_width, eq_widest_width);
    }
    return ValueRange{};
}

namespace {

// True for a band's slope, 12, 24 or 48 dB an octave, which has no range.
bool is_valid_eq_slope(int value, float number) {
    if (value < 0) {
        return false;
    }
    if (value >= eq_output) {
        return false;
    }
    if (value % eq_band_values != eq_band_slope) {
        return false;
    }
    if (number == 12.0f) {
        return true;
    }
    if (number == 24.0f) {
        return true;
    }
    return number == 48.0f;
}

}  // namespace

bool is_valid_eq_value(int value, float number) {
    if (is_valid_eq_slope(value, number)) {
        return true;
    }
    return is_in_range(eq_value_range(value), number);
}

EqBand::EqBand(int sample_rate, float frequency)
    : sample_rate_(sample_rate),
      highest_hz_(highest_share_of_rate * sample_rate),
      octave_(std::log2(static_cast<double>(frequency))),
      frequency_hz_(frequency),
      heard_on_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_octave_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_gain_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_width_(samples_in_ms(effect_glide_ms, sample_rate)),
      crossfade_(samples_in_ms(effect_crossfade_ms, sample_rate)) {
    settle();
}

void EqBand::settle() {
    if (on_) {
        heard_on_.jump_to(1.0);
    } else {
        heard_on_.jump_to(0.0);
    }
    heard_octave_.jump_to(octave_);
    heard_gain_db_.jump_to(gain_db_);
    heard_width_.jump_to(width_);
    crossfading_ = false;
}

void EqBand::set_value(int which, float number) {
    if (which == eq_band_on) {
        on_ = number == 1.0f;
    } else if (which == eq_band_shape) {
        EqShape shape = static_cast<EqShape>(static_cast<int>(number));
        if (shape != shape_) {
            start_crossfade();
            shape_ = shape;
        }
    } else if (which == eq_band_frequency) {
        octave_ = std::log2(static_cast<double>(number));
        frequency_hz_ = number;
    } else if (which == eq_band_gain) {
        gain_db_ = number;
    } else if (which == eq_band_width) {
        width_ = number;
    } else if (which == eq_band_slope) {
        int slope = static_cast<int>(number);
        if (slope != slope_) {
            start_crossfade();
            slope_ = slope;
        }
    }
}

float EqBand::value(int which) const {
    if (which == eq_band_on) {
        return switch_number(on_);
    }
    if (which == eq_band_shape) {
        return static_cast<float>(shape_);
    }
    if (which == eq_band_frequency) {
        return static_cast<float>(frequency_hz_);
    }
    if (which == eq_band_gain) {
        return static_cast<float>(gain_db_);
    }
    if (which == eq_band_width) {
        return static_cast<float>(width_);
    }
    return 0.0f;
}

void EqBand::start_crossfade() {
    // The biquads heard now carry on fading out, while the same biquads,
    // memory and all, take the new shape and fade in. Biquads the new shape
    // adds start from silence.
    leaving_ = sections_;
    leaving_count_ = section_count_;
    for (int i = section_count_; i < most_sections; i++) {
        sections_[i].clear();
    }
    crossfading_ = true;
    crossfade_.jump_to(0.0);
    designed_ = false;
}

void EqBand::design(double hz, double gain_db, double width) {
    hz = std::min(hz, highest_hz_);

    if (shape_ == EqShape::low_cut) {
        section_count_ = sections_of_slope(slope_);
        for (int i = 0; i < section_count_; i++) {
            sections_[i].set_high_pass(hz, butterworth_q(i, section_count_), sample_rate_);
        }
    } else if (shape_ == EqShape::high_cut) {
        section_count_ = sections_of_slope(slope_);
        for (int i = 0; i < section_count_; i++) {
            sections_[i].set_low_pass(hz, butterworth_q(i, section_count_), sample_rate_);
        }
    } else if (shape_ == EqShape::low_shelf) {
        section_count_ = 1;
        sections_[0].set_low_shelf(hz, gain_db, width, sample_rate_);
    } else if (shape_ == EqShape::bell) {
        section_count_ = 1;
        sections_[0].set_bell(hz, gain_db, width, sample_rate_);
    } else if (shape_ == EqShape::high_shelf) {
        section_count_ = 1;
        sections_[0].set_high_shelf(hz, gain_db, width, sample_rate_);
    } else {
        section_count_ = 1;
        sections_[0].set_notch(hz, width, sample_rate_);
    }
}

Stereo EqBand::run(std::array<Biquad, most_sections>& sections, int count, Stereo input) {
    Stereo sound = input;
    for (int i = 0; i < count; i++) {
        sound = sections[i].next(sound);
    }
    return sound;
}

Stereo EqBand::next(Stereo input) {
    double on_target = 0.0;
    if (on_) {
        on_target = 1.0;
    }
    double on = heard_on_.next(on_target);

    // Off and done fading out: the band is skipped, and a new shape is
    // simply there when it comes back on.
    if (on == 0.0) {
        if (!on_) {
            crossfading_ = false;
            return input;
        }
    }

    double octave = heard_octave_.next(octave_);
    double gain_db = heard_gain_db_.next(gain_db_);
    double width = heard_width_.next(width_);

    // The biquads' math is only worked out again when a value moves.
    bool moved = !designed_;
    if (octave != designed_octave_) {
        moved = true;
    }
    if (gain_db != designed_gain_db_) {
        moved = true;
    }
    if (width != designed_width_) {
        moved = true;
    }
    if (moved) {
        design(std::exp2(octave), gain_db, width);
        designed_ = true;
        designed_octave_ = octave;
        designed_gain_db_ = gain_db;
        designed_width_ = width;
    }

    Stereo shaped = run(sections_, section_count_, input);
    if (crossfading_) {
        double share = crossfade_.next(1.0);
        Stereo leaving = run(leaving_, leaving_count_, input);
        shaped.left = leaving.left + (shaped.left - leaving.left) * share;
        shaped.right = leaving.right + (shaped.right - leaving.right) * share;
        if (share == 1.0) {
            crossfading_ = false;
        }
    }

    Stereo output;
    output.left = input.left + (shaped.left - input.left) * on;
    output.right = input.right + (shaped.right - input.right) * on;
    return output;
}

Eq::Eq(int sample_rate)
    : Effect(EffectKind::eq),
      bands_{EqBand(sample_rate, 40.0f),   EqBand(sample_rate, 100.0f),  EqBand(sample_rate, 250.0f),
             EqBand(sample_rate, 600.0f),  EqBand(sample_rate, 1500.0f), EqBand(sample_rate, 3500.0f),
             EqBand(sample_rate, 8000.0f), EqBand(sample_rate, 16000.0f)},
      heard_output_db_(samples_in_ms(effect_glide_ms, sample_rate)) {
    settle();
}

void Eq::settle() {
    for (EqBand& band : bands_) {
        band.settle();
    }
    heard_output_db_.jump_to(output_db_);
}

void Eq::set_value(int value, float number) {
    if (!is_valid_eq_value(value, number)) {
        return;
    }
    if (value == eq_output) {
        output_db_ = number;
        return;
    }
    int band = value / eq_band_values;
    int which = value % eq_band_values;
    bands_[band].set_value(which, number);
}

float Eq::value(int value) const {
    if (value == eq_output) {
        return static_cast<float>(output_db_);
    }
    if (value < 0) {
        return 0.0f;
    }
    if (value > eq_output) {
        return 0.0f;
    }
    int band = value / eq_band_values;
    int which = value % eq_band_values;
    return bands_[band].value(which);
}

void Eq::process(float* stereo, int frames, const EffectContext& context) {
    // The EQ hears only its own sound.
    (void)context;

    for (int i = 0; i < frames; i++) {
        Stereo sound;
        sound.left = stereo[2 * i];
        sound.right = stereo[2 * i + 1];
        for (EqBand& band : bands_) {
            sound = band.next(sound);
        }

        double output_db = heard_output_db_.next(output_db_);
        double gain = 1.0;
        if (output_db != 0.0) {
            gain = gain_of_db(output_db);
        }
        stereo[2 * i] = static_cast<float>(sound.left * gain);
        stereo[2 * i + 1] = static_cast<float>(sound.right * gain);
    }
}
