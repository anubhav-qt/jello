#include "meter.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// The K-weighting filters, as BS.1770 gives them, worked out for any sample
// rate: the shelf's centre, gain and width, and the high-pass's corner and
// width (src-tauri/src/loudness.rs uses the same).
constexpr double shelf_hz = 1681.974450955533;
constexpr double shelf_gain_db = 3.999843853973347;
constexpr double shelf_q = 0.7071752369554196;
constexpr double shelf_band_power = 0.4996667741545416;
constexpr double high_pass_hz = 38.13547087602444;
constexpr double high_pass_q = 0.5003270373238773;

// BS.1770's offset, so a 1 kHz sine at full scale on one side reads
// -3.01 LUFS.
constexpr double loudness_offset = -0.691;

// How long a block is.
constexpr double block_seconds = 0.1;

double sinc(double x) {
    if (x == 0.0) {
        return 1.0;
    }
    return std::sin(pi * x) / (pi * x);
}

}  // namespace

double OutputMeter::Biquad::next(double sample) {
    double out = b0 * sample + b1 * in_1 + b2 * in_2;
    out -= a1 * out_1 + a2 * out_2;
    in_2 = in_1;
    in_1 = sample;
    out_2 = out_1;
    out_1 = out;
    return out;
}

OutputMeter::OutputMeter(int sample_rate) {
    double rate = static_cast<double>(sample_rate);

    // The shelf.
    double k = std::tan(pi * shelf_hz / rate);
    double high = std::pow(10.0, shelf_gain_db / 20.0);
    double band = std::pow(high, shelf_band_power);
    double shelf_a0 = 1.0 + k / shelf_q + k * k;
    Biquad shelf;
    shelf.b0 = (high + band * k / shelf_q + k * k) / shelf_a0;
    shelf.b1 = 2.0 * (k * k - high) / shelf_a0;
    shelf.b2 = (high - band * k / shelf_q + k * k) / shelf_a0;
    shelf.a1 = 2.0 * (k * k - 1.0) / shelf_a0;
    shelf.a2 = (1.0 - k / shelf_q + k * k) / shelf_a0;

    // The high-pass, which passes the mids at full level.
    k = std::tan(pi * high_pass_hz / rate);
    double high_pass_a0 = 1.0 + k / high_pass_q + k * k;
    Biquad high_pass;
    high_pass.b0 = 1.0;
    high_pass.b1 = -2.0;
    high_pass.b2 = 1.0;
    high_pass.a1 = 2.0 * (k * k - 1.0) / high_pass_a0;
    high_pass.a2 = (1.0 - k / high_pass_q + k * k) / high_pass_a0;

    for (Weighting& side : weighting_) {
        side.shelf = shelf;
        side.high_pass = high_pass;
    }

    block_frames_ = static_cast<int>(std::lround(block_seconds * rate));

    // For each point between two samples, the weights of the samples around
    // it: a sinc, faded out at the edges so it ends, each phase adding up to
    // 1 so a steady level stays put.
    for (int phase = 0; phase < oversampling; phase++) {
        double offset = static_cast<double>(phase) / oversampling;
        double sum = 0.0;
        std::array<double, taps> weights{};
        for (int place = 0; place < taps; place++) {
            double distance = offset - static_cast<double>(place - half_taps + 1);
            double fade = 0.5 * (1.0 + std::cos(pi * distance / half_taps));
            weights[place] = sinc(distance) * fade;
            sum += weights[place];
        }
        for (int place = 0; place < taps; place++) {
            weights_[phase][place] = static_cast<float>(weights[place] / sum);
        }
    }
}

void OutputMeter::process(const float* stereo, int frames) {
    for (int i = 0; i < frames; i++) {
        measure(stereo[2 * i], stereo[2 * i + 1]);
    }
}

void OutputMeter::measure(float left, float right) {
    float sides[2] = {left, right};
    for (int side = 0; side < 2; side++) {
        Weighting& weighting = weighting_[side];
        double weighted = weighting.high_pass.next(weighting.shelf.next(sides[side]));
        block_sum_ += weighted * weighted;

        // The newest sample goes on the end, the oldest drops off.
        std::array<float, taps>& recent = recent_[side];
        for (int place = 0; place + 1 < taps; place++) {
            recent[place] = recent[place + 1];
        }
        recent[taps - 1] = sides[side];
        float peak = true_peak_of(recent);
        if (peak > true_peak_) {
            true_peak_ = peak;
        }
    }

    block_filled_ += 1;
    if (block_filled_ < block_frames_) {
        return;
    }
    blocks_[next_block_] = block_sum_ / block_frames_;
    next_block_ = (next_block_ + 1) % block_count;
    block_sum_ = 0.0;
    block_filled_ = 0;

    double sum = 0.0;
    for (double power : blocks_) {
        sum += power;
    }
    double power = sum / block_count;
    short_term_lufs_ = lowest_lufs;
    if (power > 0.0) {
        double lufs = loudness_offset + 10.0 * std::log10(power);
        if (lufs > lowest_lufs) {
            short_term_lufs_ = static_cast<float>(lufs);
        }
    }
}

float OutputMeter::true_peak_of(const std::array<float, taps>& recent) const {
    // The points are read half_taps samples back, so the samples after them
    // have come in.
    float loudest = 0.0f;
    for (const std::array<float, taps>& weights : weights_) {
        float point = 0.0f;
        for (int place = 0; place < taps; place++) {
            point += recent[place] * weights[place];
        }
        float level = std::fabs(point);
        if (level > loudest) {
            loudest = level;
        }
    }
    return loudest;
}

float OutputMeter::take_true_peak() {
    float peak = true_peak_;
    true_peak_ = 0.0f;
    return peak;
}

float OutputMeter::short_term_lufs() const {
    return short_term_lufs_;
}
