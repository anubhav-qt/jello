#include "oscillator.h"

namespace {

// How many cents make an octave and a half step.
constexpr double cents_per_octave = 1200.0;
constexpr double cents_per_half_step = 100.0;

}  // namespace

std::array<OscillatorSettings, oscillator_count> start_oscillators() {
    std::array<OscillatorSettings, oscillator_count> oscillators{};
    oscillators[0].level = 1.0f;
    return oscillators;
}

double offset_cents(const OscillatorSettings& oscillator) {
    double cents = oscillator.octaves * cents_per_octave;
    cents += oscillator.half_steps * cents_per_half_step;
    cents += oscillator.cents;
    return cents;
}

Wave wave_of(Kind kind) {
    if (kind == Kind::sine) {
        return Wave::sine;
    } else if (kind == Kind::triangle) {
        return Wave::triangle;
    } else if (kind == Kind::square) {
        return Wave::square;
    } else {
        return Wave::saw;
    }
}
