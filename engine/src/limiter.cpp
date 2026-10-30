#include "limiter.h"

#include <algorithm>
#include <cmath>

namespace {

// How long the boost and the ceiling take to ease all the way across their
// ranges.
constexpr float ease_seconds = 0.01f;

// How far ahead the limiter looks in WAV renders.
constexpr int lookahead_ms = 1;

constexpr double pi = 3.14159265358979323846;

// A level in dB as a gain: 0 dB is 1, -6 dB about a half.
float gain_of(float db) {
    return std::pow(10.0f, db / 20.0f);
}

}  // namespace

Limiter::Limiter(int sample_rate)
    : sample_rate_(sample_rate),
      boost_db_(0.0f, limiter_most_boost_db / (ease_seconds * static_cast<float>(sample_rate))),
      ceiling_db_(-1.0f,
                  (limiter_highest_ceiling_db - limiter_lowest_ceiling_db) /
                      (ease_seconds * static_cast<float>(sample_rate))),
      sound_(2 * ring_frames, 0.0f),
      needed_(ring_frames, 1.0f),
      released_(ring_frames, 1.0) {
    set(LimiterSettings{});
    boost_ = gain_of(boost_db_.heard());
    ceiling_ = gain_of(ceiling_db_.heard());

    // The weights that read the wave between two samples from the samples
    // around them: a sinc, the shape that rebuilds a wave exactly from its
    // samples, faded out toward its ends by a Hann window so a few samples
    // are enough. Each point's weights add up to 1, so a steady sound stays
    // the same.
    for (int point = 0; point < between; point++) {
        double place = static_cast<double>(point + 1) / static_cast<double>(between + 1);
        double sum = 0.0;
        for (int tap = 0; tap < 2 * reach; tap++) {
            double distance = place - static_cast<double>(tap - reach + 1);
            double sinc = std::sin(pi * distance) / (pi * distance);
            double window = 0.5 * (1.0 + std::cos(pi * distance / (reach + 0.5)));
            weights_[point][tap] = static_cast<float>(sinc * window);
            sum += sinc * window;
        }
        for (int tap = 0; tap < 2 * reach; tap++) {
            weights_[point][tap] = static_cast<float>(weights_[point][tap] / sum);
        }
    }
}

void Limiter::set(const LimiterSettings& settings) {
    boost_db_.set(settings.boost_db);
    ceiling_db_.set(settings.ceiling_db);

    double release_samples = settings.release_ms * sample_rate_ / 1000.0;
    release_share_ = 1.0 - std::exp(-1.0 / release_samples);
}

void Limiter::use_lookahead() {
    int lookahead = sample_rate_ * lookahead_ms / 1000;
    lookahead_ = true;
    peak_wait_ = reach;
    slide_frames_ = lookahead - reach;
}

int Limiter::lookahead_frames() const {
    return peak_wait_ + slide_frames_;
}

float Limiter::reduction_db() const {
    return -20.0f * std::log10(lowest_gain_);
}

int Limiter::ring_place(int back) const {
    std::uint64_t place = frames_in_ - 1 - static_cast<std::uint64_t>(back);
    return static_cast<int>(place % ring_frames);
}

float Limiter::sound(int back, int speaker) const {
    return sound_[2 * ring_place(back) + speaker];
}

float Limiter::true_peak(int sound_at) const {
    float loudest = std::max(std::fabs(sound(sound_at, 0)), std::fabs(sound(sound_at, 1)));

    // The points between this sample and the next, each read from the
    // samples around it: from reach - 1 before to reach after.
    for (int point = 0; point < between; point++) {
        for (int speaker = 0; speaker < 2; speaker++) {
            float wave = 0.0f;
            for (int tap = 0; tap < 2 * reach; tap++) {
                int back = sound_at - (tap - reach + 1);
                wave += weights_[point][tap] * sound(back, speaker);
            }
            loudest = std::max(loudest, std::fabs(wave));
        }
    }
    return loudest;
}

float Limiter::needed_gain(int sound_at) const {
    float peak;
    if (lookahead_) {
        peak = true_peak(sound_at);
    } else {
        peak = std::max(std::fabs(sound(sound_at, 0)), std::fabs(sound(sound_at, 1)));
    }

    if (peak <= ceiling_) {
        return 1.0f;
    }
    return ceiling_ / peak;
}

void Limiter::process(float* stereo, int frames) {
    lowest_gain_ = 1.0f;
    int lookahead = lookahead_frames();

    for (int i = 0; i < frames; i++) {
        // The boost and the ceiling ease, and their gains follow.
        bool boost_was_moving = boost_db_.is_moving();
        float boost_db = boost_db_.next();
        if (boost_was_moving) {
            boost_ = gain_of(boost_db);
        }
        bool ceiling_was_moving = ceiling_db_.is_moving();
        float ceiling_db = ceiling_db_.next();
        if (ceiling_was_moving) {
            ceiling_ = gain_of(ceiling_db);
        }

        frames_in_ += 1;
        int newest = ring_place(0);
        sound_[2 * newest] = stereo[2 * i] * boost_;
        sound_[2 * newest + 1] = stereo[2 * i + 1] * boost_;

        // The gain needed by the newest sample the true peak can read. Live,
        // that is the newest sample.
        needed_[newest] = needed_gain(peak_wait_);

        // The lowest gain needed over the slide, so the gain is already down
        // when a peak comes out.
        float held = 1.0f;
        for (int back = 0; back <= slide_frames_; back++) {
            held = std::min(held, needed_[ring_place(back)]);
        }

        // Down at once, back up over the release.
        if (held < gain_) {
            gain_ = held;
        } else {
            gain_ += (held - gain_) * release_share_;
        }
        released_[newest] = gain_;

        // Averaged over the slide, so the gain goes down smoothly before a
        // peak instead of jumping. Every gain in the average is at most what
        // the sample coming out needs, so it never goes over.
        double sum = 0.0;
        for (int back = 0; back <= slide_frames_; back++) {
            sum += released_[ring_place(back)];
        }
        float gain = static_cast<float>(sum / (slide_frames_ + 1));
        lowest_gain_ = std::min(lowest_gain_, gain);

        // The sample coming out, held back by the lookahead. Rounding can
        // never take it past the ceiling.
        for (int speaker = 0; speaker < 2; speaker++) {
            float out = sound(lookahead, speaker) * gain;
            stereo[2 * i + speaker] = std::clamp(out, -ceiling_, ceiling_);
        }
    }
}
