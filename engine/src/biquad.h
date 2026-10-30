#pragma once

#include "stereo.h"

// A biquad: the small filter most EQs are built from, two samples of memory
// each side. One biquad turns the sound up or down around a frequency in
// one shape; several in a row make steeper cuts. The shapes' math is
// Robert Bristow-Johnson's Audio EQ Cookbook, the usual published recipes.
class Biquad {
public:
    // Each sets the shape. Frequencies are in Hz, gains in dB, and q is how
    // sharp the shape is. The memory stays, so the sound carries on.
    void set_low_pass(double hz, double q, int sample_rate);
    void set_high_pass(double hz, double q, int sample_rate);
    void set_low_shelf(double hz, double gain_db, double q, int sample_rate);
    void set_high_shelf(double hz, double gain_db, double q, int sample_rate);
    void set_bell(double hz, double gain_db, double q, int sample_rate);
    void set_notch(double hz, double q, int sample_rate);

    // Filters one moment of sound, each side with its own memory.
    Stereo next(Stereo input);

    // Forgets everything it heard, as if new.
    void clear();

private:
    // What one side remembers between samples.
    struct Memory {
        double first = 0.0;
        double second = 0.0;
    };

    // Filters one sample of one side.
    double next_side(double input, Memory& memory) const;

    // Takes the cookbook's six numbers and keeps them scaled so a0 is 1.
    void use(double b0, double b1, double b2, double a0, double a1, double a2);

    double b0_ = 1.0;
    double b1_ = 0.0;
    double b2_ = 0.0;
    double a1_ = 0.0;
    double a2_ = 0.0;

    Memory left_;
    Memory right_;
};
