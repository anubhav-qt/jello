#include "clock.h"

namespace {

// A new engine plays the beat at 120 beats per minute, the usual default in
// music software.
constexpr int start_tempo = 120;

}  // namespace

Clock::Clock(int sample_rate)
    : samples_per_minute_(static_cast<std::uint64_t>(sample_rate) * 60),
      tempo_(start_tempo),
      next_tempo_(start_tempo) {}

void Clock::start_beat() {
    running_ = true;
    tempo_ = next_tempo_;
    tempo_start_ = now_;
    beats_at_tempo_ = 0;
    next_beat_ = now_;
    beats_started_ = 0;
}

void Clock::stop_beat() {
    running_ = false;
}

bool Clock::is_beat_running() const {
    return running_;
}

void Clock::set_tempo(int tempo) {
    next_tempo_ = tempo;
}

bool Clock::beat_starts_now() const {
    if (!running_) {
        return false;
    }
    return now_ == next_beat_;
}

bool Clock::bar_starts_now() const {
    if (!beat_starts_now()) {
        return false;
    }
    return beats_started_ % beats_per_bar == 0;
}

std::uint64_t Clock::now() const {
    return now_;
}

std::uint64_t Clock::samples_per_bar() const {
    std::uint64_t tempo = static_cast<std::uint64_t>(tempo_);
    return beats_per_bar * samples_per_minute_ / tempo;
}

BeatLength Clock::beat_length() const {
    BeatLength length;
    length.samples_per_minute = samples_per_minute_;
    length.tempo = static_cast<std::uint64_t>(tempo_);
    return length;
}

BeatLength Clock::asked_beat_length() const {
    BeatLength length;
    length.samples_per_minute = samples_per_minute_;
    length.tempo = static_cast<std::uint64_t>(next_tempo_);
    return length;
}

std::uint64_t Clock::beat_number() const {
    return beats_started_;
}

void Clock::step() {
    if (beat_starts_now()) {
        beats_started_ += 1;
        plan_next_beat();
    }
    now_ += 1;
}

void Clock::plan_next_beat() {
    // A tempo asked for during the last beat starts with this one.
    if (next_tempo_ != tempo_) {
        tempo_ = next_tempo_;
        tempo_start_ = now_;
        beats_at_tempo_ = 0;
    }

    // A beat lasts a minute divided by the tempo, so beat n at this tempo
    // starts n times that after the tempo started, rounded down to a whole
    // sample. At 120 that is every 24,000 samples.
    beats_at_tempo_ += 1;
    std::uint64_t tempo = static_cast<std::uint64_t>(tempo_);
    next_beat_ = tempo_start_ + beats_at_tempo_ * samples_per_minute_ / tempo;
}
