#pragma once

#include <array>
#include <vector>

// The output meter (docs/design.md#mix): what the Mix panel reads about the
// sound leaving the master, after the limiter. It measures, as
// src-tauri/src/loudness.rs does for a whole render:
//
// - Short-term loudness, in LUFS (ITU-R BS.1770-4, EBU R 128): the sound
//   K-weighted (a shelf that lifts the highs by 4 dB, then a high-pass that
//   lets go of the deepest bass), its power added up over both sides in
//   blocks of 100 ms, and the last 30 blocks, 3 s, averaged.
// - The true peak: the loudest point on either side, between the samples as
//   well as on them, read at 4 times the sample rate.
//
// Only the audio thread uses it, so nothing in here locks, allocates or
// waits.
class OutputMeter {
public:
    explicit OutputMeter(int sample_rate);

    // Measures `frames` frames of stereo (left, right, left, ...).
    void process(const float* stereo, int frames);

    // The loudest true peak since the last take, as a level from 0, then
    // starts again from silence.
    float take_true_peak();

    // The loudness of the last 3 s, in LUFS, or lowest_lufs for silence. It
    // moves on every 100 ms.
    float short_term_lufs() const;

    // What silence reads.
    static constexpr float lowest_lufs = -70.0f;

private:
    // How many 100 ms blocks short-term loudness averages.
    static constexpr int block_count = 30;

    // How many times the sample rate the true peak is read at, and how many
    // samples on each side of a point the reading uses.
    static constexpr int oversampling = 4;
    static constexpr int half_taps = 6;
    static constexpr int taps = 2 * half_taps;

    // A filter of two poles and two zeros, one side at a time.
    struct Biquad {
        double b0 = 1.0;
        double b1 = 0.0;
        double b2 = 0.0;
        double a1 = 0.0;
        double a2 = 0.0;
        double in_1 = 0.0;
        double in_2 = 0.0;
        double out_1 = 0.0;
        double out_2 = 0.0;

        double next(double sample);
    };

    // The K-weighting filters for one side: the shelf, then the high-pass.
    struct Weighting {
        Biquad shelf;
        Biquad high_pass;
    };

    // Reads one frame: its weighted power and its true peak.
    void measure(float left, float right);

    // The loudest point at or just after the newest sample on one side,
    // read from that side's last `taps` samples.
    float true_peak_of(const std::array<float, taps>& recent) const;

    std::array<Weighting, 2> weighting_{};

    // The power added up in the block being measured, and how many frames
    // it holds so far, out of block_frames_.
    double block_sum_ = 0.0;
    int block_filled_ = 0;
    int block_frames_ = 0;

    // The last block_count blocks' mean powers, and where the next goes.
    std::array<double, block_count> blocks_{};
    int next_block_ = 0;

    float short_term_lufs_ = lowest_lufs;

    // Each side's last `taps` samples, oldest first, and the weights each
    // point between two samples reads them with.
    std::array<std::array<float, taps>, 2> recent_{};
    std::array<std::array<float, taps>, oversampling> weights_{};

    float true_peak_ = 0.0f;
};
