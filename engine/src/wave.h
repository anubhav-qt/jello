#pragma once

// The shapes a voice can play (docs/adr/0009-how-notes-become-sound.md).
// Every wave goes from -1 to 1 and crosses zero going up at the start of each
// cycle, so the main tones of all four line up.
enum class Wave {
    sine,      // only the main tone: pure and soft
    triangle,  // odd harmonics that fade fast: soft and hollow
    square,    // odd harmonics that fade slowly: hollow and reedy; thinner as its pulse narrows
    saw,       // every harmonic: bright and buzzy
};

// How high `wave` is at `phase`, where 0 is the start of a cycle and 1 is
// its end. `phase_step` is how far the phase moves each sample: the
// triangle, square and saw need it to round off their corners and jumps.
// `width` is how much of each cycle the square spends up, from 0.05 to 0.95
// (OscillatorSettings::width); the other waves leave it out.
double wave_height(Wave wave, double phase, double phase_step, double width);

// How much to scale `wave` so all four waves sound equally loud. Chosen by
// ear in slice 5.
float wave_trim(Wave wave);
