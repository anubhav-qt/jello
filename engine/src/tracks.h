#pragma once

#include <array>
#include <cstdint>
#include <vector>

#include "synth.h"

// The song's tracks (docs/design.md#tracks-in-the-engine). A track is an
// instrument: a synth of its own, with its own sound, routing list, LFOs and
// drum kit, played only by the notes sent to it, and turned up or down by its
// own volume before the tracks are added together. All of them are made when
// the engine is made, so the audio thread never allocates. A silent voice
// costs nothing. Only the audio thread uses them, so nothing in here locks,
// allocates or waits.
class Tracks {
public:
    // How many tracks a song can have, one for each number key.
    static constexpr int count = 9;

    explicit Tracks(int sample_rate);

    // Track `track`'s synth, from 0. The caller makes sure it is below count.
    Synth& synth(int track);

    // How loud track `track` plays, from 0 (silent) to 1 (as played). It
    // eases there over 10 ms, so it never clicks. Every track starts at 1.
    void set_volume(int track, float volume);

    // The same as each synth's own, for every track at once.
    void all_notes_off(Source source);
    void set_beats_per_sample(double beats_per_sample);
    void on_beat(std::uint64_t number, double beats_per_sample);

    // The bit of every sample set any track still plays.
    std::uint32_t sets_in_use() const;

    // Adds every track to `frames` frames of stereo (left, right, left,
    // ...), each at its own volume. It adds instead of overwriting, like a
    // synth, so the conductor can render a chunk in pieces.
    void render(float* out, int frames);

private:
    // How many frames each track renders into its own buffer at a time.
    static constexpr int buffer_frames = 256;

    std::vector<Synth> synths_;

    // Each track's volume as asked for, and as heard now, easing there.
    std::array<float, count> volumes_{};
    std::array<float, count> heard_{};

    // How far a volume eases in one sample: all the way from 0 to 1 in
    // 10 ms.
    float volume_step_;

    // Where one track renders before it is turned up or down and added.
    std::vector<float> buffer_;
};
