#pragma once

#include "envelope.h"

// Plays one note: a triangle wave at the note's pitch, shaped by its own
// envelope (docs/adr/0009-how-notes-become-sound.md). Only the audio thread
// touches a voice, so nothing in here locks, allocates or waits.
class Voice {
public:
    explicit Voice(int sample_rate);

    // Starts `note` (0 to 127, middle C is 60) at `velocity` (1 to 127).
    // If the voice is still sounding, it is taken over without a click.
    void start(int note, int velocity);

    // Starts the fade to silence.
    void release();

    // True when the voice is silent and ready for a new note.
    bool is_free() const;

    // The note this voice is playing, or played last.
    int note() const;

    // Adds `frames` frames of sound to `out`, stereo (left, right, left, ...).
    // It adds instead of overwriting, so many voices can share one buffer.
    void render(float* out, int frames);

private:
    int sample_rate_;
    Envelope envelope_;
    int note_ = 0;

    double phase_ = 0.0;       // where we are in one wave, from 0 to 1
    double phase_step_ = 0.0;  // how far the phase moves each sample
};
