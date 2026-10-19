#include "voice.h"

#include <cmath>

namespace {

// How loud one voice is. With it, a 4-note chord is about as loud as the
// step 1 test tone. A starting value, to be tuned by ear in slice 5.
constexpr float voice_level = 0.35f;

// A note's pitch in Hz. Note 69 is A4 at 440 Hz. Every 12 notes is an octave,
// and each octave up doubles the pitch.
double note_to_frequency(int note) {
    double octaves_from_a4 = (note - 69) / 12.0;
    return 440.0 * std::pow(2.0, octaves_from_a4);
}

// How hard the key was hit, turned into the envelope's peak, from 0 to 1.
// Squared, so half the velocity gives a quarter of the loudness.
float velocity_to_peak(int velocity) {
    float amount = static_cast<float>(velocity) / 127.0f;
    return amount * amount;
}

// The triangle wave at `phase` (0 to 1): rises from 0 to 1, falls to -1, then
// climbs back to 0.
double triangle(double phase) {
    if (phase < 0.25) {
        return 4.0 * phase;
    } else if (phase < 0.75) {
        return 2.0 - 4.0 * phase;
    } else {
        return 4.0 * phase - 4.0;
    }
}

}  // namespace

Voice::Voice(int sample_rate)
    : sample_rate_(sample_rate),
      envelope_(sample_rate) {}

void Voice::start(int note, int velocity, std::uint64_t press_number) {
    // The phase is left where it is, so a voice that is still sounding carries
    // on from the same point in its wave instead of jumping.
    note_ = note;
    press_number_ = press_number;
    phase_step_ = note_to_frequency(note) / sample_rate_;
    envelope_.start(velocity_to_peak(velocity));
}

void Voice::release() {
    envelope_.release();
}

bool Voice::is_free() const {
    return envelope_.is_silent();
}

bool Voice::is_releasing() const {
    return envelope_.is_releasing();
}

double Voice::level() const {
    return envelope_.level();
}

int Voice::note() const {
    return note_;
}

std::uint64_t Voice::press_number() const {
    return press_number_;
}

void Voice::render(float* out, int frames) {
    // A free voice is silent, so there is nothing to add.
    if (is_free()) {
        return;
    }

    for (int i = 0; i < frames; i++) {
        float wave = static_cast<float>(triangle(phase_));
        float loudness = envelope_.next();
        float sample = wave * loudness * voice_level;

        // Move along the wave. Wrapping back to 0 keeps the number small and
        // precise.
        phase_ += phase_step_;
        if (phase_ >= 1.0) {
            phase_ -= 1.0;
        }

        out[2 * i] += sample;      // left
        out[2 * i + 1] += sample;  // right
    }
}
