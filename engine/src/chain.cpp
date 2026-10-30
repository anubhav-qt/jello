#include "chain.h"

#include <algorithm>

Chain::Chain(int sample_rate)
    : on_step_(1000.0f / (effect_crossfade_ms * static_cast<float>(sample_rate))),
      dry_(2 * most_frames, 0.0f) {
    for (Slot& slot : slots_) {
        slot.on = switched_on();
    }
}

Chain::~Chain() {
    for (Slot& slot : slots_) {
        delete slot.effect;
    }
}

Ease Chain::switched_on() const {
    return Ease(1.0f, on_step_);
}

Effect* Chain::swap(int slot, Effect* effect) {
    Slot& place = slots_[slot];
    Effect* replaced = place.effect;
    place.effect = effect;
    place.fresh = true;
    place.on = switched_on();

    // A new effect starts unmoved, at its starting values.
    place.set.fill(0.0f);
    place.moved.fill(0.0);
    if (effect != nullptr) {
        for (int value = 0; value < effect_most_values; value++) {
            place.set[value] = effect->value(value);
        }
    }
    return replaced;
}

void Chain::move(int from, int to) {
    // Moving down: the slots after `from`, up to `to`, each move up one.
    // Moving up: the slots from `to` up to `from` each move down one.
    auto first = slots_.begin();
    if (from < to) {
        std::rotate(first + from, first + from + 1, first + to + 1);
    } else if (from > to) {
        std::rotate(first + to, first + from, first + from + 1);
    }
}

void Chain::set_on(int slot, bool on) {
    if (on) {
        slots_[slot].on.set(1.0f);
    } else {
        slots_[slot].on.set(0.0f);
    }
}

void Chain::set_value(int slot, int value, float number) {
    Slot& place = slots_[slot];
    if (place.effect == nullptr) {
        return;
    }
    if (!is_valid_effect_value(place.effect->kind(), value, number)) {
        return;
    }
    place.set[value] = number;
    apply(place, value);
}

void Chain::move_values(const Routing& routing) {
    // Nothing moved now or before.
    int count = routing.effect_move_count();
    if (count == 0) {
        if (!moving_) {
            return;
        }
    }

    for (std::array<double, effect_most_values>& slot_moves : next_moves_) {
        slot_moves.fill(0.0);
    }
    for (int place = 0; place < count; place++) {
        const EffectMove& move = routing.effect_move(place);
        next_moves_[move.slot][move.value] += move.move;
    }
    moving_ = count > 0;

    // Only the values whose move changed are set again.
    for (int s = 0; s < slot_count; s++) {
        Slot& slot = slots_[s];
        if (slot.effect == nullptr) {
            continue;
        }
        EffectKind kind = slot.effect->kind();
        for (int value = 0; value < effect_most_values; value++) {
            double move = next_moves_[s][value];
            if (move == slot.moved[value]) {
                continue;
            }
            if (!effect_value_range(kind, value).exists) {
                continue;
            }
            slot.moved[value] = move;
            apply(slot, value);
        }
    }
}

void Chain::apply(Slot& slot, int value) {
    EffectKind kind = slot.effect->kind();
    float heard = moved_value(kind, value, slot.set[value], slot.moved[value]);
    slot.effect->set_value(value, heard);
}

float Chain::take_reduction_db(int slot) {
    Effect* effect = slots_[slot].effect;
    if (effect == nullptr) {
        return 0.0f;
    }
    return effect->take_reduction_db();
}

void Chain::process(float* stereo, int frames, const EffectContext& context) {
    for (Slot& slot : slots_) {
        if (slot.effect == nullptr) {
            continue;
        }

        // A new effect starts right at its values, with nothing to glide.
        // Changes after this glide as usual.
        bool heard = slot.on.heard() > 0.0f;
        if (heard) {
            if (slot.fresh) {
                slot.effect->settle();
                slot.fresh = false;
            }
        }

        // Fully on or fully off, nothing to crossfade.
        if (!slot.on.is_moving()) {
            if (slot.on.heard() == 1.0f) {
                slot.effect->process(stereo, frames, context);
            }
            continue;
        }

        // Crossfading: the sound before the effect, faded out as the sound
        // after it fades in, or the other way round.
        std::copy(stereo, stereo + 2 * frames, dry_.begin());
        slot.effect->process(stereo, frames, context);
        for (int i = 0; i < frames; i++) {
            float on = slot.on.next();
            for (int speaker = 0; speaker < 2; speaker++) {
                float dry = dry_[2 * i + speaker];
                float wet = stereo[2 * i + speaker];
                stereo[2 * i + speaker] = dry + (wet - dry) * on;
            }
        }
    }
}
