#pragma once

// The kinds of effect a chain's slot can hold (docs/design.md#mix-in-the-engine).
// More come in later slices.
enum class EffectKind {
    none,  // an empty slot
    eq,
    filter,
    saturation,
    compressor,
    gate,
    volume_shaper,
    chorus,
    delay,
    reverb,
};

// How many kinds there are, none included.
constexpr int effect_kind_count = 10;

// How long an effect's numbers take to glide to a new value, and how long a
// choice or a switch takes to crossfade, in ms.
constexpr int effect_glide_ms = 20;
constexpr int effect_crossfade_ms = 10;

// How much of an echoing effect (chorus, delay, reverb) is heard when it is
// new: all of it on a bus, which the dry sound skips, and some elsewhere.
constexpr double effect_start_mix_on_bus = 1.0;
constexpr double effect_start_mix = 0.3;

// "Listens to", a value of the effects that react to loudness (the
// sidechain): listen_to_own for the effect's own input, or a chain's
// number, from 0 to most_chains - 1, for that chain's sound after its
// strip. Which chains each chain may listen to is checked before the value
// is sent (engine.cpp).
constexpr int listen_to_own = -1;
constexpr int most_chains = 14;  // Mixer::chain_count, checked there

// How many values an effect can have, numbered from 0: every kind's fit
// (checked in effect.cpp).
constexpr int effect_most_values = 64;

// The most a connection can move an effect's value either way, in the
// value's own units: across the widest range, 20 Hz to 20 kHz.
constexpr float effect_most_move = 20000.0f;

// The numbers a value takes: from `lowest` to `highest`, only whole ones if
// `whole`. A value whose numbers are not one stretch, like "listens to" or
// a cut's slope, has none (`exists` false), and connections never move it.
struct ValueRange {
    bool exists = false;
    float lowest = 0.0f;
    float highest = 0.0f;
    bool whole = false;
};

// A range of numbers, and one of whole numbers.
ValueRange number_range(float lowest, float highest);
ValueRange whole_range(int lowest, int highest);

// True if `number` is within `range`. NaN is not, and nothing is within no
// range.
bool is_in_range(const ValueRange& range, float number);

// A switch as a value's number: 1 for on, 0 for off.
float switch_number(bool on);

// What an effect may hear and know besides its own sound, for one piece of
// sound (Effect::process).
struct EffectContext {
    // Each chain's sound in this piece, after its chain and strip, by chain
    // number: most_chains of them. A chain that has not run yet in this
    // piece is silent here.
    const float* const* chains = nullptr;

    // Where the piece starts, in beats since the beat started, and how many
    // beats pass each sample.
    double beat = 0.0;
    double beats_per_sample = 0.0;
};

// One effect in a chain's slot. Each kind has its own numbered values, and
// starts where it changes the sound little. Effects are made on the thread
// that sends them (make_effect), then only the audio thread uses them until
// they come back to be freed, so nothing in process or set_value locks,
// allocates or waits.
class Effect {
public:
    explicit Effect(EffectKind kind);
    virtual ~Effect() = default;

    EffectKind kind() const;

    // Puts `frames` frames of stereo (left, right, left, ...) through the
    // effect, in place.
    virtual void process(float* stereo, int frames, const EffectContext& context) = 0;

    // Sets value `value` to `number`. Ignored unless is_valid_effect_value
    // says both suit this kind, since a timed value may reach a slot whose
    // effect has changed since it was sent.
    virtual void set_value(int value, float number) = 0;

    // What value `value` is set to, for the values with a range
    // (effect_value_range); 0 for the others.
    virtual float value(int value) const = 0;

    // Jumps every glide and crossfade straight to the values set, with
    // nothing left to glide. A new effect does it once, just before its
    // first sound, so it starts at the values sent with it (Chain::swap).
    virtual void settle() = 0;

    // The most the effect turned the sound down since the last time it was
    // asked, in dB, 0 for not at all. Effects that never do say 0.
    virtual float take_reduction_db() {
        return 0.0f;
    }

private:
    EffectKind kind_;
};

// A new effect of `kind`, at its starting values, or nullptr for none.
// `on_bus` is true for a bus's chain, where echoing effects start fully
// wet. It allocates, so never on the audio thread.
Effect* make_effect(EffectKind kind, int sample_rate, bool on_bus);

// The mix an echoing effect starts at (effect_start_mix_on_bus).
double start_mix(bool on_bus);

// True if `kind` has a value numbered `value` and `number` is within its
// range.
bool is_valid_effect_value(EffectKind kind, int value, float number);

// The range of value `value` of `kind`.
ValueRange effect_value_range(EffectKind kind, int value);

// What value `value` of `kind` becomes when it is set to `set` and a
// connection moves it by `move`: kept within its range, and rounded for a
// whole one. A value with no range, or set outside it (the compressor's
// limit), stays at `set`.
float moved_value(EffectKind kind, int value, float set, double move);

// True if value `value` of `kind` is its "listens to".
bool is_listens_to_value(EffectKind kind, int value);

// True if `number` is a whole number from `lowest` to `highest`, for values
// that are choices. NaN is not.
bool is_whole_within(float number, int lowest, int highest);

// True if `number` is from `lowest` to `highest`. NaN is not.
bool is_number_within(float number, float lowest, float highest);

// A level in dB as a gain: 0 dB is 1, -6 dB about a half.
double gain_of_db(double db);
