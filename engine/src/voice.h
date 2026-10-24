#pragma once

#include <array>
#include <cstdint>

#include "envelope.h"
#include "filter.h"
#include "lfo.h"
#include "noise.h"
#include "oscillator.h"
#include "routing.h"
#include "sample_set.h"
#include "stereo.h"
#include "unison.h"

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

// The player (routing.h) that plays `source`'s notes: for the loop, the
// layer in slot `layer`, 0 to 7.
int player_of(Source source, int layer);

// The lowest and highest A4 a song can be tuned to, in Hz: a half step below
// and above 440, about. Further than that is a change of key.
constexpr float tuning_lowest_hz = 415.0f;
constexpr float tuning_highest_hz = 466.0f;

// The longest a glide can take, in ms (docs/adr/0036-glide.md).
constexpr int glide_longest_ms = 10000;

// How a note sounds, all of which it starts with and keeps until it ends:
// changing any of it changes the notes started after. Each part is the
// song's own (docs/adr/0030-a-sound-profile-per-song.md), and a new song
// starts with these.
struct NoteSound {
    // The oscillators every note plays (docs/adr/0035-oscillators-with-kinds.md).
    std::array<OscillatorSettings, oscillator_count> oscillators = start_oscillators();
    // The instrument each one plays when it is a sample, or nullptr for
    // none, which is silent (docs/design.md#sampler).
    std::array<const SampleSet*, oscillator_count> instruments{};
    LoudnessEnvelopeSettings loudness_envelope{};
    FilterEnvelopeSettings filter_envelope{};
    // How much the filter's cutoff follows the note, from 0 to 1. At 1, it
    // moves an octave up for each octave the note is above middle C, so
    // every note is equally bright. At 0, every note has middle C's cutoff,
    // so higher notes sound darker.
    float key_tracking = 1.0f;
    UnisonSettings unison{};
    // A4's pitch in Hz, from tuning_lowest_hz to tuning_highest_hz. Every
    // other note is tuned from it.
    float a4_hz = 440.0f;
    // How long a melody note takes to slide to its pitch while the glide key
    // is held (docs/adr/0036-glide.md), from 0 (it jumps) to
    // glide_longest_ms. Without the key, notes never slide.
    int glide_ms = 100;
};

// Who starts a note, and when.
struct NoteStart {
    // Who plays it. A loop's note also says its `layer`, the slot the looper
    // gave its layer, 0 to 7; every other note says 0.
    Source source = Source::live;
    int layer = 0;
    // How old the note is: a smaller number was pressed earlier. Every note
    // gets its own.
    std::uint64_t press_number = 0;
    // Its player's key press, from 0, shared by the notes of one chord
    // (Routing::press).
    std::uint64_t press = 0;
    // The sample it starts on, counted from the engine's first.
    std::uint64_t sample = 0;
};

// Plays one note: its oscillators, each tuned from the note's pitch
// (docs/adr/0035-oscillators-with-kinds.md), played by one or more copies
// (docs/adr/0029-unison.md), through its own low-pass filter
// (docs/adr/0028-a-low-pass-filter.md), shaped by its own envelope
// (docs/adr/0009-how-notes-become-sound.md), and moved by the song's
// routing list (docs/adr/0031-lfos-and-a-routing-list.md). Only the audio
// thread touches a voice, so nothing in here locks, allocates or waits.
class Voice {
public:
    explicit Voice(int sample_rate);

    // Starts `note` (0 to 127, middle C is 60) at `velocity` (1 to 127),
    // sounding as `sound` says, moved by `routing`, for the player and at
    // the moment `start` says. A silent voice starts at `volume`, from 0 to
    // 1. If the voice is still sounding, it is taken over without a click,
    // even when the kinds are different ones, and its volume eases to the
    // new one as it renders. Its filter starts at `filter`; one still
    // sounding keeps its filter's memory and glides on. Its copies'
    // oscillators each carry on from where they were in their cycles.
    void start(int note, int velocity, const NoteStart& start, float volume, const FilterTargets& filter,
               const NoteSound& sound, const Routing& routing);

    // Slides the note just started in from `from_hz` (docs/adr/0036-glide.md):
    // its pitch starts there and slides to the note's over `glide_ms`, as
    // the routing list moves the glide time. Only right after start, on the
    // same sample, before it renders.
    void slide_in(double from_hz, int glide_ms, const Routing& routing);

    // Bends the note this voice plays to `note`, tuned from A4 at `a4_hz`,
    // and gives it `press_number`. Nothing starts again: its envelopes, its
    // LFOs and its kinds carry on, as one note. Its pitch slides from
    // wherever it is on sample `now` to the new note's over `glide_ms`, as
    // the routing list moves the glide time, from its next 32 samples on.
    void bend_to(int note, std::uint64_t press_number, std::uint64_t now, double a4_hz, int glide_ms,
                 const Routing& routing);

    // Works out how far the routing list moves the note at sample
    // `block_end`, the end of the next control_samples samples, and heads
    // there in straight lines as those samples render. `routing` has just
    // been moved on to `block_end` too.
    void tick(const Routing& routing, std::uint64_t block_end);

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

    // That note's own pitch, in Hz, where a slide to it ends.
    double note_hz() const;

    // Where its pitch is on sample `now`, in Hz, partway along a slide,
    // before the routing list moves it.
    double pitch_at(std::uint64_t now) const;

    // The press number its note started with.
    std::uint64_t press_number() const;

    // Who started its note.
    Source source() const;

    // For a loop's note, its layer's slot.
    int layer() const;

    // Sets the bit of every sample set the voice still plays, while it
    // sounds.
    void add_sets_in_use(std::uint32_t& sets) const;

    // Adds `frames` frames of sound to `out`, stereo (left, right, left, ...).
    // It adds instead of overwriting, so many voices can share one buffer.
    // The voice plays at `volume`, from 0 to 1. When that changes, it eases
    // there a step each sample, so turning a layer up or down never clicks.
    // Its filter glides toward `filter` the same way.
    void render(float* out, int frames, float volume, const FilterTargets& filter);

private:
    // A value that moves in a straight line to where it is sent, a step
    // each sample, so a move worked out every control_samples samples never
    // jumps.
    struct Line {
        double value = 0.0;
        double target = 0.0;
        double step = 0.0;

        // Lands on the last target, then heads for `to` over `samples`. At
        // 0 samples, it jumps there.
        void head_to(double to, int samples);

        // Moves one sample along.
        void next();
    };

    // One copy of the note (docs/adr/0029-unison.md). It plays every
    // oscillator, each at its own pitch, from the same place between the
    // speakers.
    struct Copy {
        // Where each oscillator is in one cycle, from 0 to 1, and how far
        // that moves each sample.
        // A sample oscillator's phase is where it is in its recording, in
        // frames instead, and its step how many frames it moves.
        std::array<double, oscillator_count> phases{};
        std::array<Line, oscillator_count> phase_steps{};
        // Each oscillator's own noise, so no two copies or oscillators hiss
        // alike. Only a noise oscillator moves it on.
        std::array<Noise, oscillator_count> noises{};
        // How much of it goes to each speaker: exactly 1 and 1 in the middle.
        Line left;
        Line right;
    };

    // How far every target is moved, in its own units, the routing list's
    // connections added up.
    struct Moves {
        std::array<double, target_count> by_target{};

        double of(RoutingTarget target) const;
        void add(RoutingTarget target, double move);
    };

    // Every source's reading, both ways, and how far each has faded in.
    struct Readings {
        std::array<Slide, source_count> slides{};
        std::array<double, source_count> fades{};

        void set(RoutingSource source, Slide slide, double fade);
        Slide slide(RoutingSource source) const;
        double fade(RoutingSource source) const;
    };

    // Reads every source now, `age_ms` after the note started.
    Readings read_sources(const Routing& routing, double age_ms) const;

    // Where LFO `index` is for this note.
    Slide read_lfo(const Routing& routing, int index) const;

    // Adds up every connection's move. The live targets are moved at once,
    // faded in; the targets read as a note starts only when `at_start` is
    // true, and never faded.
    Moves add_moves(const Routing& routing, const Readings& readings, bool at_start) const;

    // `sound` with the targets read as a note starts moved by `moves`.
    NoteSound moved_sound(const NoteSound& sound, const Moves& moves) const;

    // Heads every live setting for where `moves` puts it on sample
    // `sample`, over `samples`, or jumps there at 0.
    void head_for(const Moves& moves, int samples, std::uint64_t sample);

    // The glide time `glide_ms` as the routing list moves it, read
    // `age_ms` after the note started.
    int moved_glide_ms(int glide_ms, const Routing& routing, double age_ms) const;

    // Starts the pitch's slide on sample `sample`, from `from_hz` to the
    // note's own, over `glide_ms`. At 0 ms there is no slide.
    void start_glide(double from_hz, std::uint64_t sample, int glide_ms);

    // How many cents the pitch is from the note's own on sample `sample`,
    // partway along its slide: all of the way at its start, none at its
    // end.
    double glide_cents_at(std::uint64_t sample) const;

    // How high oscillator `index` of `copy` is right now on each side,
    // trimmed so every kind is equally loud, before its level. Only a
    // stereo recording differs between the sides.
    Stereo oscillator_height(const Copy& copy, int index) const;

    // Picks oscillator `index`'s zone in `set` for `note` and starts every
    // copy at the start of its recording. With no set or zone it is silent.
    void start_sample(int index, int note, const OscillatorSettings& oscillator, const SampleSet* set);

    // Moves sample oscillator `index` of `copy` on by its step, round its
    // loop or up to its end.
    void move_sample(Copy& copy, int index);

    // The copies' heights at their current phases, each the sum of its
    // oscillators at their levels, added up on each side.
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
    int player_ = 0;

    // The kinds this voice's note started with, one for each oscillator.
    std::array<Kind, oscillator_count> kinds_{};

    // For a sample oscillator: its instrument, the zone that plays the
    // note, or nullptr for silence, and the pitch it sounds at as recorded,
    // in Hz.
    std::array<const SampleSet*, oscillator_count> sets_{};
    std::array<const SampleZone*, oscillator_count> zones_{};
    std::array<double, oscillator_count> root_hz_{};

    // What the note started with, which the live moves are added to: its
    // pitch in Hz, how far each oscillator is tuned from it in cents, its
    // level and its width, its copies' detune and spread, its key tracking,
    // and its envelopes' sustains and filter envelope amount.
    double hz_ = 0.0;
    std::array<double, oscillator_count> offset_cents_{};
    std::array<double, oscillator_count> levels_{};
    std::array<double, oscillator_count> widths_{};
    double detune_cents_ = 0.0;
    double spread_ = 0.0;
    double key_tracking_ = 1.0;
    double loudness_sustain_ = 1.0;
    double filter_sustain_ = 0.0;

    // The sample its note started on, the sample its own LFOs have been
    // moved on to, and how many beats have passed since it started.
    std::uint64_t start_sample_ = 0;
    std::uint64_t lfo_clock_ = 0;
    double age_beats_ = 0.0;

    // Its own copy of each per-note LFO: where it started, how far it has
    // gone since its cycle 0 began, and how far its speed is moved, in
    // octaves. For LFOs in presses, the height its press gave it.
    std::array<double, lfo_count> lfo_starts_{};
    std::array<double, lfo_count> lfo_cycles_{};
    std::array<double, lfo_count> lfo_speed_moves_{};
    std::array<double, lfo_count> press_heights_{};

    // The note's random number and its height (RoutingSource).
    double random_ = 0.0;
    double note_height_ = 0.0;

    // The note it started with, which its filter's key tracking was worked
    // out from. A bend to another note leaves it, and moves the cutoff along
    // with the pitch instead.
    int tracking_note_ = 0;

    // Its glide (docs/adr/0036-glide.md): how many cents its pitch started
    // its slide from the note's own, the sample the slide started on, and
    // how many samples it takes. 0 samples is no slide.
    double glide_from_cents_ = 0.0;
    std::uint64_t glide_start_ = 0;
    int glide_samples_ = 0;

    // The live moves, in straight lines: each oscillator's level and the
    // loudness, from 0 to 1, each oscillator's width, and the filter's.
    std::array<Line, oscillator_count> oscillator_levels_{};
    std::array<Line, oscillator_count> oscillator_widths_{};
    Line gain_;
    Line cutoff_move_;
    Line resonance_move_;
    Line amount_move_;
    Line type_move_;

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

    // When a voice that is still sounding starts a new note, its new kinds
    // can be at a different height from where the old ones were heading. The gap
    // between them is added on top of the new wave and shrinks away, so the
    // sound glides across instead of jumping. Each side has its own.
    Stereo gap_;                // the gap when it started shrinking
    int gap_samples_left_ = 0;  // samples until it is gone
};
