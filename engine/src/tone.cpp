#include "tone.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double frequency = 440.0;     // A4
constexpr float level = 0.25f;          // well below full scale (1.0)
constexpr double fade_seconds = 0.010;  // 10 ms
constexpr double two_pi = 6.283185307179586;

}  // namespace

Tone::Tone(int sample_rate)
    : on_(false),
      phase_(0.0),
      phase_step_(frequency / sample_rate),
      volume_(0.0f),
      volume_step_(static_cast<float>(level / (fade_seconds * sample_rate))) {}

void Tone::set_on(bool on) {
    on_.store(on);
}

void Tone::render(float* out, int frames) {
    // Read the switch once per block of audio.
    float target = 0.0f;
    if (on_.load()) {
        target = level;
    }

    for (int i = 0; i < frames; i++) {
        // The fade: move the volume one small step toward the target.
        if (volume_ < target) {
            volume_ = std::min(volume_ + volume_step_, target);
        } else if (volume_ > target) {
            volume_ = std::max(volume_ - volume_step_, target);
        }

        float wave = static_cast<float>(std::sin(two_pi * phase_));
        float sample = volume_ * wave;

        // Move along the wave. Wrapping back to 0 keeps the number small and
        // precise.
        phase_ += phase_step_;
        if (phase_ >= 1.0) {
            phase_ -= 1.0;
        }

        out[2 * i] = sample;      // left
        out[2 * i + 1] = sample;  // right
    }
}
