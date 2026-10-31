#pragma once

#include <cstdint>
#include <vector>

#include "stereo.h"

// A sampled instrument's recordings (docs/design.md#sampler): a piano, a
// drum kit, anything recorded. Rust reads the files and sends them in, all
// at the engine's rate; the engine keeps its own copy. A set is made and
// freed off the audio thread and never changes in between, so the audio
// thread only ever reads it.

// When a recording plays: as a note starts, as its key comes up, or as the
// pedal goes down or up.
enum class SampleTrigger {
    attack = 0,
    release = 1,
    pedal_down = 2,
    pedal_up = 3,
};

// One recording and the notes it plays.
struct SampleZone {
    // The notes it plays, 0 to 127, both ends included. A drum kit's zones
    // name its pads instead (drums.h).
    int lowest_note = 0;
    int highest_note = 127;
    // The note it was recorded at: played there, it sounds as recorded.
    int root_note = 60;
    // The touches it was recorded for, from 1 (softest) to 127 (hardest),
    // both ends included (OscillatorSettings::touch).
    int lowest_touch = 1;
    int highest_touch = 127;
    SampleTrigger trigger = SampleTrigger::attack;
    // For a release: how many decibels quieter it starts for each second
    // the note sounded, as its strings have rung down, and the share of its
    // loudness a soft touch takes away, from 0 to 1.
    float fade_db_per_second = 0.0f;
    float touch_share = 0.0f;
    // How far off that note the recording is, in cents, to tune it back.
    float tune_cents = 0.0f;
    // 1 for mono, 2 for stereo (left, right, left, ...).
    int channels = 1;
    std::uint64_t frame_count = 0;
    // Where it loops while held, in frames: from loop_start up to just
    // before loop_end. A loop_end of 0 is no loop: it plays once.
    std::uint64_t loop_start = 0;
    std::uint64_t loop_end = 0;
    std::vector<float> frames;

    bool loops() const;
};

class SampleSet {
public:
    // `id` is the set's place in the engine's table, from 0, so the audio
    // thread can say which sets it still plays.
    explicit SampleSet(int id);

    int id() const;

    // Adds a zone, copying `frame_count` frames of `channels` channels
    // from `frames`. Returns false, adding nothing, if anything is out of
    // range. Never on the audio thread: it allocates.
    bool add_zone(const SampleZone& settings, const float* frames);

    // The zone that plays `note` at `touch`: the first that includes both,
    // or else the one nearest to the note, one made for the touch first.
    // nullptr when the set has no zones.
    const SampleZone* zone_for(int note, int touch) const;

    // The zone that plays exactly `note`, or nullptr. For drum pads, where
    // the nearest one would be a different drum.
    const SampleZone* exact_zone_for(int note) const;

    // The zones that play as a key comes up, and as the pedal goes down or
    // up. Every release zone for a note and touch plays at once.
    const std::vector<SampleZone>& release_zones() const;
    const std::vector<SampleZone>& pedal_zones(bool down) const;

    // Whether `zone` is for `note` and `touch`.
    static bool plays(const SampleZone& zone, int note, int touch);

private:
    int id_;
    std::vector<SampleZone> zones_;
    std::vector<SampleZone> release_zones_;
    std::vector<SampleZone> pedal_down_zones_;
    std::vector<SampleZone> pedal_up_zones_;
};

// How many sets the engine keeps at once: one bit each in a 32-bit mask.
constexpr int sample_set_most = 32;

// The sound of `zone` at `place` frames in, between its frames: read with
// 4-point Hermite interpolation, which keeps the tone clean when the
// recording plays faster or slower than it was made. A mono zone sounds the
// same on both sides. Past the end of a zone that plays once it is silent;
// a looping zone reads on round its loop.
Stereo read_zone(const SampleZone& zone, double place);

// `place` moved on `step` frames through `zone`: round its loop if it
// loops, or held at its end, where it is silent.
double move_through(const SampleZone& zone, double place, double step);

// True once `place` is past the end of a zone that plays once.
bool is_past_end(const SampleZone& zone, double place);
