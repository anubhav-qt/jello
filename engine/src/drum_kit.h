#pragma once

#include <array>
#include <cstdint>

#include "sample_set.h"
#include "voice.h"

// The drum kit (docs/design.md#sampler): an instrument beside the synth,
// with voices of its own, so drums and chords play together. Each pad plays
// its recording once, to its end, whatever the key does; a pad in a choke
// group cuts off the others sounding in it, like a closed hi-hat stopping an
// open one. Only the audio thread uses it, so nothing in here locks,
// allocates or waits.

// The drum pads, played as the notes after the vibrato and glide keys', so
// the looper records, snaps and plays them back like any note.
constexpr int drum_first_note = 130;
constexpr int drum_pad_count = 16;

// True for a note that hits a drum pad.
bool is_drum_note(int note);

// The most a pad can be tuned either way, in half steps, and the choke
// groups there are, from 1; 0 is none.
constexpr int drum_most_half_steps = 12;
constexpr int drum_most_group = 4;

// One pad's settings. A hit keeps the ones it started with.
struct DrumPad {
    // How loud, from 0 (silent) to 1 (as recorded).
    float level = 1.0f;
    // How far it is tuned, in half steps, from minus to plus the most.
    int half_steps = 0;
    // Where it sits, from -1 (left) to 1 (right).
    float pan = 0.0f;
    // Its choke group, from 0 (none) to drum_most_group.
    int group = 0;
};

class DrumKit {
public:
    // How many hits can sound at once.
    static constexpr int voice_count = 16;

    explicit DrumKit(int sample_rate);

    // Hits after this play `set`'s zones, one per pad, or nothing for
    // nullptr. Hits already sounding finish on the set they started on.
    void set_kit(const SampleSet* set);

    // Hits on pad `pad`, from 0, after this sound as `settings` says.
    void set_pad(int pad, const DrumPad& settings);

    // Hits pad `pad` at `velocity` (1 to 127), for `source` and, for the
    // loop, layer `layer`.
    void hit(int pad, int velocity, Source source, int layer);

    // Adds every hit sounding to `frames` frames of stereo, adding instead
    // of overwriting. A loop's hits play at their layer's volume in
    // `layer_volumes`, each easing there so it never clicks.
    void render(float* out, int frames, const std::array<float, 8>& layer_volumes);

    // Sets the bit of every sample set the kit or a hit still plays.
    void add_sets_in_use(std::uint32_t& sets) const;

private:
    struct Hit {
        bool sounding = false;
        int pad = 0;
        int group = 0;
        Source source = Source::live;
        int layer = 0;
        const SampleSet* set = nullptr;
        const SampleZone* zone = nullptr;
        double place = 0.0;
        double step = 1.0;
        // How loud each side is, with the velocity and the pad's level.
        float left = 0.0f;
        float right = 0.0f;
        // The layer volume it plays at now, easing to the one asked for.
        float volume = 1.0f;
        // While cut off by its choke group: how many samples of its fade
        // are left. -1 while it is not fading.
        int fade_left = -1;
        // Smaller hit earlier.
        std::uint64_t number = 0;
    };

    // A hit not sounding, or the oldest when every one is.
    Hit& free_hit();

    // How loud `hit` should play: its layer's for a loop's hit, 1 for any
    // other.
    static float volume_for(const Hit& hit, const std::array<float, 8>& layer_volumes);

    const SampleSet* kit_ = nullptr;
    std::array<DrumPad, drum_pad_count> pads_{};
    std::array<Hit, voice_count> hits_{};
    std::uint64_t hit_count_ = 0;
    int fade_samples_;
    float volume_step_;
};
