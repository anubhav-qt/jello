#include "click.h"

#include <cmath>

namespace {

// A tick is a sine wave that starts loud and fades fast. Kept by ear as
// they first were, in looper slice 1.

// The pitch of the high tick, on the first beat of a bar, and of the other
// ticks: A6 and A5, an octave apart.
constexpr double high_hz = 1760.0;
constexpr double low_hz = 880.0;

// How loud a tick starts.
constexpr double tick_level = 0.25;

// A tick gets half as loud every 4 ms. After 40 ms it is 1,024 times quieter
// than it started, too quiet to hear, so it stops there.
constexpr double half_life_ms = 4.0;
constexpr int tick_ms = 40;

constexpr double pi = 3.14159265358979323846;

}  // namespace

Click::Click(int sample_rate)
    : sample_rate_(sample_rate),
      tick_samples_(sample_rate * tick_ms / 1000) {
    double half_life_samples = sample_rate * half_life_ms / 1000.0;
    fade_ = std::pow(0.5, 1.0 / half_life_samples);
}

void Click::start(bool high) {
    double hz = low_hz;
    if (high) {
        hz = high_hz;
    }

    // Every tick starts its wave from zero, so they all sound the same.
    phase_ = 0.0;
    phase_step_ = hz / sample_rate_;
    level_ = tick_level;
    samples_left_ = tick_samples_;
}

float Click::next_sample() {
    if (samples_left_ == 0) {
        return 0.0f;
    }

    double sample = level_ * std::sin(2.0 * pi * phase_);

    phase_ += phase_step_;
    if (phase_ >= 1.0) {
        phase_ -= 1.0;
    }
    level_ *= fade_;
    samples_left_ -= 1;

    return static_cast<float>(sample);
}
