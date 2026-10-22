#include "wave.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// How far `phase` is past `point` in a cycle, from 0 to 1. Before the phase
// reaches `point`, the distance is counted from `point` in the cycle before.
double distance_past(double phase, double point) {
    double distance = phase - point;
    if (distance < 0.0) {
        distance += 1.0;
    }
    return distance;
}

// How much to bend a sample near a jump up from -1 to 1 (a "PolyBLEP"). An
// instant jump is made of harmonics that go on forever, and the ones too
// high for the sample rate fold back down as stray, off-key tones. Bending
// the sample just before the jump and the one just after it rounds the jump
// off and keeps those tones quiet. `distance` is how far the phase is past
// the jump, from 0 to 1, and `phase_step` is how far it moves each sample.
// Away from the jump the bend is 0. A jump down subtracts the bend.
double jump_bend(double distance, double phase_step) {
    if (distance < phase_step) {
        // The sample just after the jump: pull it down toward the middle.
        double nearness = 1.0 - distance / phase_step;
        return -(nearness * nearness);
    } else if (distance > 1.0 - phase_step) {
        // The sample just before the jump: push it up toward the middle.
        double nearness = 1.0 - (1.0 - distance) / phase_step;
        return nearness * nearness;
    } else {
        return 0.0;
    }
}

// How much to bend a sample near a corner where the slope turns up by 1
// height per sample (a "PolyBLAMP"). A corner is a sudden change in
// steepness, a jump in the slope, so it makes stray tones too, only quieter
// than a jump's. Bending the sample on each side of the corner rounds it
// off. Both move the same way, up toward where a smooth curve would pass.
// The bend is a jump's bend added up sample after sample, which is where
// the cube and the 6 come from. `distance` and `phase_step` work as in
// jump_bend. A turn down subtracts the bend.
double corner_bend(double distance, double phase_step) {
    if (distance < phase_step) {
        // The sample just after the corner.
        double nearness = 1.0 - distance / phase_step;
        return nearness * nearness * nearness / 6.0;
    } else if (distance > 1.0 - phase_step) {
        // The sample just before the corner.
        double nearness = 1.0 - (1.0 - distance) / phase_step;
        return nearness * nearness * nearness / 6.0;
    } else {
        return 0.0;
    }
}

// Swings smoothly up to 1, down to -1, then back to 0.
double sine(double phase) {
    return std::sin(2.0 * pi * phase);
}

// Rises from 0 to 1, falls to -1, then climbs back to 0, all in straight
// lines. It turns down a quarter of the way through each cycle and up at
// three quarters, and both corners are rounded off.
double triangle(double phase, double phase_step) {
    double height = 0.0;
    if (phase < 0.25) {
        height = 4.0 * phase;
    } else if (phase < 0.75) {
        height = 2.0 - 4.0 * phase;
    } else {
        height = 4.0 * phase - 4.0;
    }

    // The wave climbs 2 in half a cycle, a slope of 4 a cycle, and falls just
    // as steeply. So each corner turns the slope by 8 a cycle, or
    // 8 * phase_step a sample.
    double turn = 8.0 * phase_step;
    double top = distance_past(phase, 0.25);
    double bottom = distance_past(phase, 0.75);
    height -= turn * corner_bend(top, phase_step);     // turns down at the top
    height += turn * corner_bend(bottom, phase_step);  // turns up at the bottom
    return height;
}

// 1 for the first `width` of each cycle and -1 for the rest: at a width of
// a half, the square. It jumps up at the start of each cycle and down once
// `width` of it has passed, and both jumps are rounded off.
double square(double phase, double phase_step, double width) {
    double height = 0.0;
    if (phase < width) {
        height = 1.0;
    } else {
        height = -1.0;
    }

    double fall = distance_past(phase, width);
    height += jump_bend(phase, phase_step);  // the jump up at the start
    height -= jump_bend(fall, phase_step);   // the jump down

    // A pulse up for less than half of each cycle spends most of it below
    // the middle line, and one up for more, above it. The lean makes no
    // sound of its own, but the loudness envelope would turn it into a
    // thump as each note starts and ends. So the wave is moved by its
    // average, 2 * width - 1, to balance on the middle line. At a half it
    // stays where it is.
    height -= 2.0 * width - 1.0;
    return height;
}

// Rises from 0 to 1 over the first half of each cycle, drops to -1, then
// rises back to 0. The drop is rounded off.
double saw(double phase, double phase_step) {
    double height = 0.0;
    if (phase < 0.5) {
        height = 2.0 * phase;
    } else {
        height = 2.0 * phase - 2.0;
    }

    double halfway = distance_past(phase, 0.5);
    height -= jump_bend(halfway, phase_step);  // the drop halfway
    return height;
}

}  // namespace

double wave_height(Wave wave, double phase, double phase_step, double width) {
    if (wave == Wave::sine) {
        return sine(phase);
    } else if (wave == Wave::triangle) {
        return triangle(phase, phase_step);
    } else if (wave == Wave::square) {
        return square(phase, phase_step, width);
    } else {
        return saw(phase, phase_step);
    }
}

// Loudness here is a wave's RMS: square its heights, average them over a
// cycle, then take the square root. Unlike the highest point, it counts the
// whole wave, so it follows how loud a wave sounds much better. At full
// height the triangle and saw both come to 0.577, a sine to 0.707, and a
// square to 1, since it sits at full height all the time. Each trim brings
// its wave to the triangle's 0.577. Ears still heard square and saw as louder,
// since their bright upper harmonics stand out, so by ear in slice 5 they were
// turned down a further 2 dB (times 0.794). A narrower pulse, balanced on the
// middle line, comes to less than the square: 2 * sqrt(width * (1 - width)),
// so 0.6 at a width of 10%. It is left quieter, as on analog synths, and its
// brighter harmonics make up much of that to the ear.
float wave_trim(Wave wave) {
    if (wave == Wave::sine) {
        return 0.8165f;  // 0.577 / 0.707
    } else if (wave == Wave::triangle) {
        return 1.0f;
    } else if (wave == Wave::square) {
        return 0.4586f;  // 0.577 / 1, then 2 dB down
    } else {
        return 0.7943f;  // 2 dB down
    }
}
