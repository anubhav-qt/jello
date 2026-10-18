#pragma once

#include <atomic>

// A steady test beep: a sine wave (the purest tone) at 440 Hz, the note A4,
// the A above middle C that orchestras tune to. Turning it on or off glides
// the volume over 10 ms so it never clicks.
class Tone {
public:
    explicit Tone(int sample_rate);

    // Safe to call from any thread.
    void set_on(bool on);

    // Fills `frames` frames of stereo (left, right, left, ...).
    // Runs on the audio thread, so it never locks, allocates or waits.
    void render(float* out, int frames);

private:
    std::atomic<bool> on_;

    double phase_;       // where we are in one wave, from 0 to 1
    double phase_step_;  // how far the phase moves each sample

    float volume_;       // the current volume, glides toward the target
    float volume_step_;  // how far the volume moves each sample
};
