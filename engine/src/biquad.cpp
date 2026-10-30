#include "biquad.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// The frequency as an angle each sample: a full turn is the sample rate.
double angle_of(double hz, int sample_rate) {
    return 2.0 * pi * hz / sample_rate;
}

}  // namespace

void Biquad::use(double b0, double b1, double b2, double a0, double a1, double a2) {
    b0_ = b0 / a0;
    b1_ = b1 / a0;
    b2_ = b2 / a0;
    a1_ = a1 / a0;
    a2_ = a2 / a0;
}

void Biquad::set_low_pass(double hz, double q, int sample_rate) {
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    use((1.0 - cos_w) / 2.0, 1.0 - cos_w, (1.0 - cos_w) / 2.0, 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha);
}

void Biquad::set_high_pass(double hz, double q, int sample_rate) {
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    use((1.0 + cos_w) / 2.0, -(1.0 + cos_w), (1.0 + cos_w) / 2.0, 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha);
}

void Biquad::set_low_shelf(double hz, double gain_db, double q, int sample_rate) {
    double a = std::pow(10.0, gain_db / 40.0);
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    double root = 2.0 * std::sqrt(a) * alpha;
    use(a * ((a + 1.0) - (a - 1.0) * cos_w + root),
        2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w),
        a * ((a + 1.0) - (a - 1.0) * cos_w - root),
        (a + 1.0) + (a - 1.0) * cos_w + root,
        -2.0 * ((a - 1.0) + (a + 1.0) * cos_w),
        (a + 1.0) + (a - 1.0) * cos_w - root);
}

void Biquad::set_high_shelf(double hz, double gain_db, double q, int sample_rate) {
    double a = std::pow(10.0, gain_db / 40.0);
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    double root = 2.0 * std::sqrt(a) * alpha;
    use(a * ((a + 1.0) + (a - 1.0) * cos_w + root),
        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w),
        a * ((a + 1.0) + (a - 1.0) * cos_w - root),
        (a + 1.0) - (a - 1.0) * cos_w + root,
        2.0 * ((a - 1.0) - (a + 1.0) * cos_w),
        (a + 1.0) - (a - 1.0) * cos_w - root);
}

void Biquad::set_bell(double hz, double gain_db, double q, int sample_rate) {
    double a = std::pow(10.0, gain_db / 40.0);
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    use(1.0 + alpha * a, -2.0 * cos_w, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos_w, 1.0 - alpha / a);
}

void Biquad::set_notch(double hz, double q, int sample_rate) {
    double w = angle_of(hz, sample_rate);
    double cos_w = std::cos(w);
    double alpha = std::sin(w) / (2.0 * q);
    use(1.0, -2.0 * cos_w, 1.0, 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha);
}

void Biquad::clear() {
    left_ = Memory{};
    right_ = Memory{};
}

Stereo Biquad::next(Stereo input) {
    Stereo output;
    output.left = next_side(input.left, left_);
    output.right = next_side(input.right, right_);
    return output;
}

double Biquad::next_side(double input, Memory& memory) const {
    // Transposed direct form II: the memory holds what the last two samples
    // still add to this one, which stays steady as the numbers move.
    double output = b0_ * input + memory.first;
    memory.first = b1_ * input - a1_ * output + memory.second;
    memory.second = b2_ * input - a2_ * output;
    return output;
}
