#pragma once

#include <array>
#include <cstdint>

#include "sample_set.h"
#include "voice.h"

// The recordings a piano makes around its notes (docs/design.md#sampler): the
// dampers and strings as a key comes up, and the pedal going down and up.
// Each plays once beside the voices, so it never takes a note's voice. Only
// the audio thread uses it, so nothing in here locks, allocates or waits.
class ReleaseSounds {
public:
    // How many can sound at once. Lifting the pedal off a big chord starts
    // two for every note. Past this, a new one takes over the oldest.
    static constexpr int sound_count = 64;

    explicit ReleaseSounds(int sample_rate);

    // Plays `zone` of `set` once, `step` frames each sample, at `gain`, for
    // `source` and, for the loop, layer `layer`.
    void play(const SampleSet* set, const SampleZone* zone, double step, float gain, Source source, int layer);

    // The same for a pedal's sound. Its player's last one fades out, as one
    // pedal never makes two sounds at once.
    void play_pedal(const SampleSet* set, const SampleZone* zone, float gain, Source source, int layer);

    // Adds every sound to `frames` frames of stereo, adding instead of
    // overwriting. A loop's play at their layer's volume in `layer_volumes`.
    void render(float* out, int frames, const std::array<float, 8>& layer_volumes);

    // Sets the bit of every sample set a sound still plays.
    void add_sets_in_use(std::uint32_t& sets) const;

private:
    struct Sound {
        bool sounding = false;
        bool from_pedal = false;
        Source source = Source::live;
        int layer = 0;
        const SampleSet* set = nullptr;
        const SampleZone* zone = nullptr;
        double place = 0.0;
        double step = 1.0;
        float gain = 0.0f;
        // While fading out: how many samples of its fade are left. -1 while
        // it is not fading.
        int fade_left = -1;
        // A smaller number started earlier.
        std::uint64_t number = 0;
    };

    // A sound not playing, or the oldest when every one is.
    Sound& free_sound();

    std::array<Sound, sound_count> sounds_{};
    std::uint64_t started_ = 0;
    int fade_samples_;
};
