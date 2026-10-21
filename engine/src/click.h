#pragma once

// The click: a short tick on every beat, higher on the first beat of each
// bar (docs/adr/0021-a-looper-on-a-beat.md). It is its own sound, not one
// of the synth's voices, so it never takes a note's voice. Only the audio
// thread uses it, so nothing in here locks, allocates or waits.
class Click {
public:
    explicit Click(int sample_rate);

    // Starts a tick. `high` makes it the higher one, for the first beat of
    // a bar.
    void start(bool high);

    // The tick's next sample, or 0 when no tick is sounding.
    float next_sample();

private:
    int sample_rate_;
    int tick_samples_;          // how many samples a tick lasts
    double fade_;               // what the level is multiplied by each sample
    int samples_left_ = 0;      // until the tick ends
    double level_ = 0.0;        // how loud the tick is right now
    double phase_ = 0.0;        // where we are in one cycle, from 0 to 1
    double phase_step_ = 0.0;   // how far the phase moves each sample
};
