#pragma once

#include <array>
#include <vector>

#include "ease.h"
#include "effect.h"
#include "routing.h"

// A chain of effect slots (docs/design.md#mix-in-the-engine): every track,
// bus and the master has one. The sound goes through the slots in order;
// an empty slot passes it on untouched, so an empty chain changes nothing.
// The chain owns its effects: it frees the ones still in it when it goes,
// and hands back the ones it swaps out. Only the audio thread uses it.
class Chain {
public:
    static constexpr int slot_count = 8;

    // The most frames process takes at a time.
    static constexpr int most_frames = 256;

    explicit Chain(int sample_rate);
    ~Chain();

    Chain(const Chain&) = delete;
    Chain& operator=(const Chain&) = delete;

    // Puts `effect`, or nullptr for none, in slot `slot`, switched on, and
    // returns the effect it replaces, or nullptr, for the caller to free
    // off the audio thread. The caller makes sure `slot` is below
    // slot_count, here and below. Values set before the effect is first
    // heard start right there, without gliding.
    Effect* swap(int slot, Effect* effect);

    // Moves slot `from`'s effect, with everything it remembers and its on
    // switch, to slot `to`, shifting the slots between by one.
    void move(int from, int to);

    // Switches slot `slot`'s effect on or off. Off passes the sound
    // untouched. Either way it crossfades over effect_crossfade_ms.
    void set_on(int slot, bool on);

    // Sets one of slot `slot`'s effect's values (Effect::set_value), heard
    // with the connections' move on top. An empty slot ignores it.
    void set_value(int slot, int value, float number);

    // Moves the effects' values by `routing`'s moves of effects
    // (Routing::effect_move), on top of what each is set to; a value no
    // longer moved goes back to it. Called every 32 samples, where they
    // start, so the moves glide like any new value.
    void move_values(const Routing& routing);

    // Puts `frames` frames of stereo, at most most_frames, through every
    // slot, in place. `context` is what the effects may hear and know
    // besides (EffectContext).
    void process(float* stereo, int frames, const EffectContext& context);

    // The most slot `slot`'s effect turned the sound down since the last
    // time it was asked, in dB (Effect::take_reduction_db); 0 for an empty
    // slot.
    float take_reduction_db(int slot);

private:
    // How far the connections move each value of each slot's effect.
    using Moves = std::array<std::array<double, effect_most_values>, slot_count>;

    struct Slot {
        Effect* effect = nullptr;
        // True until the effect is first heard, when it settles at the
        // values set by then (Effect::settle).
        bool fresh = false;
        // How much of the effect is heard, from 0 (off) to 1 (on). The
        // chain sets its step when it is made.
        Ease on{1.0f, 0.0f};
        // What each of the effect's values is set to, and how far the
        // connections move it now.
        std::array<float, effect_most_values> set{};
        std::array<double, effect_most_values> moved{};
    };

    // The on switch of an effect just put in a slot: on, with nothing to
    // crossfade.
    Ease switched_on() const;

    // Sets value `value` of `slot`'s effect to what it is set to, moved.
    static void apply(Slot& slot, int value);

    float on_step_;
    std::array<Slot, slot_count> slots_;

    // The sound before a slot, while its on switch crossfades.
    std::vector<float> dry_;

    // True while any value is moved, and the next moves, worked out here
    // so the audio thread never makes room for them.
    bool moving_ = false;
    Moves next_moves_{};
};
