#include "lfo.h"

#include <cmath>

namespace {

constexpr double pi = 3.14159265358979323846;

// A little nudge for working out which step a point is on. A step's start,
// worked out as a fraction like 1/3, can land a hair before it.
constexpr double step_nudge = 1e-9;

// Turns a number into one that looks unrelated to it: SplitMix64's mixing
// step, by Sebastiano Vigna (public domain). Numbers next to each other come
// out far apart.
std::uint64_t mix(std::uint64_t number) {
    std::uint64_t x = number + 0x9e3779b97f4a7c15ULL;
    x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9ULL;
    x = (x ^ (x >> 27)) * 0x94d049bb133111ebULL;
    return x ^ (x >> 31);
}

// A shape that holds one height for each stretch of its cycle and jumps
// between them, `into` of the way through a stretch, from 0 to 1. With
// smoothing, the first `smooth` share of the stretch slides from `before`,
// the height it jumps from, to `height`.
Slide hold(double before, double height, double into, double smooth) {
    if (into < smooth) {
        Slide slide;
        slide.from = before;
        slide.to = height;
        slide.done = into / smooth;
        return slide;
    }
    return Slide::at(height);
}

// Rises from 0 to 1, falls to -1, then climbs back to 0, all in straight
// lines, like the triangle wave (wave.cpp) without its rounded corners.
double triangle(double phase) {
    if (phase < 0.25) {
        return 4.0 * phase;
    } else if (phase < 0.75) {
        return 2.0 - 4.0 * phase;
    } else {
        return 4.0 * phase - 4.0;
    }
}

Slide square(double phase, double smooth) {
    if (phase < 0.5) {
        return hold(-1.0, 1.0, phase / 0.5, smooth);
    }
    return hold(1.0, -1.0, (phase - 0.5) / 0.5, smooth);
}

// The jump back down is the first `smooth` share of each cycle, and the rise
// the rest. Fully smoothed, it is all jump: a saw down.
Slide saw_up(double phase, double smooth) {
    if (phase < smooth) {
        Slide slide;
        slide.from = 1.0;
        slide.to = -1.0;
        slide.done = phase / smooth;
        return slide;
    }
    double risen = (phase - smooth) / (1.0 - smooth);
    return Slide::at(-1.0 + 2.0 * risen);
}

// A saw up upside down.
Slide saw_down(double phase, double smooth) {
    if (phase < smooth) {
        Slide slide;
        slide.from = -1.0;
        slide.to = 1.0;
        slide.done = phase / smooth;
        return slide;
    }
    double fallen = (phase - smooth) / (1.0 - smooth);
    return Slide::at(1.0 - 2.0 * fallen);
}

Slide steps(const LfoSettings& settings, double phase) {
    int count = settings.step_count;
    double place = phase * count;

    int step = static_cast<int>(std::floor(place + step_nudge));
    if (step > count - 1) {
        step = count - 1;
    }
    double into = place - step;
    if (into < 0.0) {
        into = 0.0;
    }

    // The step before the first is the last, as the steps go round.
    int before = step - 1;
    if (before < 0) {
        before = count - 1;
    }
    return hold(settings.steps[before], settings.steps[step], into, settings.smooth);
}

}  // namespace

Slide Slide::at(double value) {
    Slide slide;
    slide.from = value;
    slide.to = value;
    slide.done = 1.0;
    return slide;
}

double Slide::value() const {
    return from + (to - from) * done;
}

Slide lfo_height(const LfoSettings& settings, double cycles, std::uint64_t who, int lfo) {
    double whole = std::floor(cycles);
    double phase = cycles - whole;
    std::uint64_t cycle = static_cast<std::uint64_t>(whole);
    std::uint64_t what = static_cast<std::uint64_t>(lfo);

    if (settings.shape == LfoShape::sine) {
        return Slide::at(std::sin(2.0 * pi * phase));
    } else if (settings.shape == LfoShape::triangle) {
        return Slide::at(triangle(phase));
    } else if (settings.shape == LfoShape::square) {
        return square(phase, settings.smooth);
    } else if (settings.shape == LfoShape::saw_up) {
        return saw_up(phase, settings.smooth);
    } else if (settings.shape == LfoShape::saw_down) {
        return saw_down(phase, settings.smooth);
    } else if (settings.shape == LfoShape::random_steps) {
        double before = random_height(who, what, cycle - 1);
        double height = random_height(who, what, cycle);
        return hold(before, height, phase, settings.smooth);
    } else if (settings.shape == LfoShape::smooth_random) {
        // A half cosine from one height to the next, so it leaves and lands
        // gently.
        Slide slide;
        slide.from = random_height(who, what, cycle);
        slide.to = random_height(who, what, cycle + 1);
        slide.done = (1.0 - std::cos(pi * phase)) / 2.0;
        return slide;
    } else {
        return steps(settings, phase);
    }
}

double random_height(std::uint64_t who, std::uint64_t what, std::uint64_t when) {
    std::uint64_t x = mix(who);
    x = mix(x ^ what);
    x = mix(x ^ when);

    // The top 53 bits, as many as a double holds exactly, as a share from 0
    // to 1, then stretched to -1 to 1.
    double share = static_cast<double>(x >> 11) / 9007199254740992.0;
    return 2.0 * share - 1.0;
}
