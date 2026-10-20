#include "voice.h"

#include <cmath>

namespace {

// How loud one voice is. With it, a 4-note chord is about as loud as the
// step 1 test tone. Chosen by ear in slice 5.
constexpr float voice_level = 0.35f;

// How long a gap takes to shrink away, the same as the envelope's attack.
// Short enough that the new wave takes over at once, long enough not to
// click. A starting value, to be tuned by ear in slice 5.
constexpr int gap_ms = 5;

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

}  // namespace

Voice::Voice(int sample_rate)
    : sample_rate_(sample_rate),
      gap_samples_(sample_rate * gap_ms / 1000),
      envelope_(sample_rate) {}

void Voice::start(int note, int velocity, std::uint64_t press_number, Wave wave) {
    // Where the sound was heading next: the old wave, plus what is left of
    // any earlier gap.
    bool was_sounding = !is_free();
    double old_height = height_now() + gap_now();

    // The phase is left where it is, so a voice that is still sounding carries
    // on from the same point in its wave instead of jumping.
    note_ = note;
    press_number_ = press_number;
    wave_ = wave;
    phase_step_ = note_to_frequency(note) / sample_rate_;
    envelope_.start(velocity_to_peak(velocity));

    // A new wave can be at a different height here. The gap makes up the
    // difference, so the sound carries on from where it was, then shrinks
    // away. A silent voice was heading nowhere, so it has no gap.
    if (was_sounding) {
        gap_ = old_height - height_now();
        gap_samples_left_ = gap_samples_;
    } else {
        gap_ = 0.0;
        gap_samples_left_ = 0;
    }
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
        double exact_height = height_now() + gap_now();
        float height = static_cast<float>(exact_height);
        float loudness = envelope_.next();
        float sample = height * loudness * voice_level;

        // The gap shrinks a step each sample, until it is gone.
        if (gap_samples_left_ > 0) {
            gap_samples_left_ -= 1;
        }

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

double Voice::height_now() const {
    double height = wave_height(wave_, phase_, phase_step_);
    return height * wave_trim(wave_);
}

double Voice::gap_now() const {
    if (gap_samples_left_ == 0) {
        return 0.0;
    }
    double share_left = static_cast<double>(gap_samples_left_) / gap_samples_;
    return gap_ * share_left;
}
