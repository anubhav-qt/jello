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
// and always takes its full time, so the level never jumps. Every note sets
// its stages as it starts: its loudness envelope's, or its filter envelope's.

// The shortest and longest any stage of the loudness envelope can take. At
// 0 ms a stage would jump the loudness in one sample, which clicks.
constexpr int loudness_shortest_ms = 1;
constexpr int loudness_longest_ms = 10000;

// The loudness envelope: how a note's loudness rises when it starts, falls
// to where it holds while the key is held, and fades out once it is let go.
// Each song has its own (docs/adr/0030-a-sound-profile-per-song.md).
// Changing it changes the notes started after. A new song starts here,
// checked by ear in slice 5: the attack made no audible difference between 2
// and 20 ms, and with the sustain at the full peak a held note never dips,
// so the decay has nothing to do until the sustain is turned down.
struct LoudnessEnvelopeSettings {
    int attack_ms = 5;
    int decay_ms = 300;
    // Where it holds while the key is held, as a share of the peak, from 0
    // to 1.
    float sustain = 1.0f;
    int release_ms = 300;
};

class Envelope {
public:
    explicit Envelope(int sample_rate);

    // Sets how long the attack, decay and release take, and the sustain
    // level as a share of the peak, from 0 to 1. Each stage started after
    // this uses them; the one under way finishes as it began.
    void set_stages(int attack_ms, int decay_ms, double sustain, int release_ms);

    // Moves the sustain level while the note plays, as a share of the peak,
    // from 0 to 1 (docs/adr/0031-lfos-and-a-routing-list.md). A decay under
    // way heads for it instead, and a held sustain glides there over
    // `samples`. The same level as now changes nothing.
    void set_sustain(double sustain, int samples);

    // Starts the attack, gliding from the current level to `peak`.
    void start(float peak);

    // Starts the release, gliding from the current level to silence.
    // Does nothing if the envelope is already releasing or silent.
    void release();

    // Moves one sample forward and returns the new level.
    float next();

    // True before the first start, and again once the release has ended.
    bool is_silent() const;

    // True from the moment the release starts until it has ended.
    bool is_releasing() const;

    // The level right now, from 0 up to the peak.
    double level() const;

    // The level right now as a share of the peak, from 0 to 1.
    double share() const;

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

    int sample_rate_;
    // Set by set_stages, which the constructor calls first.
    int attack_samples_ = 0;
    int decay_samples_ = 0;
    double sustain_ = 0.0;  // a share of the peak
    int release_samples_ = 0;

    Stage stage_ = Stage::silent;
    int samples_left_ = 0;  // samples until the glide reaches its target

    // Doubles, not floats: adding a tiny step 14,400 times in a float drifts
    // so far that the landing jumps and the release dips below zero.
    double level_ = 0.0;
    double peak_ = 0.0;    // the peak of the current note
    double target_ = 0.0;  // where the current glide ends
    double step_ = 0.0;    // how much the level moves each sample
};
