#pragma once

#include <cstdint>

// How many beats make a bar. Every bar has 4 beats for now; other sizes,
// like a waltz's 3, can come later.
constexpr int beats_per_bar = 4;

// How long a beat lasts: a minute's samples divided by the tempo. It is kept
// as those two numbers, not divided out, so every part of the engine works
// out the same whole samples from it.
struct BeatLength {
    std::uint64_t samples_per_minute;  // 2,880,000 at 48,000 a second
    std::uint64_t tempo;               // beats per minute
};

// Keeps the beat (docs/adr/0021-a-looper-on-a-beat.md). It counts every
// sample the engine makes, and while the beat is running it knows which
// sample each beat starts on. Only the audio thread uses it, so nothing in
// here locks, allocates or waits.
class Clock {
public:
    explicit Clock(int sample_rate);

    // Starts the beat on the current sample, as the first beat of a bar.
    void start_beat();

    // Stops the beat. The samples are still counted.
    void stop_beat();

    // True while the beat is running.
    bool is_beat_running() const;

    // Plays the beat at `tempo` beats per minute from the next beat on, so
    // the beat already playing keeps its length.
    void set_tempo(int tempo);

    // True when a beat starts on the current sample.
    bool beat_starts_now() const;

    // True when a bar starts on the current sample: its first beat.
    bool bar_starts_now() const;

    // The current sample: how many samples were made before it.
    std::uint64_t now() const;

    // About how many samples a bar lasts at the current tempo. Each beat
    // starts on a whole sample, so a bar can come out one sample longer.
    std::uint64_t samples_per_bar() const;

    // How long a beat lasts at the current tempo.
    BeatLength beat_length() const;

    // How long a beat lasts at the tempo asked for last. The beat starting
    // on the current sample plays at it, and so does the beat whenever it
    // starts again.
    BeatLength asked_beat_length() const;

    // How many beats have started since the beat started, before the
    // current sample. On a beat's own sample, that beat's number: 0 for the
    // first.
    std::uint64_t beat_number() const;

    // Moves on to the next sample.
    void step();

private:
    // Works out where the next beat starts. Called on the sample a beat
    // starts on.
    void plan_next_beat();

    // How many samples the engine makes in a minute: 48,000 a second, 60
    // seconds.
    std::uint64_t samples_per_minute_;

    // How many samples have been made so far. The current sample is the
    // next one to be made.
    std::uint64_t now_ = 0;

    bool running_ = false;
    int tempo_;       // beats per minute the beat plays at now
    int next_tempo_;  // the tempo asked for, from the next beat on

    // Each beat's start is worked out from the sample its tempo started on,
    // not by adding up beat lengths, so tiny roundings never add up into
    // drift.
    std::uint64_t tempo_start_ = 0;     // the sample the tempo started on
    std::uint64_t beats_at_tempo_ = 0;  // beats planned since then
    std::uint64_t next_beat_ = 0;       // the sample the next beat starts on

    // How many beats have started since the beat started, so we know where
    // each bar starts.
    std::uint64_t beats_started_ = 0;
};
