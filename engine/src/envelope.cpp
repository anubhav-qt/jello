#include "envelope.h"

namespace {

// The loudness envelope's stages: every voice shares these, all checked by
// ear in slice 5. The attack made no audible difference between 2 and 20 ms,
// so it stays at 5. With the sustain at the full peak, a held note never
// dips, so the decay has nothing to do for now. It stays for sounds that
// will want a dip later.
constexpr int attack_ms = 5;
constexpr int decay_ms = 300;
constexpr double sustain_level = 1.0;  // the full peak
constexpr int release_ms = 300;

// At least one sample, so even a 0 ms stage still ends.
int ms_to_samples(int ms, int sample_rate) {
    int samples = sample_rate * ms / 1000;
    if (samples < 1) {
        return 1;
    }
    return samples;
}

}  // namespace

Envelope::Envelope(int sample_rate)
    : sample_rate_(sample_rate),
      attack_samples_(ms_to_samples(attack_ms, sample_rate)),
      decay_samples_(ms_to_samples(decay_ms, sample_rate)),
      sustain_(sustain_level),
      release_samples_(ms_to_samples(release_ms, sample_rate)) {}

void Envelope::set_stages(int attack_ms, int decay_ms, double sustain, int release_ms) {
    attack_samples_ = ms_to_samples(attack_ms, sample_rate_);
    decay_samples_ = ms_to_samples(decay_ms, sample_rate_);
    sustain_ = sustain;
    release_samples_ = ms_to_samples(release_ms, sample_rate_);
}

void Envelope::start(float peak) {
    peak_ = peak;
    glide(Stage::attack, peak, attack_samples_);
}

void Envelope::release() {
    if (stage_ == Stage::silent || stage_ == Stage::release) {
        return;
    }
    glide(Stage::release, 0.0, release_samples_);
}

float Envelope::next() {
    // Silence and sustain hold still.
    if (stage_ == Stage::silent || stage_ == Stage::sustain) {
        return static_cast<float>(level_);
    }

    level_ += step_;
    samples_left_ -= 1;

    if (samples_left_ == 0) {
        // Land exactly on the target. Each addition above rounds a little,
        // and the release must end at exactly 0.
        level_ = target_;

        if (stage_ == Stage::attack) {
            glide(Stage::decay, peak_ * sustain_, decay_samples_);
        } else if (stage_ == Stage::decay) {
            stage_ = Stage::sustain;
        } else {
            stage_ = Stage::silent;  // the release has ended
        }
    }

    return static_cast<float>(level_);
}

bool Envelope::is_silent() const {
    return stage_ == Stage::silent;
}

bool Envelope::is_releasing() const {
    return stage_ == Stage::release;
}

double Envelope::level() const {
    return level_;
}

void Envelope::glide(Stage stage, double target, int samples) {
    stage_ = stage;
    target_ = target;
    samples_left_ = samples;
    step_ = (target - level_) / samples;
}
