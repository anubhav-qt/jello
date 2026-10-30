#pragma once

#include <array>

#include "biquad.h"
#include "effect.h"
#include "filter.h"

// The EQ's shapes, one per band.
enum class EqShape {
    low_cut,     // turns everything below the frequency down, by the slope
    low_shelf,   // turns everything below the frequency up or down, by the gain
    bell,        // turns a bump around the frequency up or down, as wide as the width
    high_shelf,  // the low shelf's mirror
    high_cut,    // the low cut's mirror
    notch,       // cuts a narrow band out at the frequency
};

constexpr int eq_shape_count = 6;
constexpr int eq_band_count = 8;

// Each band's values are numbered band * eq_band_values + one of these,
// from band 0. The output gain comes after every band's.
constexpr int eq_band_on = 0;         // 0 or 1
constexpr int eq_band_shape = 1;      // an EqShape
constexpr int eq_band_frequency = 2;  // eq_lowest_hz to eq_highest_hz
constexpr int eq_band_gain = 3;       // dB, -eq_most_gain_db to eq_most_gain_db
constexpr int eq_band_width = 4;      // Q, eq_narrowest_width to eq_widest_width
constexpr int eq_band_slope = 5;      // dB an octave: 12, 24 or 48, for cuts
constexpr int eq_band_values = 6;
constexpr int eq_output = eq_band_count * eq_band_values;  // dB, like the gain

constexpr float eq_lowest_hz = 20.0f;
constexpr float eq_highest_hz = 20000.0f;
constexpr float eq_most_gain_db = 24.0f;
constexpr float eq_narrowest_width = 0.1f;
constexpr float eq_widest_width = 18.0f;

// True if the EQ has value `value` and `number` is within its range.
// The range of value `value`.
ValueRange eq_value_range(int value);

bool is_valid_eq_value(int value, float number);

// One band of the EQ: its shape, made of up to four biquads in a row.
class EqBand {
public:
    EqBand(int sample_rate, float frequency);

    // Sets one of the band's values, numbered from eq_band_on to
    // eq_band_slope. The caller has checked it.
    void set_value(int which, float number);

    // What one of the band's values is set to, for those with a range.
    float value(int which) const;

    // Jumps the band's glides and crossfade to its values (Effect::settle).
    void settle();

    // Puts one moment of sound through the band.
    Stereo next(Stereo input);

private:
    // How many biquads a steepest cut takes.
    static constexpr int most_sections = 4;

    // Works the biquads' numbers out again for the shape, the slope and
    // the values heard now.
    void design(double hz, double gain_db, double width);

    // Starts a crossfade from the shape heard now to a new shape or slope.
    void start_crossfade();

    // Puts `input` through the first `count` of `sections`.
    static Stereo run(std::array<Biquad, most_sections>& sections, int count, Stereo input);

    int sample_rate_;
    double highest_hz_;

    bool on_ = false;
    EqShape shape_ = EqShape::bell;
    int slope_ = 24;
    // The frequency as an octave number (log2 of its Hz), so it glides in
    // equal musical steps.
    // The frequency as an octave number (log2 of its Hz), and as set, in
    // Hz.
    double octave_;
    double frequency_hz_;
    double gain_db_ = 0.0;
    double width_ = 1.0;

    Glide heard_on_;
    Glide heard_octave_;
    Glide heard_gain_db_;
    Glide heard_width_;

    std::array<Biquad, most_sections> sections_;
    int section_count_ = 1;

    // The biquads of the shape before a new one, fading out.
    std::array<Biquad, most_sections> leaving_;
    int leaving_count_ = 0;
    bool crossfading_ = false;
    Glide crossfade_;

    // The values the biquads were last worked out for.
    bool designed_ = false;
    double designed_octave_ = 0.0;
    double designed_gain_db_ = 0.0;
    double designed_width_ = 0.0;
};

// The EQ (docs/design.md#mix-in-the-engine): eq_band_count bands in a row,
// then an output gain. A new EQ has every band off, so it changes nothing.
class Eq : public Effect {
public:
    explicit Eq(int sample_rate);

    void process(float* stereo, int frames, const EffectContext& context) override;
    void set_value(int value, float number) override;
    float value(int value) const override;
    void settle() override;

private:
    std::array<EqBand, eq_band_count> bands_;
    double output_db_ = 0.0;
    Glide heard_output_db_;
};
