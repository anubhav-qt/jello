#pragma once

#include <cstdint>

// White noise (docs/adr/0035-oscillators-with-kinds.md): a new random
// height every sample, from -1 to 1, so every pitch at once, a hiss. Its
// heights look random but come from its start, so a render always sounds
// the same. Only the audio thread uses it, so nothing in here locks,
// allocates or waits.
class Noise {
public:
    // Starts a new run of heights, always the same for the same `who` and
    // `what`.
    void start(std::uint64_t who, std::uint64_t what);

    // Moves on to the next sample's height.
    void next();

    // The height for this sample, from -1 to 1.
    double height() const;

private:
    // Where it is in its run. Each sample adds the same big odd number, and
    // mixing turns that into a height (SplitMix64, by Sebastiano Vigna,
    // public domain).
    std::uint64_t state_ = 0;
    double height_ = 0.0;
};

// How much to scale noise so it is as loud as the triangle. Its heights
// are spread evenly from -1 to 1, so its RMS is the triangle's 0.577
// already. A starting value, to be tuned by ear.
constexpr float noise_trim = 1.0f;
