#include "amp.h"

#include <cmath>

namespace {

// How much louder each preamp stage hits its curve at full gain, in dB.
constexpr double first_stage_most_db = 34.0;
constexpr double second_stage_most_db = 20.0;

// A bent wave stays near full height however hard it is pushed, so more gain
// is turned down by this much, in dB, to stay about as loud.
constexpr double gain_trim_most_db = 28.0;

// How far the tone knobs reach either way, in dB.
constexpr double tone_most_db = 12.0;

// The speaker's top falls away above this many Hz, by its knob.
constexpr double speaker_top_lowest_hz = 2500.0;
constexpr double speaker_top_highest_hz = 8000.0;

// The volume knob at full, as a gain.
constexpr double most_volume = 2.5;

// Butterworth's two sections, for a steep speaker top with no bump.
constexpr double top_q_a = 0.54;
constexpr double top_q_b = 1.31;

double db_of_knob(float knob) {
    return (static_cast<double>(knob) - 0.5) * 2.0 * tone_most_db;
}

void run(Biquad& filter, float* stereo, int frames) {
    for (int i = 0; i < frames; i++) {
        Stereo sound;
        sound.left = stereo[2 * i];
        sound.right = stereo[2 * i + 1];
        Stereo out = filter.next(sound);
        stereo[2 * i] = static_cast<float>(out.left);
        stereo[2 * i + 1] = static_cast<float>(out.right);
    }
}

}  // namespace

Amp::Amp(int sample_rate)
    : sample_rate_(sample_rate),
      first_stage_(sample_rate),
      second_stage_(sample_rate) {
    first_stage_.set_value(saturation_curve, static_cast<float>(SaturationCurve::tube));
    second_stage_.set_value(saturation_curve, static_cast<float>(SaturationCurve::tube));
    first_stage_.settle();
    second_stage_.settle();

    tighten_.set_high_pass(110.0, 0.7, sample_rate);
    between_stages_.set_high_pass(220.0, 0.7, sample_rate);
    speaker_low_cut_.set_high_pass(75.0, 0.8, sample_rate);
    speaker_thump_.set_bell(120.0, 3.0, 1.0, sample_rate);
    speaker_honk_.set_bell(2700.0, 5.0, 1.2, sample_rate);
    set(settings_);
}

void Amp::set(const AmpSettings& settings) {
    settings_ = settings;
    double gain = settings.gain;
    first_stage_.set_value(saturation_drive, static_cast<float>(gain * first_stage_most_db));
    second_stage_.set_value(saturation_drive, static_cast<float>(gain * second_stage_most_db));

    bass_.set_low_shelf(150.0, db_of_knob(settings.bass), 0.7, sample_rate_);
    middle_.set_bell(650.0, db_of_knob(settings.middle), 0.8, sample_rate_);
    treble_.set_high_shelf(2800.0, db_of_knob(settings.treble), 0.7, sample_rate_);
    presence_.set_high_shelf(4500.0, db_of_knob(settings.presence), 0.7, sample_rate_);

    double top = speaker_top_lowest_hz * std::pow(speaker_top_highest_hz / speaker_top_lowest_hz, settings.speaker);
    speaker_top_a_.set_low_pass(top, top_q_a, sample_rate_);
    speaker_top_b_.set_low_pass(top, top_q_b, sample_rate_);

    double volume = settings.volume;
    output_gain_ = volume * volume * most_volume * gain_of_db(-gain * gain_trim_most_db);
}

void Amp::process(float* stereo, int frames) {
    if (!settings_.on) {
        return;
    }
    EffectContext context;
    run(tighten_, stereo, frames);
    first_stage_.process(stereo, frames, context);
    run(between_stages_, stereo, frames);
    second_stage_.process(stereo, frames, context);

    run(bass_, stereo, frames);
    run(middle_, stereo, frames);
    run(treble_, stereo, frames);
    run(presence_, stereo, frames);

    run(speaker_low_cut_, stereo, frames);
    run(speaker_thump_, stereo, frames);
    run(speaker_honk_, stereo, frames);
    run(speaker_top_a_, stereo, frames);
    run(speaker_top_b_, stereo, frames);

    for (int i = 0; i < 2 * frames; i++) {
        stereo[i] = static_cast<float>(stereo[i] * output_gain_);
    }
}
