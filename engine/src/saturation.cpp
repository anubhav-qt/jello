#include "saturation.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// The anti-aliasing filters' cutoff, as a share of the base sample rate:
// 21,600 Hz at 48,000, just above the highest pitch anyone hears.
constexpr double anti_alias_share = 0.45;

// How far the tube's curve is pushed to one side, which makes it round one
// side of the wave more than the other.
constexpr double tube_lean = 0.3;

// The tone control's tilt: it turns the highs up and the lows down by up
// to this many dB, or the other way round, either side of this frequency.
constexpr double tone_most_db = 6.0;
constexpr double tone_hz = 800.0;

// The high-pass that takes the steady push out, and how long the matching
// listens back, in ms.
constexpr double dc_hz = 10.0;
constexpr double match_ms = 300.0;

int samples_in_ms(int ms, int sample_rate) {
    return sample_rate * ms / 1000;
}

// The Q of biquad `index` of `count` that together make a Butterworth
// low-pass, the steepest with no bump before its cutoff.
double butterworth_q(int index, int count) {
    double order = 2.0 * count;
    double angle = (2.0 * index + 1.0) * pi / (2.0 * order);
    return 1.0 / (2.0 * std::cos(angle));
}

// How far a one-pole filter moves toward its input each sample for a
// cutoff of `hz`.
double one_pole_share(double hz, int sample_rate) {
    return 1.0 - std::exp(-2.0 * pi * hz / sample_rate);
}

}  // namespace

ValueRange saturation_value_range(int value) {
    if (value == saturation_drive) {
        return number_range(0.0f, saturation_most_drive_db);
    }
    if (value == saturation_curve) {
        return whole_range(0, saturation_curve_count - 1);
    }
    if (value == saturation_tone) {
        return number_range(-1.0f, 1.0f);
    }
    if (value == saturation_output) {
        return number_range(-saturation_most_output_db, saturation_most_output_db);
    }
    if (value == saturation_match) {
        return whole_range(0, 1);
    }
    if (value == saturation_mix) {
        return number_range(0.0f, 1.0f);
    }
    return ValueRange{};
}

bool is_valid_saturation_value(int value, float number) {
    return is_in_range(saturation_value_range(value), number);
}

Saturation::Saturation(int sample_rate)
    : Effect(EffectKind::saturation),
      sample_rate_(sample_rate),
      heard_drive_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_curve_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_tone_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_output_db_(samples_in_ms(effect_glide_ms, sample_rate)),
      heard_match_(samples_in_ms(effect_crossfade_ms, sample_rate)),
      heard_mix_(samples_in_ms(effect_glide_ms, sample_rate)),
      dc_share_(one_pole_share(dc_hz, sample_rate)),
      tone_share_(one_pole_share(tone_hz, sample_rate)),
      match_share_(1.0 - std::exp(-1000.0 / (match_ms * sample_rate))) {
    settle();
    set_up(fold_rate_, fold_oversampling);
    set_up(rate_, oversampling);
}

void Saturation::settle() {
    heard_drive_db_.jump_to(drive_db_);
    heard_curve_.jump_to(1.0);
    heard_tone_.jump_to(tone_);
    heard_output_db_.jump_to(output_db_);
    if (match_) {
        heard_match_.jump_to(1.0);
    } else {
        heard_match_.jump_to(0.0);
    }
    heard_mix_.jump_to(mix_);
}

void Saturation::set_up(Oversampler& rate, int times) {
    rate.times = times;
    int high_rate = sample_rate_ * times;
    double cutoff = anti_alias_share * sample_rate_;
    for (int i = 0; i < filter_sections; i++) {
        rate.up[i].set_low_pass(cutoff, butterworth_q(i, filter_sections), high_rate);
        rate.down[i].set_low_pass(cutoff, butterworth_q(i, filter_sections), high_rate);
    }
}

Saturation::Oversampler& Saturation::oversampler_of(SaturationCurve curve) {
    if (curve == SaturationCurve::fold) {
        return fold_rate_;
    }
    return rate_;
}

void Saturation::set_value(int value, float number) {
    if (!is_valid_saturation_value(value, number)) {
        return;
    }
    if (value == saturation_drive) {
        drive_db_ = number;
    } else if (value == saturation_curve) {
        SaturationCurve curve = static_cast<SaturationCurve>(static_cast<int>(number));
        if (curve != curve_) {
            leaving_curve_ = curve_;
            curve_ = curve;
            heard_curve_.jump_to(0.0);
        }
    } else if (value == saturation_tone) {
        tone_ = number;
    } else if (value == saturation_output) {
        output_db_ = number;
    } else if (value == saturation_match) {
        match_ = number == 1.0f;
    } else if (value == saturation_mix) {
        mix_ = number;
    }
}

float Saturation::value(int value) const {
    if (value == saturation_drive) {
        return static_cast<float>(drive_db_);
    }
    if (value == saturation_curve) {
        return static_cast<float>(curve_);
    }
    if (value == saturation_tone) {
        return static_cast<float>(tone_);
    }
    if (value == saturation_output) {
        return static_cast<float>(output_db_);
    }
    if (value == saturation_match) {
        return switch_number(match_);
    }
    if (value == saturation_mix) {
        return static_cast<float>(mix_);
    }
    return 0.0f;
}

double Saturation::curved(double value, SaturationCurve curve) const {
    if (curve == SaturationCurve::tape) {
        return std::tanh(value);
    }
    if (curve == SaturationCurve::tube) {
        return std::tanh(value + tube_lean) - std::tanh(tube_lean);
    }
    if (curve == SaturationCurve::hard) {
        return std::clamp(value, -1.0, 1.0);
    }

    // Fold: past 1 it turns back down, past -1 back up, again and again,
    // like a triangle wave 4 wide.
    double place = (value + 1.0) / 4.0;
    place -= std::floor(place);
    if (place < 0.5) {
        return 4.0 * place - 1.0;
    }
    return 3.0 - 4.0 * place;
}

Stereo Saturation::bend(Stereo input, double drive_gain, double curve_share) {
    Oversampler& arriving = oversampler_of(curve_);
    if (curve_share >= 1.0) {
        return oversampled(arriving, input, drive_gain, curve_, curve_, 1.0);
    }

    // Both curves at one rate: mixed before its filters.
    Oversampler& leaving = oversampler_of(leaving_curve_);
    if (&leaving == &arriving) {
        return oversampled(arriving, input, drive_gain, leaving_curve_, curve_, curve_share);
    }

    // At different rates: each at its own, mixed after.
    Stereo arrived = oversampled(arriving, input, drive_gain, curve_, curve_, 1.0);
    Stereo left = oversampled(leaving, input, drive_gain, leaving_curve_, leaving_curve_, 1.0);
    Stereo output;
    output.left = left.left + (arrived.left - left.left) * curve_share;
    output.right = left.right + (arrived.right - left.right) * curve_share;
    return output;
}

Stereo Saturation::oversampled(Oversampler& rate, Stereo input, double drive_gain, SaturationCurve from,
                               SaturationCurve to, double share) {
    Stereo output;
    for (int step = 0; step < rate.times; step++) {
        // The base rate's sample, then silence between, made as many times
        // louder as there are samples, so the filled-in wave keeps its
        // level.
        Stereo sound;
        if (step == 0) {
            sound.left = input.left * rate.times;
            sound.right = input.right * rate.times;
        }
        for (Biquad& section : rate.up) {
            sound = section.next(sound);
        }

        Stereo bent;
        bent.left = curved(sound.left * drive_gain, to);
        bent.right = curved(sound.right * drive_gain, to);
        if (share < 1.0) {
            double left = curved(sound.left * drive_gain, from);
            double right = curved(sound.right * drive_gain, from);
            bent.left = left + (bent.left - left) * share;
            bent.right = right + (bent.right - right) * share;
        }

        for (Biquad& section : rate.down) {
            bent = section.next(bent);
        }

        // Back at the base rate, only the last sample of each run is kept.
        if (step == rate.times - 1) {
            output = bent;
        }
    }
    return output;
}

void Saturation::process(float* stereo, int frames, const EffectContext& context) {
    // Saturation hears only its own sound.
    (void)context;

    double match_target = 0.0;
    if (match_) {
        match_target = 1.0;
    }

    for (int i = 0; i < frames; i++) {
        Stereo input;
        input.left = stereo[2 * i];
        input.right = stereo[2 * i + 1];

        double drive_gain = gain_of_db(heard_drive_db_.next(drive_db_));
        double curve_share = heard_curve_.next(1.0);
        Stereo bent = bend(input, drive_gain, curve_share);

        // The steady push out.
        Stereo steady;
        steady.left = bent.left - dc_in_.left + (1.0 - dc_share_) * dc_out_.left;
        steady.right = bent.right - dc_in_.right + (1.0 - dc_share_) * dc_out_.right;
        dc_in_ = bent;
        dc_out_ = steady;

        // The tone: the lows and highs either side of tone_hz, one turned
        // up and the other down. At 0 they add back up to the sound as it
        // was.
        tone_low_.left += (steady.left - tone_low_.left) * tone_share_;
        tone_low_.right += (steady.right - tone_low_.right) * tone_share_;
        double tilt = gain_of_db(heard_tone_.next(tone_) * tone_most_db);
        Stereo toned;
        toned.left = tone_low_.left / tilt + (steady.left - tone_low_.left) * tilt;
        toned.right = tone_low_.right / tilt + (steady.right - tone_low_.right) * tilt;

        // The output gain, or the gain that makes the result as loud as the
        // input has been lately.
        double input_power = input.left * input.left + input.right * input.right;
        double output_power = toned.left * toned.left + toned.right * toned.right;
        input_power_ += (input_power - input_power_) * match_share_;
        output_power_ += (output_power - output_power_) * match_share_;
        double matched = 1.0;
        if (output_power_ > 1e-12) {
            matched = std::sqrt(input_power_ / output_power_);
        }
        double most = gain_of_db(saturation_most_output_db);
        matched = std::clamp(matched, 1.0 / most, most);
        double output_gain = gain_of_db(heard_output_db_.next(output_db_));
        double gain = output_gain + (matched - output_gain) * heard_match_.next(match_target);

        double mix = heard_mix_.next(mix_);
        double wet_left = toned.left * gain;
        double wet_right = toned.right * gain;
        stereo[2 * i] = static_cast<float>(input.left + (wet_left - input.left) * mix);
        stereo[2 * i + 1] = static_cast<float>(input.right + (wet_right - input.right) * mix);
    }
}
