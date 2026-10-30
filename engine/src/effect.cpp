#include "effect.h"

#include <algorithm>
#include <cmath>

#include "chorus.h"
#include "compressor.h"
#include "delay.h"
#include "eq.h"
#include "filter_effect.h"
#include "gate.h"
#include "reverb.h"
#include "saturation.h"
#include "volume_shaper.h"

// Every kind's values fit in effect_most_values.
static_assert(eq_output < effect_most_values, "the EQ's values fit");
static_assert(filter_effect_value_count <= effect_most_values, "the filter's values fit");
static_assert(saturation_value_count <= effect_most_values, "saturation's values fit");
static_assert(compressor_value_count <= effect_most_values, "the compressor's values fit");
static_assert(gate_value_count <= effect_most_values, "the gate's values fit");
static_assert(shaper_value_count <= effect_most_values, "the volume shaper's values fit");
static_assert(chorus_value_count <= effect_most_values, "the chorus's values fit");
static_assert(delay_value_count <= effect_most_values, "the delay's values fit");
static_assert(reverb_value_count <= effect_most_values, "the reverb's values fit");

Effect::Effect(EffectKind kind) : kind_(kind) {}

EffectKind Effect::kind() const {
    return kind_;
}

Effect* make_effect(EffectKind kind, int sample_rate, bool on_bus) {
    if (kind == EffectKind::eq) {
        return new Eq(sample_rate);
    }
    if (kind == EffectKind::filter) {
        return new FilterEffect(sample_rate);
    }
    if (kind == EffectKind::saturation) {
        return new Saturation(sample_rate);
    }
    if (kind == EffectKind::compressor) {
        return new Compressor(sample_rate);
    }
    if (kind == EffectKind::gate) {
        return new Gate(sample_rate);
    }
    if (kind == EffectKind::volume_shaper) {
        return new VolumeShaper(sample_rate);
    }
    if (kind == EffectKind::chorus) {
        return new Chorus(sample_rate, on_bus);
    }
    if (kind == EffectKind::delay) {
        return new Delay(sample_rate, on_bus);
    }
    if (kind == EffectKind::reverb) {
        return new Reverb(sample_rate, on_bus);
    }
    return nullptr;
}

double start_mix(bool on_bus) {
    if (on_bus) {
        return effect_start_mix_on_bus;
    }
    return effect_start_mix;
}

ValueRange effect_value_range(EffectKind kind, int value) {
    if (kind == EffectKind::eq) {
        return eq_value_range(value);
    }
    if (kind == EffectKind::filter) {
        return filter_effect_value_range(value);
    }
    if (kind == EffectKind::saturation) {
        return saturation_value_range(value);
    }
    if (kind == EffectKind::compressor) {
        return compressor_value_range(value);
    }
    if (kind == EffectKind::gate) {
        return gate_value_range(value);
    }
    if (kind == EffectKind::volume_shaper) {
        return volume_shaper_value_range(value);
    }
    if (kind == EffectKind::chorus) {
        return chorus_value_range(value);
    }
    if (kind == EffectKind::delay) {
        return delay_value_range(value);
    }
    if (kind == EffectKind::reverb) {
        return reverb_value_range(value);
    }
    return ValueRange{};
}

float moved_value(EffectKind kind, int value, float set, double move) {
    ValueRange range = effect_value_range(kind, value);
    if (!is_in_range(range, set)) {
        return set;
    }
    double moved = set + move;
    if (range.whole) {
        moved = std::round(moved);
    }
    moved = std::clamp(moved, static_cast<double>(range.lowest), static_cast<double>(range.highest));
    return static_cast<float>(moved);
}

bool is_valid_effect_value(EffectKind kind, int value, float number) {
    if (kind == EffectKind::eq) {
        return is_valid_eq_value(value, number);
    }
    if (kind == EffectKind::filter) {
        return is_valid_filter_effect_value(value, number);
    }
    if (kind == EffectKind::saturation) {
        return is_valid_saturation_value(value, number);
    }
    if (kind == EffectKind::compressor) {
        return is_valid_compressor_value(value, number);
    }
    if (kind == EffectKind::gate) {
        return is_valid_gate_value(value, number);
    }
    if (kind == EffectKind::volume_shaper) {
        return is_valid_volume_shaper_value(value, number);
    }
    if (kind == EffectKind::chorus) {
        return is_valid_chorus_value(value, number);
    }
    if (kind == EffectKind::delay) {
        return is_valid_delay_value(value, number);
    }
    if (kind == EffectKind::reverb) {
        return is_valid_reverb_value(value, number);
    }
    return false;
}

bool is_listens_to_value(EffectKind kind, int value) {
    if (kind == EffectKind::filter) {
        return value == filter_effect_listens_to;
    }
    if (kind == EffectKind::compressor) {
        return value == compressor_listens_to;
    }
    if (kind == EffectKind::gate) {
        return value == gate_listens_to;
    }
    return false;
}

ValueRange number_range(float lowest, float highest) {
    return ValueRange{true, lowest, highest, false};
}

ValueRange whole_range(int lowest, int highest) {
    return ValueRange{true, static_cast<float>(lowest), static_cast<float>(highest), true};
}

bool is_in_range(const ValueRange& range, float number) {
    if (!range.exists) {
        return false;
    }
    if (range.whole) {
        return is_whole_within(number, static_cast<int>(range.lowest), static_cast<int>(range.highest));
    }
    return is_number_within(number, range.lowest, range.highest);
}

float switch_number(bool on) {
    if (on) {
        return 1.0f;
    }
    return 0.0f;
}

bool is_number_within(float number, float lowest, float highest) {
    if (std::isnan(number)) {
        return false;
    }
    if (number < lowest) {
        return false;
    }
    return number <= highest;
}

bool is_whole_within(float number, int lowest, int highest) {
    if (!is_number_within(number, static_cast<float>(lowest), static_cast<float>(highest))) {
        return false;
    }
    return number == std::floor(number);
}

double gain_of_db(double db) {
    return std::pow(10.0, db / 20.0);
}
