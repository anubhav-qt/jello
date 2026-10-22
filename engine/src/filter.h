#pragma once

#include "envelope.h"
#include "stereo.h"

// The filter (docs/adr/0028-a-low-pass-filter.md). Every voice puts its wave
// through one of its own, before the loudness envelope, so each note can open
// and close on its own. It started as a low-pass filter, which lets low
// sounds through and turns high ones down: the harmonics above its cutoff,
// the quieter, higher copies of a note that make it bright. Now it has four
// types (docs/design.md#filter-types).

// What the filter keeps. The same filter works out every type at once, so
// each is just a different part of what it hears.
enum class FilterType {
    low_pass,   // lets the lows through and turns the highs down
    high_pass,  // lets the highs through and turns the lows down
    band_pass,  // keeps a band around the cutoff and turns both sides down
    notch,      // cuts a narrow band out at the cutoff and keeps the rest
};

// How many types there are, for stepping through them.
constexpr int filter_type_count = 4;

// The lowest cutoff, and the highest, where a low-pass is open: it lets
// everything through, so the filter is off and the sound is exactly as it
// would be without it. A high-pass is open at the lowest cutoff. A band-pass
// and a notch never are.
constexpr float filter_lowest_hz = 20.0f;
constexpr float filter_open_hz = 20000.0f;

// The most the filter envelope can open the cutoff, in octaves, and the
// longest any of its stages can take.
constexpr float filter_most_octaves = 8.0f;
constexpr int filter_longest_ms = 10000;

// Where the filter's controls are. Moving them changes every note, even
// those already sounding.
struct FilterSettings {
    FilterType type = FilterType::low_pass;
    // Middle C's cutoff, in Hz, from filter_lowest_hz up to filter_open_hz.
    // Every other note's cutoff follows it, as far as the song's key
    // tracking says (NoteSound::key_tracking in voice.h).
    float cutoff_hz = filter_open_hz;
    // How much the filter boosts the sound right at its cutoff, from 0 (no
    // bump) to 1 (a strong, singing peak).
    float resonance = 0.0f;
};

// The filter envelope: an ADSR of its own that opens the cutoff when a note
// starts and closes it again. Changing it changes the notes started after.
struct FilterEnvelopeSettings {
    // How far it opens the cutoff at its peak, in octaves above the
    // cutoff, from 0 to filter_most_octaves. At 0 it changes nothing.
    float amount = 0.0f;
    int attack_ms = 5;
    int decay_ms = 300;
    // Where it holds while the key is held, as a share of the amount, from
    // 0 to 1.
    float sustain = 0.0f;
    int release_ms = 300;
};

// The settings as the voices use them, worked out once whenever they change
// instead of on every sample.
struct FilterTargets {
    FilterType type = FilterType::low_pass;
    // Middle C's cutoff as an octave number: log2 of its Hz. Cutoffs move in
    // octaves, not Hz, so a glide moves in equal musical steps.
    double cutoff = 0.0;
    double resonance = 0.0;
    // How much of the sound goes through the filter: 0 when it is open, 1
    // otherwise.
    double share = 0.0;
};

FilterTargets filter_targets_of(const FilterSettings& settings);

// How far the routing list moves one note's filter right now
// (docs/adr/0031-lfos-and-a-routing-list.md). Each is added on top, and the
// sum kept within the filter's ranges.
struct FilterMoves {
    double cutoff = 0.0;     // octaves
    double resonance = 0.0;  // a share
    double amount = 0.0;     // octaves of the filter envelope's amount
    double type = 0.0;       // whole steps through the types, and round again
};

// `type` moved `steps` steps through low-pass, high-pass, band-pass and
// notch, round and round. Steps are rounded to whole ones.
FilterType moved_filter_type(FilterType type, double steps);

// A value that moves to a new target in a straight line over a set number of
// samples, so a control moved in one jump is heard as a quick, smooth move
// instead of a click.
class Glide {
public:
    explicit Glide(int samples);

    // Jumps straight to `value`, with nothing left to glide.
    void jump_to(double value);

    // Moves one sample toward `target` and returns the value. A target
    // different from the last one starts a new glide from wherever the
    // value is now.
    double next(double target);

private:
    int samples_;
    int samples_left_ = 0;
    double value_ = 0.0;
    double target_ = 0.0;
    double step_ = 0.0;  // how far the value moves each sample
};

// How much of each of the filter's three parts comes out: the low part, the
// band around the cutoff, and the high part. Each type is one mix of them
// (part_mix_of), and moving between types glides from one mix to the other.
struct PartMix {
    double low = 1.0;
    double band = 0.0;
    double high = 0.0;
};

// The mix that makes `type`: a notch is the low and high parts together,
// everything but the band.
PartMix part_mix_of(FilterType type);

// A state variable filter, 12 dB per octave: one octave past the cutoff, a
// harmonic comes out about 4 times quieter. On every sample it splits what
// it hears into a low part, a band around the cutoff and a high part, and a
// PartMix says how much of each comes out. Low-pass, high-pass and
// band-pass are all as loud right at the cutoff, as loud as the resonance
// makes them. It is built the zero-delay-feedback way, from Andrew Simper's
// published math (Cytomic), so it stays smooth and steady even when its
// cutoff moves on every sample. It filters the left and right sides through
// the same cutoff and resonance, each with a memory of its own
// (docs/adr/0029-unison.md).
class StateVariableFilter {
public:
    // Sets the cutoff, in Hz, and the resonance, from 0 to 1. The filter
    // remembers what it was doing, so the sound carries on smoothly.
    void set(double cutoff_hz, double resonance, int sample_rate);

    // Filters one moment of sound, each side on its own, and mixes its
    // parts as `mix` says.
    Stereo next(Stereo input, const PartMix& mix);

    // Forgets everything it heard, as if new.
    void clear();

private:
    // What one side remembers between samples: the charge left in the
    // filter's two stages, like the two capacitors of an analog filter.
    struct Memory {
        double first = 0.0;
        double second = 0.0;
    };

    // Filters one sample of one side, with that side's memory.
    double next_side(double input, Memory& memory, const PartMix& mix) const;

    // Worked out from the cutoff and resonance by set. k is how much of the
    // band the filter feeds back, which sets how big the resonance's bump
    // is: the smaller, the bigger.
    double a1_ = 0.0;
    double a2_ = 0.0;
    double a3_ = 0.0;
    double k_ = 0.0;

    Memory left_;
    Memory right_;
};

// One voice's filter: its state variable filter for each side, its filter
// envelope, and the glides that keep a moved control or type from clicking.
class VoiceFilter {
public:
    explicit VoiceFilter(int sample_rate);

    // Starts `note` with `envelope`. Its cutoff follows middle C's by
    // `key_tracking`, from 0 to 1: at 1, an octave higher for each octave
    // the note is above middle C, and at 0, not at all. `fresh` is true for
    // a silent voice: it starts right at `targets`, its type moved
    // `type_steps` by the routing list, with nothing left over from its last
    // note. A voice still sounding keeps its memory and glides on to the new
    // note's cutoff and type, so it never clicks.
    void start(int note, bool fresh, const FilterTargets& targets, const FilterEnvelopeSettings& envelope,
               float key_tracking, double type_steps);

    // Starts the filter envelope's release.
    void release();

    // Moves the filter envelope's sustain while the note plays
    // (Envelope::set_sustain).
    void set_sustain(double sustain, int samples);

    // Where the filter envelope is right now, from 0 to 1.
    double envelope_level() const;

    // Filters one moment of the wave, both sides, gliding a step toward
    // `targets`, moved by `moves`.
    Stereo next(Stereo wave, const FilterTargets& targets, const FilterMoves& moves);

private:
    int sample_rate_;
    double highest_hz_;  // the cutoff never goes past this
    StateVariableFilter filter_;
    Envelope envelope_;

    // How many octaves this note's cutoff sits above middle C's, and how
    // far its filter envelope opens it at the peak.
    double tracking_ = 0.0;
    double amount_ = 0.0;

    Glide cutoff_;
    Glide resonance_;
    Glide share_;

    // How much of each part comes out, each gliding to the mix of the type
    // it is moved to, so a new type fades in.
    Glide low_;
    Glide band_;
    Glide high_;

    // The cutoff, in octaves, and resonance the filter was last set to. Its
    // math is only worked out again when one of them moves, or before it is
    // first set.
    bool filter_ready_ = false;
    double set_cutoff_ = 0.0;
    double set_resonance_ = 0.0;
};
