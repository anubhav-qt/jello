#include "envelope.h"

#include <algorithm>

namespace {

// At least one sample, so even a 0 ms stage still ends.
int ms_to_samples(int ms, int sample_rate) {
    int samples = sample_rate * ms / 1000;
    if (samples < 1) {
        return 1;
    }
    return samples;
}

}  // namespace

Envelope::Envelope(int sample_rate) : sample_rate_(sample_rate) {
    // A new song's loudness envelope, until a note sets its own.
    LoudnessEnvelopeSettings start;
    set_stages(start.attack_ms, start.decay_ms, start.sustain, start.release_ms);
}

void Envelope::set_stages(int attack_ms, int decay_ms, double sustain, int release_ms) {
    attack_samples_ = ms_to_samples(attack_ms, sample_rate_);
    decay_samples_ = ms_to_samples(decay_ms, sample_rate_);
    sustain_ = sustain;
    release_samples_ = ms_to_samples(release_ms, sample_rate_);
}

void Envelope::set_sustain(double sustain, int samples) {
    if (sustain == sustain_) {
        return;
    }
    sustain_ = sustain;

    if (stage_ == Stage::decay) {
        // Head for the new level in the time the decay has left.
        target_ = peak_ * sustain_;
        step_ = (target_ - level_) / samples_left_;
    } else if (stage_ == Stage::sustain) {
        // At least one sample, so the glide always ends.
        int glide_samples = std::max(samples, 1);
        glide(Stage::sustain, peak_ * sustain_, glide_samples);
    }
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
    // Silence holds still, and so does the sustain once it has glided to a
    // moved level.
    if (stage_ == Stage::silent) {
        return static_cast<float>(level_);
    }
    if (stage_ == Stage::sustain) {
        if (samples_left_ == 0) {
            return static_cast<float>(level_);
        }
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
        } else if (stage_ == Stage::release) {
            stage_ = Stage::silent;  // the release has ended
        }
        // A sustain that glided to a moved level holds there.
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

double Envelope::share() const {
    if (peak_ <= 0.0) {
        return 0.0;
    }
    return level_ / peak_;
}

void Envelope::glide(Stage stage, double target, int samples) {
    stage_ = stage;
    target_ = target;
    samples_left_ = samples;
    step_ = (target - level_) / samples;
}
