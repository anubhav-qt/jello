#pragma once

// A note's loudness over time: ADSR, made of straight lines
// (docs/adr/0009-how-notes-become-sound.md).
//
//   attack:  rises to the peak when the note starts
//   decay:   falls to the sustain level
//   sustain: stays there while the key is held
//   release: falls to silence after the key is let go
//
// Every stage is one glide in a straight line from wherever the level is now,
// and always takes its full time, so the level never jumps.
class Envelope {
public:
    explicit Envelope(int sample_rate);

    // Starts the attack, gliding from the current level to `peak`.
    void start(float peak);

    // Starts the release, gliding from the current level to silence.
    // Does nothing if the envelope is already releasing or silent.
    void release();

    // Moves one sample forward and returns the new level.
    float next();

    // True before the first start, and again once the release has ended.
    bool is_silent() const;

private:
    enum class Stage {
        silent,
        attack,
        decay,
        sustain,
        release,
    };

    // Starts gliding from the current level to `target` over `samples`.
    void glide(Stage stage, double target, int samples);

    int attack_samples_;
    int decay_samples_;
    int release_samples_;

    Stage stage_ = Stage::silent;
    int samples_left_ = 0;  // samples until the glide reaches its target

    // Doubles, not floats: adding a tiny step 14,400 times in a float drifts
    // so far that the landing jumps and the release dips below zero.
    double level_ = 0.0;
    double peak_ = 0.0;    // the peak of the current note
    double target_ = 0.0;  // where the current glide ends
    double step_ = 0.0;    // how much the level moves each sample
};
