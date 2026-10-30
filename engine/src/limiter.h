#pragma once

#include <array>
#include <cstdint>
#include <vector>

#include "ease.h"

// The limits of the limiter's settings.
constexpr float limiter_most_boost_db = 24.0f;
constexpr float limiter_lowest_ceiling_db = -12.0f;
constexpr float limiter_highest_ceiling_db = 0.0f;
constexpr int limiter_shortest_release_ms = 1;
constexpr int limiter_longest_release_ms = 1000;

// The limiter's settings. A new engine starts here.
struct LimiterSettings {
    // How much louder the sound is made before the ceiling holds it, from 0
    // to limiter_most_boost_db.
    float boost_db = 0.0f;
    // The loudest the sound may get, from limiter_lowest_ceiling_db to
    // limiter_highest_ceiling_db. -1 dB is about 0.89.
    float ceiling_db = -1.0f;
    // How long it takes to come back up after turning the sound down: two
    // thirds of the way in this time, from limiter_shortest_release_ms to
    // limiter_longest_release_ms.
    int release_ms = 100;
};

// The master limiter (docs/design.md#mix-in-the-engine), the last step
// before the speakers or a WAV. It is a ceiling the sound never passes: it
// turns the sound down just enough, the moment it would go over, and lets
// it back up over the release. Both speakers are turned down together, so
// the sound never leans to one side.
//
// Live, it turns down on the very sample that would go over, so every sound
// stays on its exact sample. For WAV renders it looks 1 ms ahead instead: it
// holds the sound back that long and starts turning down before a peak
// arrives, which sounds cleaner, and it also catches the peaks between
// samples (true peak). Only the audio thread uses it, so nothing in here
// locks, allocates or waits.
class Limiter {
public:
    explicit Limiter(int sample_rate);

    // Changes the settings. The boost and the ceiling ease there over 10 ms.
    void set(const LimiterSettings& settings);

    // Looks 1 ms ahead from now on, for WAV renders. Call it before the
    // first sound goes through.
    void use_lookahead();

    // How many frames the sound is held back: 0 live, 1 ms with lookahead.
    int lookahead_frames() const;

    // Puts `frames` frames of stereo (left, right, left, ...) through the
    // limiter, in place.
    void process(float* stereo, int frames);

    // The most it turned the sound down in the last process, in dB, 0 for
    // not at all.
    float reduction_db() const;

private:
    // How many samples on each side of a point the true peak reads.
    static constexpr int reach = 8;

    // How many points between two samples the true peak checks.
    static constexpr int between = 3;

    // How many frames the rings hold: at least the lookahead and the true
    // peak's reach, at any sample rate up to 96 kHz.
    static constexpr int ring_frames = 128;

    // The gain sample `sound_at` needs, at the ceiling heard now: 1 when it
    // is under the ceiling, less when it would go over.
    float needed_gain(int sound_at) const;

    // The loudest the sound gets at sample `sound_at`, or between it and the
    // next, in either speaker.
    float true_peak(int sound_at) const;

    // The sound `back` frames before the newest, in one speaker.
    float sound(int back, int speaker) const;

    // Where sample `back` frames before the newest is in a ring.
    int ring_place(int back) const;

    int sample_rate_;
    bool lookahead_ = false;

    // With lookahead: how many frames the true peak waits for the samples
    // after a point, and how many frames the gain slides down over before a
    // peak. Together they make the lookahead. Both 0 live.
    int peak_wait_ = 0;
    int slide_frames_ = 0;

    Ease boost_db_;
    Ease ceiling_db_;
    float boost_ = 1.0f;
    float ceiling_ = 0.0f;
    double release_share_ = 0.0;

    // The weights the true peak reads the samples with, for each point
    // between two samples.
    std::array<std::array<float, 2 * reach>, between> weights_{};

    // The newest frames as they came in, after the boost; the gains each
    // sample needs; and those gains after the release.
    std::vector<float> sound_;
    std::vector<float> needed_;
    std::vector<double> released_;

    // How many frames have come in, and the release's gain now.
    std::uint64_t frames_in_ = 0;
    double gain_ = 1.0;

    float lowest_gain_ = 1.0f;
};
