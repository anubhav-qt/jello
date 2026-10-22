#pragma once

#include <array>
#include <cstdint>

#include "envelope.h"
#include "filter.h"
#include "stereo.h"
#include "unison.h"
#include "wave.h"

// Who started a note: the player at the keys, the looper playing a loop back
// (docs/adr/0021-a-looper-on-a-beat.md), or the song player playing Compose's
// song (docs/adr/0022-compose-as-a-tape.md). Each voice remembers it, and for
// the loop which layer (docs/adr/0026-layers.md), so the loop's C, the song's
// C and a C played live never cut each other off, nor do two layers' Cs.
enum class Source {
    live,
    loop,
    song,
};

// Plays one note: a wave at the note's pitch, played by one or more copies
// (docs/adr/0029-unison.md), through its own low-pass filter
// (docs/adr/0028-a-low-pass-filter.md), shaped by its own envelope
// (docs/adr/0009-how-notes-become-sound.md). Only the audio thread touches a
// voice, so nothing in here locks, allocates or waits.
class Voice {
public:
    explicit Voice(int sample_rate);

    // Starts `note` (0 to 127, middle C is 60) at `velocity` (1 to 127),
    // playing `wave`, for `source`. A loop's note also says its `layer`, the
    // slot the looper gave its layer, 0 to 7; every other note says 0.
    // `press_number` says how old the note is: a smaller number was pressed
    // earlier. A silent voice starts at `volume`, from 0 to 1. If the voice is
    // still sounding, it is taken over without a click, even when `wave` is a
    // different wave, and its volume eases to the new one as it renders.
    // Its filter starts at `filter` and its filter envelope is `envelope`;
    // one still sounding keeps its filter's memory and glides on. Its copies
    // are tuned and placed by `unison`; each one carries on from where it
    // was in its cycle.
    void start(int note, int velocity, std::uint64_t press_number, Wave wave, Source source, int layer, float volume,
               const FilterTargets& filter, const FilterEnvelopeSettings& envelope, const UnisonSettings& unison);

    // Starts the fade to silence.
    void release();

    // True when the voice is silent and ready for a new note.
    bool is_free() const;

    // True while the voice fades out after its key was let go.
    bool is_releasing() const;

    // How loud the voice's envelope is right now, from 0 up to its peak.
    double level() const;

    // The note this voice is playing, or played last.
    int note() const;

    // The press number its note started with.
    std::uint64_t press_number() const;

    // Who started its note.
    Source source() const;

    // For a loop's note, its layer's slot.
    int layer() const;

    // Adds `frames` frames of sound to `out`, stereo (left, right, left, ...).
    // It adds instead of overwriting, so many voices can share one buffer.
    // The voice plays at `volume`, from 0 to 1. When that changes, it eases
    // there a step each sample, so turning a layer up or down never clicks.
    // Its filter glides toward `filter` the same way.
    void render(float* out, int frames, float volume, const FilterTargets& filter);

private:
    // One copy of the wave (docs/adr/0029-unison.md).
    struct Copy {
        double phase = 0.0;       // where it is in one cycle, from 0 to 1
        double phase_step = 0.0;  // how far the phase moves each sample
        // How much of it goes to each speaker: exactly 1 and 1 in the middle.
        double left = 1.0;
        double right = 1.0;
    };

    // Tunes and places the copies for `note` and `unison`. Their phases stay
    // where they are.
    void set_copies(int note, const UnisonSettings& unison);

    // The copies' heights at their current phases, added up on each side,
    // trimmed so every wave is equally loud.
    Stereo height_now() const;

    // How much of the gap is left on each side for the next sample. It
    // shrinks in a straight line: all of it on the first sample, a little
    // less on each one after, and none once its time is up.
    Stereo gap_now() const;

    int sample_rate_;
    int gap_samples_;  // how many samples a gap takes to shrink away
    Envelope envelope_;
    VoiceFilter filter_;
    int note_ = 0;
    std::uint64_t press_number_ = 0;
    Source source_ = Source::live;
    int layer_ = 0;
    Wave wave_ = Wave::triangle;  // the wave this voice's note started with

    // How loud the voice plays right now, from 0 to 1, and how far that
    // moves each sample on its way to a new volume.
    float volume_ = 1.0f;
    float volume_step_;

    // Every copy the voice can play, and how many its note plays: the first
    // `copy_count_`. Their sum is scaled by `copy_scale_`, 1 over the square
    // root of how many, so a note stays about as loud however many play it.
    std::array<Copy, unison_most_copies> copies_{};
    int copy_count_ = 1;
    double copy_scale_ = 1.0;

    // When a voice that is still sounding starts a new wave, the new wave can
    // be at a different height from where the old one was heading. The gap
    // between them is added on top of the new wave and shrinks away, so the
    // sound glides across instead of jumping. Each side has its own.
    Stereo gap_;                // the gap when it started shrinking
    int gap_samples_left_ = 0;  // samples until it is gone
};
