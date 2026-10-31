#pragma once

#include "biquad.h"
#include "saturation.h"

// A guitar amp's knobs (docs/design.md#guitar), each from 0 to 1. The bass,
// middle, treble and presence leave the sound as it is at 0.5.
struct AmpSettings {
    bool on = false;
    // How hard the preamp is pushed: clean at 0, fully broken up at 1.
    float gain = 0.2f;
    float bass = 0.5f;
    float middle = 0.5f;
    float treble = 0.5f;
    float presence = 0.5f;
    // How much top the speaker lets through.
    float speaker = 0.5f;
    float volume = 0.5f;
};

// A guitar amp at the end of a track's notes, after they are added up, so the
// strings of a chord break up against each other as they do in a real amp.
// Two preamp stages bend the sound, then the tone knobs shape it and a speaker
// takes off the harsh top the bending made. Off, it leaves the sound untouched.
class Amp {
public:
    explicit Amp(int sample_rate);

    void set(const AmpSettings& settings);

    void process(float* stereo, int frames);

private:
    AmpSettings settings_;
    int sample_rate_;

    // Before each stage, taking off the lows that would blur the bending.
    Biquad tighten_;
    Biquad between_stages_;
    Saturation first_stage_;
    Saturation second_stage_;

    Biquad bass_;
    Biquad middle_;
    Biquad treble_;
    Biquad presence_;

    Biquad speaker_low_cut_;
    Biquad speaker_thump_;
    Biquad speaker_honk_;
    Biquad speaker_top_a_;
    Biquad speaker_top_b_;

    double output_gain_ = 1.0;
};
