#include "filter.h"

#include <algorithm>
#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// How long a moved control takes to glide to where it was moved, short
// enough to feel instant and long enough not to click.
constexpr int glide_ms = 20;

// Middle C, the note whose cutoff the cutoff control sets.
constexpr int middle_c = 60;

// The cutoff never goes past this share of the sample rate, 21,600 Hz at
// 48,000 samples a second. Closer to half the rate, the highest pitch the
// samples can hold, the filter's math stops behaving.
constexpr double highest_share_of_rate = 0.45;

// How sharp the bump at the cutoff is (its Q), at no resonance and at full.
// 0.7071 is no bump at all; 10 is a strong, singing peak that still stops
// short of whistling on its own.
constexpr double lowest_q = 0.7071;
constexpr double highest_q = 10.0;

int glide_samples(int sample_rate) {
    return sample_rate * glide_ms / 1000;
}

}  // namespace

FilterTargets filter_targets_of(const FilterSettings& settings) {
    FilterTargets targets;
    targets.cutoff = std::log2(static_cast<double>(settings.cutoff_hz));
    targets.resonance = settings.resonance;
    if (settings.cutoff_hz >= filter_open_hz) {
        targets.share = 0.0;
    } else {
        targets.share = 1.0;
    }
    return targets;
}

Glide::Glide(int samples) : samples_(samples) {}

void Glide::jump_to(double value) {
    value_ = value;
    target_ = value;
    samples_left_ = 0;
}

double Glide::next(double target) {
    if (target != target_) {
        target_ = target;
        samples_left_ = samples_;
        step_ = (target - value_) / samples_;
    }

    if (samples_left_ > 0) {
        value_ += step_;
        samples_left_ -= 1;
        // Land exactly on the target, as the envelope does.
        if (samples_left_ == 0) {
            value_ = target_;
        }
    }
    return value_;
}

void LowPass::set(double cutoff_hz, double resonance, int sample_rate) {
    // The resonance is squared, so the first half of the control stays
    // gentle and the peak grows quickly near the top.
    double q = lowest_q + resonance * resonance * (highest_q - lowest_q);
    double k = 1.0 / q;

    // g is the cutoff, bent the way the analog filter's math needs before
    // it can run on samples, so the cutoff lands on the right pitch.
    double g = std::tan(pi * cutoff_hz / sample_rate);
    a1_ = 1.0 / (1.0 + g * (g + k));
    a2_ = g * a1_;
    a3_ = g * a2_;
}

Stereo LowPass::next(Stereo input) {
    Stereo output;
    output.left = next_side(input.left, left_);
    output.right = next_side(input.right, right_);
    return output;
}

double LowPass::next_side(double input, Memory& memory) const {
    // How far the input is from where the second stage was.
    double difference = input - memory.second;

    // The first stage passes a band around the cutoff, and the second turns
    // that into the low-pass: what comes out.
    double band = a1_ * memory.first + a2_ * difference;
    double low = memory.second + a2_ * memory.first + a3_ * difference;

    memory.first = 2.0 * band - memory.first;
    memory.second = 2.0 * low - memory.second;
    return low;
}

void LowPass::clear() {
    left_ = Memory{};
    right_ = Memory{};
}

VoiceFilter::VoiceFilter(int sample_rate)
    : sample_rate_(sample_rate),
      highest_hz_(sample_rate * highest_share_of_rate),
      envelope_(sample_rate),
      cutoff_(glide_samples(sample_rate)),
      resonance_(glide_samples(sample_rate)),
      share_(glide_samples(sample_rate)) {}

void VoiceFilter::start(int note, bool fresh, const FilterTargets& targets, const FilterEnvelopeSettings& envelope,
                        float key_tracking) {
    double octaves_above_middle_c = (note - middle_c) / 12.0;
    tracking_ = key_tracking * octaves_above_middle_c;
    amount_ = envelope.amount;

    // A silent voice's filter envelope starts from nothing, even if its last
    // note's filter envelope was still letting go when the sound ended.
    if (fresh) {
        envelope_ = Envelope(sample_rate_);
    }
    envelope_.set_stages(envelope.attack_ms, envelope.decay_ms, envelope.sustain, envelope.release_ms);
    envelope_.start(1.0f);

    // A voice still sounding glides on from where it was. A silent one has
    // nothing to glide from, and nothing of its last note left to hear.
    if (fresh) {
        cutoff_.jump_to(targets.cutoff + tracking_);
        resonance_.jump_to(targets.resonance);
        share_.jump_to(targets.share);
        low_pass_.clear();
    }
}

void VoiceFilter::release() {
    envelope_.release();
}

void VoiceFilter::set_sustain(double sustain, int samples) {
    envelope_.set_sustain(sustain, samples);
}

double VoiceFilter::envelope_level() const {
    return envelope_.level();
}

Stereo VoiceFilter::next(Stereo wave, const FilterTargets& targets, const FilterMoves& moves) {
    double amount = std::clamp(amount_ + moves.amount, 0.0, static_cast<double>(filter_most_octaves));

    // The note's cutoff follows middle C's, plus however far its filter
    // envelope has opened it right now, plus the routing list's move.
    double opening = envelope_.next() * amount;
    double cutoff = cutoff_.next(targets.cutoff + tracking_) + opening + moves.cutoff;
    double resonance = std::clamp(resonance_.next(targets.resonance) + moves.resonance, 0.0, 1.0);
    double share = share_.next(targets.share);

    // tan and exp2 are slow next to the rest, so the low-pass's math is only
    // worked out again when the cutoff or the resonance moves.
    if (!low_pass_ready_ || cutoff != set_cutoff_ || resonance != set_resonance_) {
        double hz = std::exp2(cutoff);
        if (hz > highest_hz_) {
            hz = highest_hz_;
        }
        if (hz < filter_lowest_hz) {
            hz = filter_lowest_hz;
        }
        low_pass_.set(hz, resonance, sample_rate_);
        set_cutoff_ = cutoff;
        set_resonance_ = resonance;
        low_pass_ready_ = true;
    }

    // The filter always runs, even when open, so turning it on mid-note
    // never starts it from nothing. Open, the share is 0 and the wave comes
    // out exactly as it went in.
    Stereo filtered = low_pass_.next(wave);
    Stereo out;
    out.left = wave.left * (1.0 - share) + filtered.left * share;
    out.right = wave.right * (1.0 - share) + filtered.right * share;
    return out;
}
