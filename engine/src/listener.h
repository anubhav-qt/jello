#pragma once

#include "effect.h"
#include "filter.h"
#include "stereo.h"

// True if `number` is a "listens to": listen_to_own or a chain's number.
bool is_valid_listens_to(float number);

// What an effect that reacts to loudness listens to (the sidechain,
// docs/design.md#mix-in-the-engine): its own input, or another chain's
// sound in the same piece, so it never lags. Moving to another crossfades
// over effect_crossfade_ms. Only the audio thread uses it.
class Listener {
public:
    explicit Listener(int sample_rate);

    // Listens to `listens_to` from now on: listen_to_own, or a chain's
    // number, which the caller makes sure of.
    void set(int listens_to);

    // Starts hearing the selected source without a crossfade.
    void settle();

    // Starts a piece: `own` is the effect's input, before the effect
    // changes it.
    void start(const float* own, const EffectContext& context);

    // The sound listened to on frame `frame` of the piece. Read it before
    // the effect writes over that frame of its own input.
    Stereo next(int frame);

private:
    // Frame `frame` of `sound`.
    static Stereo frame_of(const float* sound, int frame);

    // `listens_to`'s sound in this piece.
    const float* sound_of(int listens_to) const;

    int listens_to_ = listen_to_own;
    int leaving_ = listen_to_own;

    // How much of `listens_to_` is heard, from 0 to 1, while it crossfades
    // in from `leaving_`.
    Glide share_;

    const float* own_ = nullptr;
    const float* const* chains_ = nullptr;
};
