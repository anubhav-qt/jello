#pragma once

#include <array>

#include "biquad.h"
#include "effect.h"
#include "filter.h"

// How saturation bends loud sound.
enum class SaturationCurve {
    tape,  // soft: rounds peaks off gently, like tape
    tube,  // warm: rounds one side more than the other, like a tube
    hard,  // hard clip: cuts peaks flat
    fold,  // folds peaks back down, for a buzzy sound
};

constexpr int saturation_curve_count = 4;

// Saturation's values, numbered.
constexpr int saturation_drive = 0;   // dB, 0 to saturation_most_drive_db
constexpr int saturation_curve = 1;   // a SaturationCurve
constexpr int saturation_tone = 2;    // -1 (darker) to 1 (brighter)
constexpr int saturation_output = 3;  // dB, -saturation_most_output_db to saturation_most_output_db
constexpr int saturation_match = 4;   // 0 or 1: 1 matches the input's loudness instead
constexpr int saturation_mix = 5;     // 0 to 1
constexpr int saturation_value_count = 6;

constexpr float saturation_most_drive_db = 36.0f;
constexpr float saturation_most_output_db = 24.0f;

// True if saturation has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange saturation_value_range(int value);

bool is_valid_saturation_value(int value, float number);

// Saturation (docs/design.md#mix-in-the-engine): makes the sound louder by
// the drive, then bends it with a curve, which adds harmonics. Bending
// adds harmonics too high for the samples to hold, and those would fold
// back down as harsh tones that are not in the music (aliasing), so the
// curve is worked out at 4 times the sample rate, with the too-high part
// filtered out before coming back down. Fold adds far more of them, so it
// is worked out at 16 times. A tone control tilts the result
// darker or brighter, then an output gain, or a gain that keeps it as loud
// as the input, and a mix with the dry sound. A new one has no drive.
class Saturation : public Effect {
public:
    explicit Saturation(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    // How many times the sample rate the curves run at: fold, and the
    // others.
    static constexpr int fold_oversampling = 16;
    static constexpr int oversampling = 4;

    // How many biquads each anti-aliasing filter has: 8 make it steep
    // enough that what folds back is far below hearing.
    static constexpr int filter_sections = 8;

    // The filters around the curve at `times` the sample rate: one fills
    // in the samples between, the other takes out what is too high to
    // come back down.
    struct Oversampler {
        int times = 1;
        std::array<Biquad, filter_sections> up;
        std::array<Biquad, filter_sections> down;
    };

    // One moment of sound at the base sample rate through the drive and
    // the curve, crossfading from the curve it is leaving by
    // `curve_share`.
    Stereo bend(Stereo input, double drive_gain, double curve_share);

    // One moment of sound through the drive and `rate`, bent by `from`
    // and `to` mixed by `share`: all `to` at 1.
    Stereo oversampled(Oversampler& rate, Stereo input, double drive_gain, SaturationCurve from,
                       SaturationCurve to, double share);

    // The oversampler `curve` runs in.
    Oversampler& oversampler_of(SaturationCurve curve);

    // Sets `rate`'s filters up for `times` the sample rate.
    void set_up(Oversampler& rate, int times);

    // `value` bent by `curve`.
    double curved(double value, SaturationCurve curve) const;

    int sample_rate_;

    double drive_db_ = 0.0;
    SaturationCurve curve_ = SaturationCurve::tape;
    SaturationCurve leaving_curve_ = SaturationCurve::tape;
    double tone_ = 0.0;
    double output_db_ = 0.0;
    bool match_ = false;
    double mix_ = 1.0;

    Glide heard_drive_db_;
    Glide heard_curve_;
    Glide heard_tone_;
    Glide heard_output_db_;
    Glide heard_match_;
    Glide heard_mix_;

    // Fold's filters, at 16 times the rate, and the other curves', at 4.
    Oversampler fold_rate_;
    Oversampler rate_;

    // A gentle high-pass that takes out the steady push the tube's lopsided
    // curve adds (DC), and the low side of the tone's tilt.
    double dc_share_;
    Stereo dc_in_;
    Stereo dc_out_;
    double tone_share_;
    Stereo tone_low_;

    // How loud the input and the result have been lately, as average
    // powers, for matching.
    double match_share_;
    double input_power_ = 0.0;
    double output_power_ = 0.0;
};
