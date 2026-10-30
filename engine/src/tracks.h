#pragma once

#include <cstdint>
#include <vector>

#include "synth.h"

// The song's tracks (docs/design.md#tracks-in-the-engine). A track is an
// instrument: a synth of its own, with its own sound, routing list, LFOs and
// drum kit, played only by the notes sent to it. The mixer (mixer.h) renders
// them and turns each up or down. All of them are made when the engine is
// made, so the audio thread never allocates. A silent voice costs nothing.
// Only the audio thread uses them, so nothing in here locks, allocates or
// waits.
class Tracks {
public:
    // How many tracks a song can have, one for each number key.
    static constexpr int count = 9;

    explicit Tracks(int sample_rate);

    // Track `track`'s synth, from 0. The caller makes sure it is below count.
    Synth& synth(int track);

    // The same as each synth's own, for every track at once.
    void all_notes_off(Source source);
    void set_beats_per_sample(double beats_per_sample);
    void on_beat(std::uint64_t number, double beats_per_sample);

    // The bit of every sample set any track still plays.
    std::uint32_t sets_in_use() const;

private:
    std::vector<Synth> synths_;
};
