// The C API functions (include/jello_engine.h) and the audio device.
// All talking to the speakers happens in this file, so adding another audio
// system later (like ASIO on Windows) stays in one place.

#include "jello_engine.h"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <memory>
#include <new>
#include <vector>

#include "miniaudio_settings.h"
#include "capture.h"
#include "conductor.h"
#include "compressor.h"
#include "eq.h"
#include "chorus.h"
#include "delay.h"
#include "filter_effect.h"
#include "reverb.h"
#include "gate.h"
#include "saturation.h"
#include "volume_shaper.h"

namespace {

constexpr int sample_rate = 48000;
constexpr int channels = 2;  // stereo

// MIDI numbers notes from 0 to 127. Middle C is 60.
bool is_valid_note(int note) {
    return note >= 0 && note <= 127;
}

// A note a key can play: a note, or a drum pad's.
bool is_valid_played_note(int note) {
    if (is_valid_note(note)) {
        return true;
    }
    return is_drum_note(note);
}

// How hard a key was hit, from 1 to 127. In MIDI a velocity of 0 means
// "let go", so a note never starts at 0.
bool is_valid_velocity(int velocity) {
    return velocity >= 1 && velocity <= 127;
}

bool is_valid_tempo(int tempo) {
    return tempo >= JELLO_SLOWEST_TEMPO && tempo <= JELLO_FASTEST_TEMPO;
}

// The C API and the looper agree on how many notes a loop holds.
static_assert(JELLO_LOOP_MAX_NOTES == Looper::max_notes, "the C API and the looper hold the same number of notes");

// The C API and the looper agree on how many layers a loop holds, and on the
// number for the newest.
static_assert(JELLO_LOOP_MAX_LAYERS == Looper::max_layers, "the C API and the looper hold the same number of layers");
static_assert(JELLO_NEWEST_LAYER == Looper::newest_layer, "the C API and the looper name the newest layer alike");

// The C API and the tracks agree on how many there are and how many layers
// each holds.
static_assert(JELLO_TRACK_COUNT == Tracks::count, "the C API and the conductor have the same tracks");
static_assert(JELLO_TRACK_MAX_LAYERS == Looper::max_track_layers, "the C API and the looper hold as many layers a track");

// A track's number: from 0, below JELLO_TRACK_COUNT.
bool is_valid_track(int track) {
    if (track < 0) {
        return false;
    }
    return track < JELLO_TRACK_COUNT;
}

// A layer slot's number on a track: from 0, below JELLO_TRACK_MAX_LAYERS.
bool is_valid_slot(int slot) {
    if (slot < 0) {
        return false;
    }
    return slot < JELLO_TRACK_MAX_LAYERS;
}

// A layer's number for the C API: from 0, below JELLO_LOOP_MAX_LAYERS.
bool is_valid_layer(int layer) {
    if (layer < 0) {
        return false;
    }
    return layer < JELLO_LOOP_MAX_LAYERS;
}

// A loop note's pitch: a note, a press of the vibrato or glide key, or a
// drum pad's hit.
bool is_valid_loop_pitch(int note) {
    if (note < 0) {
        return false;
    }
    return note < loop_pitch_count;
}

// A layer's volume, from 0 to 1.
bool is_valid_volume(float volume) {
    if (std::isnan(volume)) {
        return false;
    }
    if (volume < 0.0f) {
        return false;
    }
    return volume <= 1.0f;
}

// The C API and the filter agree on its ranges.
static_assert(JELLO_FILTER_LOWEST_HZ == static_cast<int>(filter_lowest_hz),
              "the C API and the filter have the same lowest cutoff");
static_assert(JELLO_FILTER_OPEN_HZ == static_cast<int>(filter_open_hz),
              "the C API and the filter open at the same cutoff");
static_assert(JELLO_FILTER_MOST_OCTAVES == static_cast<int>(filter_most_octaves),
              "the C API and the filter open the cutoff as far");
static_assert(JELLO_FILTER_LONGEST_MS == filter_longest_ms, "the C API and the filter have the same longest stage");
static_assert(JELLO_FILTER_NOTCH == static_cast<int>(FilterType::notch), "the notch is the same type");
static_assert(JELLO_FILTER_NOTCH + 1 == filter_type_count, "every filter type has a number");

// The C API and unison agree on its ranges.
static_assert(JELLO_UNISON_MOST_COPIES == unison_most_copies, "the C API and unison have the same most copies");
static_assert(JELLO_UNISON_MOST_DETUNE_CENTS == static_cast<int>(unison_most_detune_cents),
              "the C API and unison have the same most detune");

// The C API and the loudness envelope agree on how short and long a stage
// can be.
static_assert(JELLO_LOUDNESS_SHORTEST_MS == loudness_shortest_ms,
              "the C API and the loudness envelope have the same shortest stage");
static_assert(JELLO_LOUDNESS_LONGEST_MS == loudness_longest_ms,
              "the C API and the loudness envelope have the same longest stage");

// The C API and the oscillators agree on how many there are and how far
// they can be tuned.
static_assert(JELLO_OSCILLATOR_COUNT == oscillator_count, "the C API and the voices have the same oscillators");
static_assert(JELLO_OSCILLATOR_MOST_OCTAVES == oscillator_most_octaves,
              "the C API and the oscillators tune as many octaves");
static_assert(JELLO_OSCILLATOR_MOST_HALF_STEPS == oscillator_most_half_steps,
              "the C API and the oscillators tune as many half steps");
static_assert(JELLO_OSCILLATOR_MOST_CENTS == static_cast<int>(oscillator_most_cents),
              "the C API and the oscillators tune as many cents");
static_assert(JELLO_KIND_NOISE == static_cast<int>(Kind::noise), "noise is the same kind");
static_assert(JELLO_KIND_SAMPLE == static_cast<int>(Kind::sample), "the sample is the same kind");
static_assert(JELLO_KIND_STRING + 1 == kind_count, "every kind has a number");

// The C API and the sampler agree on how many sets there are, and on the
// drum pads.
static_assert(JELLO_SAMPLE_SET_MOST == sample_set_most, "the C API and the engine keep as many sets");
static_assert(JELLO_DRUM_FIRST_NOTE == drum_first_note, "the C API and the drum kit start the pads alike");
static_assert(JELLO_DRUM_PAD_COUNT == drum_pad_count, "the C API and the drum kit have the same pads");
static_assert(JELLO_DRUM_MOST_HALF_STEPS == drum_most_half_steps, "the C API and the drum kit tune pads as far");
static_assert(JELLO_DRUM_MOST_GROUP == drum_most_group, "the C API and the drum kit have the same choke groups");
static_assert(JELLO_PULSE_NARROWEST_PERCENT == static_cast<int>(oscillator_narrowest_width * 100.0f + 0.5f),
              "the C API and the oscillators have the same narrowest pulse");
static_assert(JELLO_PULSE_WIDEST_PERCENT == static_cast<int>(oscillator_widest_width * 100.0f + 0.5f),
              "the C API and the oscillators have the same widest pulse");

// The C API and the voices agree on how far notes can be tuned.
static_assert(JELLO_TUNING_LOWEST_HZ == static_cast<int>(tuning_lowest_hz), "the C API and the voices tune as low");
static_assert(JELLO_TUNING_HIGHEST_HZ == static_cast<int>(tuning_highest_hz), "the C API and the voices tune as high");

// The C API and the synth agree on how long a glide can take, and on the
// pitch the looper records the glide key on.
static_assert(JELLO_GLIDE_LONGEST_MS == glide_longest_ms, "the C API and the voices glide as long");
static_assert(JELLO_PEDAL_KEY_NOTE == Synth::pedal_key_note, "the C API and the synth record the pedal alike");
static_assert(JELLO_TRIGGER_RELEASE == static_cast<int>(SampleTrigger::release), "the triggers have the same numbers");
static_assert(JELLO_TRIGGER_PEDAL_UP == static_cast<int>(SampleTrigger::pedal_up), "the triggers have the same numbers");
static_assert(JELLO_GLIDE_KEY_NOTE == Synth::glide_key_note, "the C API and the synth record the glide key alike");

// True if `value` is a number from `lowest` to `highest`. NaN, not a number
// at all, is not.
bool is_within(float value, float lowest, float highest) {
    if (std::isnan(value)) {
        return false;
    }
    if (value < lowest) {
        return false;
    }
    return value <= highest;
}

// The same for doubles.
bool is_within_double(double value, double lowest, double highest) {
    if (std::isnan(value)) {
        return false;
    }
    if (value < lowest) {
        return false;
    }
    return value <= highest;
}

// True if `value` is a whole number from `lowest` to `highest`.
bool is_within_int(int value, int lowest, int highest) {
    if (value < lowest) {
        return false;
    }
    return value <= highest;
}

// A pan: from -1 (fully left) to 1 (fully right).
bool is_valid_pan(float pan) {
    return is_within(pan, -1.0f, 1.0f);
}

// The C API and the mixer agree on how many buses there are, and on the
// limiter's limits.
static_assert(JELLO_BUS_COUNT == Mixer::bus_count, "the C API and the mixer have the same buses");
static_assert(JELLO_LIMITER_MOST_BOOST_DB == static_cast<int>(limiter_most_boost_db), "the same most boost");
static_assert(JELLO_LIMITER_LOWEST_CEILING_DB == static_cast<int>(limiter_lowest_ceiling_db), "the same lowest ceiling");
static_assert(JELLO_LIMITER_SHORTEST_RELEASE_MS == limiter_shortest_release_ms, "the same shortest release");
static_assert(JELLO_LIMITER_LONGEST_RELEASE_MS == limiter_longest_release_ms, "the same longest release");

// The C API and the engine agree on the chains and every effect's values.
static_assert(JELLO_EFFECT_SLOTS == Chain::slot_count, "the same slots");
static_assert(JELLO_FIRST_BUS_CHAIN == Mixer::first_bus_chain, "the same first bus chain");
static_assert(JELLO_MASTER_CHAIN == Mixer::master_chain, "the same master chain");
static_assert(JELLO_CHAIN_COUNT == Mixer::chain_count, "the same chains");
static_assert(JELLO_EFFECT_VOLUME_SHAPER == static_cast<int>(EffectKind::volume_shaper), "the same kinds");
static_assert(JELLO_EFFECT_REVERB == static_cast<int>(EffectKind::reverb), "the same kinds");
static_assert(JELLO_EFFECT_REVERB + 1 == effect_kind_count, "every kind has a number");
static_assert(JELLO_EFFECT_START_MIX_PERCENT == static_cast<int>(effect_start_mix * 100.0 + 0.5), "the same start mix");
static_assert(JELLO_LISTEN_TO_OWN == listen_to_own, "the same own input");
static_assert(JELLO_EQ_BAND_COUNT == eq_band_count, "the same bands");
static_assert(JELLO_EQ_BAND_SLOPE == eq_band_slope, "the same band values");
static_assert(JELLO_EQ_BAND_VALUES == eq_band_values, "the same band values");
static_assert(JELLO_EQ_OUTPUT == eq_output, "the same output");
static_assert(JELLO_EQ_NOTCH == static_cast<int>(EqShape::notch), "the same shapes");
static_assert(JELLO_EQ_NOTCH + 1 == eq_shape_count, "every shape has a number");
static_assert(JELLO_EQ_LOWEST_HZ == static_cast<int>(eq_lowest_hz), "the same lowest frequency");
static_assert(JELLO_EQ_HIGHEST_HZ == static_cast<int>(eq_highest_hz), "the same highest frequency");
static_assert(JELLO_EQ_MOST_GAIN_DB == static_cast<int>(eq_most_gain_db), "the same most gain");
static_assert(JELLO_EQ_WIDEST_WIDTH == static_cast<int>(eq_widest_width), "the same widest width");
static_assert(JELLO_FILTER_EFFECT_LISTENS_TO == filter_effect_listens_to, "the same filter values");
static_assert(JELLO_FILTER_EFFECT_LISTENS_TO + 1 == filter_effect_value_count, "every filter value has a number");
static_assert(JELLO_FOLLOW_LONGEST_ATTACK_MS == static_cast<int>(follow_longest_attack_ms), "the same attack");
static_assert(JELLO_FOLLOW_SHORTEST_RELEASE_MS == static_cast<int>(follow_shortest_release_ms), "the same release");
static_assert(JELLO_FOLLOW_LONGEST_RELEASE_MS == static_cast<int>(follow_longest_release_ms), "the same release");
static_assert(JELLO_SATURATION_MIX == saturation_mix, "the same saturation values");
static_assert(JELLO_SATURATION_MIX + 1 == saturation_value_count, "every saturation value has a number");
static_assert(JELLO_SATURATION_FOLD == static_cast<int>(SaturationCurve::fold), "the same curves");
static_assert(JELLO_SATURATION_FOLD + 1 == saturation_curve_count, "every curve has a number");
static_assert(JELLO_SATURATION_MOST_DRIVE_DB == static_cast<int>(saturation_most_drive_db), "the same drive");
static_assert(JELLO_SATURATION_MOST_OUTPUT_DB == static_cast<int>(saturation_most_output_db), "the same output");
static_assert(JELLO_COMPRESSOR_LOW_CUT == compressor_low_cut, "the same compressor values");
static_assert(JELLO_COMPRESSOR_LOW_CUT + 1 == compressor_value_count, "every compressor value has a number");
static_assert(JELLO_COMPRESSOR_LISTENS_TO == compressor_listens_to, "the same compressor values");
static_assert(JELLO_COMPRESSOR_LOWEST_THRESHOLD_DB == static_cast<int>(compressor_lowest_threshold_db), "the same threshold");
static_assert(JELLO_COMPRESSOR_MOST_RATIO == static_cast<int>(compressor_most_ratio), "the same ratio");
static_assert(JELLO_COMPRESSOR_LIMIT == static_cast<int>(compressor_limit), "the same limit");
static_assert(JELLO_COMPRESSOR_LONGEST_ATTACK_MS == static_cast<int>(compressor_longest_attack_ms), "the same attack");
static_assert(JELLO_COMPRESSOR_SHORTEST_RELEASE_MS == static_cast<int>(compressor_shortest_release_ms), "the same release");
static_assert(JELLO_COMPRESSOR_LONGEST_RELEASE_MS == static_cast<int>(compressor_longest_release_ms), "the same release");
static_assert(JELLO_COMPRESSOR_WIDEST_KNEE_DB == static_cast<int>(compressor_widest_knee_db), "the same knee");
static_assert(JELLO_COMPRESSOR_MOST_MAKEUP_DB == static_cast<int>(compressor_most_makeup_db), "the same makeup");
static_assert(JELLO_COMPRESSOR_LOWEST_CUT_HZ == static_cast<int>(compressor_lowest_cut_hz), "the same low cut");
static_assert(JELLO_COMPRESSOR_HIGHEST_CUT_HZ == static_cast<int>(compressor_highest_cut_hz), "the same low cut");
static_assert(JELLO_GATE_LISTENS_TO == gate_listens_to, "the same gate values");
static_assert(JELLO_GATE_LISTENS_TO + 1 == gate_value_count, "every gate value has a number");
static_assert(JELLO_GATE_LOWEST_THRESHOLD_DB == static_cast<int>(gate_lowest_threshold_db), "the same threshold");
static_assert(JELLO_GATE_SILENT_RANGE_DB == static_cast<int>(gate_silent_range_db), "the same range");
static_assert(JELLO_GATE_SILENT_REDUCTION_DB == -JELLO_GATE_SILENT_RANGE_DB, "silent reads as the range");
static_assert(JELLO_GATE_LONGEST_ATTACK_MS == static_cast<int>(gate_longest_attack_ms), "the same attack");
static_assert(JELLO_GATE_LONGEST_HOLD_MS == static_cast<int>(gate_longest_hold_ms), "the same hold");
static_assert(JELLO_GATE_SHORTEST_RELEASE_MS == static_cast<int>(gate_shortest_release_ms), "the same release");
static_assert(JELLO_GATE_LONGEST_RELEASE_MS == static_cast<int>(gate_longest_release_ms), "the same release");
static_assert(JELLO_SHAPER_MOST_POINTS == shaper_most_points, "the same points");
static_assert(JELLO_SHAPER_POINT_BEND == shaper_point_bend, "the same point values");
static_assert(JELLO_SHAPER_POINT_VALUES == shaper_point_values, "the same point values");
static_assert(JELLO_SHAPER_CYCLE == shaper_cycle, "the same shaper values");
static_assert(JELLO_SHAPER_DEPTH == shaper_depth, "the same shaper values");
static_assert(JELLO_SHAPER_POINTS + 1 == shaper_value_count, "every shaper value has a number");
static_assert(JELLO_SHAPER_LONGEST_CYCLE == static_cast<int>(shaper_longest_cycle), "the same cycle");
static_assert(JELLO_CHORUS_MIX == chorus_mix, "the same chorus values");
static_assert(JELLO_CHORUS_MIX + 1 == chorus_value_count, "every chorus value has a number");
static_assert(JELLO_CHORUS_MOST_COPIES == chorus_most_copies, "the same copies");
static_assert(JELLO_CHORUS_FASTEST_HZ == static_cast<int>(chorus_fastest_hz), "the same speed");
static_assert(JELLO_CHORUS_LONGEST_CYCLE == static_cast<int>(chorus_longest_cycle), "the same cycle");
static_assert(JELLO_CHORUS_SHORTEST_DELAY_MS == static_cast<int>(chorus_shortest_delay_ms), "the same delay");
static_assert(JELLO_CHORUS_LONGEST_DELAY_MS == static_cast<int>(chorus_longest_delay_ms), "the same delay");
static_assert(JELLO_CHORUS_MOST_FEEDBACK_PERCENT == static_cast<int>(chorus_most_feedback * 100.0f + 0.5f),
              "the same feedback");
static_assert(JELLO_DELAY_MIX == delay_mix, "the same delay values");
static_assert(JELLO_DELAY_MIX + 1 == delay_value_count, "every delay value has a number");
static_assert(JELLO_DELAY_SHORTEST_MS == static_cast<int>(delay_shortest_ms), "the same time");
static_assert(JELLO_DELAY_LONGEST_MS == static_cast<int>(delay_longest_ms), "the same time");
static_assert(JELLO_DELAY_LONGEST_BEATS == static_cast<int>(delay_longest_beats), "the same beats");
static_assert(JELLO_DELAY_LOWEST_HZ == static_cast<int>(delay_lowest_hz), "the same cuts");
static_assert(JELLO_DELAY_HIGHEST_HZ == static_cast<int>(delay_highest_hz), "the same cuts");
static_assert(JELLO_DELAY_FASTEST_WOBBLE_HZ == static_cast<int>(delay_fastest_wobble_hz), "the same wobble");
static_assert(JELLO_REVERB_MIX == reverb_mix, "the same reverb values");
static_assert(JELLO_REVERB_MIX + 1 == reverb_value_count, "every reverb value has a number");
static_assert(JELLO_REVERB_LONGEST_DECAY == static_cast<int>(reverb_longest_decay), "the same decay");
static_assert(JELLO_REVERB_LONGEST_PRE_DELAY_MS == static_cast<int>(reverb_longest_pre_delay_ms), "the same pre-delay");
static_assert(JELLO_REVERB_LOWEST_DAMPING_HZ == static_cast<int>(reverb_lowest_damping_hz), "the same damping");
static_assert(JELLO_REVERB_HIGHEST_DAMPING_HZ == static_cast<int>(reverb_highest_damping_hz), "the same damping");
static_assert(JELLO_REVERB_LOWEST_CUT_HZ == static_cast<int>(reverb_lowest_cut_hz), "the same low cut");
static_assert(JELLO_REVERB_HIGHEST_CUT_HZ == static_cast<int>(reverb_highest_cut_hz), "the same low cut");

// A chain's number, and an effect slot's.
bool is_valid_chain(int chain) {
    return is_within_int(chain, 0, JELLO_CHAIN_COUNT - 1);
}

bool is_valid_effect_slot(int slot) {
    return is_within_int(slot, 0, JELLO_EFFECT_SLOTS - 1);
}

// True if chain `chain` may listen to `source`, a "listens to": its own
// input, or another track. A bus or the master may listen to a bus too,
// but never to itself, and no one to the master.
bool can_listen_to(int chain, int source) {
    if (source == JELLO_LISTEN_TO_OWN) {
        return true;
    }
    if (source == chain) {
        return false;
    }
    if (source == JELLO_MASTER_CHAIN) {
        return false;
    }
    if (chain < JELLO_FIRST_BUS_CHAIN) {
        return source < JELLO_FIRST_BUS_CHAIN;
    }
    return true;
}

// A bus's number: from 0, below JELLO_BUS_COUNT.
bool is_valid_bus(int bus) {
    if (bus < 0) {
        return false;
    }
    return bus < JELLO_BUS_COUNT;
}

// How long a filter envelope's stage takes, in ms.
bool is_valid_stage_ms(int ms) {
    if (ms < 0) {
        return false;
    }
    return ms <= JELLO_FILTER_LONGEST_MS;
}

// How long a loudness envelope's stage takes, in ms.
bool is_valid_loudness_ms(int ms) {
    if (ms < JELLO_LOUDNESS_SHORTEST_MS) {
        return false;
    }
    return ms <= JELLO_LOUDNESS_LONGEST_MS;
}

// The C API and the LFOs agree on how many there are and on their ranges.
static_assert(JELLO_LFO_COUNT == lfo_count, "the C API and the routing list have the same LFOs");
static_assert(JELLO_LFO_MOST_STEPS == lfo_most_steps, "the C API and the LFOs have the same most steps");
static_assert(JELLO_LFO_MOST_PRESSES == lfo_most_presses, "the C API and the LFOs have the same most presses");
static_assert(JELLO_LFO_LONGEST_MS == lfo_longest_ms, "the C API and the LFOs have the same longest delay");
static_assert(JELLO_LFO_LONGEST_DELAY_BEATS == static_cast<int>(lfo_longest_delay_beats),
              "the C API and the LFOs have the same longest delay in beats");
static_assert(JELLO_CONNECTION_COUNT == connection_count, "the C API and the routing list have the same connections");
static_assert(JELLO_VIBRATO_KEY_LONGEST_MS == vibrato_key_longest_ms,
              "the C API and the routing list have the same longest vibrato key times");
static_assert(JELLO_VIBRATO_KEY_NOTE == Synth::vibrato_key_note, "the C API and the synth record the vibrato key alike");

// The C API's numbers line up with the routing list's sources and targets.
static_assert(JELLO_SOURCE_LFO_1 == static_cast<int>(RoutingSource::lfo_1), "LFO 1 is the same source");
static_assert(JELLO_SOURCE_VIBRATO_KEY == static_cast<int>(RoutingSource::vibrato_key), "the vibrato key is the same source");
static_assert(JELLO_SOURCE_FILTER_ENVELOPE == static_cast<int>(RoutingSource::filter_envelope),
              "the filter envelope is the same source");
static_assert(JELLO_SOURCE_FILTER_ENVELOPE + 1 == source_count, "every source has a number");
static_assert(JELLO_TARGET_FILTER_TYPE == static_cast<int>(RoutingTarget::filter_type),
              "the filter type is the same target");
static_assert(JELLO_TARGET_LFO_1_SPEED == static_cast<int>(RoutingTarget::lfo_1_speed), "LFO 1's speed is the same target");
static_assert(JELLO_TARGET_OSCILLATOR_1_LEVEL == static_cast<int>(RoutingTarget::oscillator_1_level),
              "oscillator 1's level is the same target");
static_assert(JELLO_TARGET_OSCILLATOR_1_PITCH == static_cast<int>(RoutingTarget::oscillator_1_pitch),
              "oscillator 1's pitch is the same target");
static_assert(JELLO_TARGET_OSCILLATOR_1_WIDTH == static_cast<int>(RoutingTarget::oscillator_1_width),
              "oscillator 1's width is the same target");
static_assert(JELLO_TARGET_OSCILLATOR_1_KIND == static_cast<int>(RoutingTarget::oscillator_1_kind),
              "oscillator 1's kind is the same target");
static_assert(JELLO_TARGET_COPIES == static_cast<int>(RoutingTarget::copies), "the copies are the same target");
static_assert(JELLO_TARGET_FILTER_RELEASE == static_cast<int>(RoutingTarget::filter_release),
              "the filter release is the same target");
static_assert(JELLO_TARGET_GLIDE_TIME == static_cast<int>(RoutingTarget::glide_time), "the glide time is the same target");
static_assert(JELLO_TARGET_EFFECT == static_cast<int>(RoutingTarget::effect), "an effect is the same target");
static_assert(JELLO_TARGET_EFFECT + 1 == target_count, "every target has a number");
static_assert(JELLO_EFFECT_MOST_VALUES == effect_most_values, "the C API and the effects have as many values");
static_assert(JELLO_EFFECT_MOST_MOVE == effect_most_move, "the C API and the routing list move effects as far");

// An LFO's settings from the C API, checked: false if any is out of range.
bool lfo_of_api(const JelloLfo& api, LfoSettings& lfo) {
    if (api.shape < JELLO_LFO_SINE) {
        return false;
    }
    if (api.shape > JELLO_LFO_STEPS) {
        return false;
    }
    if (api.step_count < 1) {
        return false;
    }
    if (api.step_count > JELLO_LFO_MOST_STEPS) {
        return false;
    }
    for (int i = 0; i < api.step_count; i++) {
        if (!is_within(api.steps[i], -1.0f, 1.0f)) {
            return false;
        }
    }
    if (api.speed < JELLO_LFO_HZ) {
        return false;
    }
    if (api.speed > JELLO_LFO_PRESSES) {
        return false;
    }
    if (!is_within_double(api.hz, lfo_slowest_hz, lfo_fastest_hz)) {
        return false;
    }
    if (!is_within_double(api.beats, lfo_shortest_beats, lfo_longest_beats)) {
        return false;
    }
    if (api.presses < 1) {
        return false;
    }
    if (api.presses > JELLO_LFO_MOST_PRESSES) {
        return false;
    }
    if (!is_within_double(api.start, 0.0, 1.0)) {
        return false;
    }
    if (!is_within_double(api.spread, 0.0, 1.0)) {
        return false;
    }
    if (!is_within_int(api.delay_ms, 0, JELLO_LFO_LONGEST_MS)) {
        return false;
    }
    if (!is_within_int(api.fade_ms, 0, JELLO_LFO_LONGEST_MS)) {
        return false;
    }
    if (!is_within_double(api.delay_beats, 0.0, lfo_longest_delay_beats)) {
        return false;
    }
    if (!is_within_double(api.fade_beats, 0.0, lfo_longest_delay_beats)) {
        return false;
    }
    if (!is_within_double(api.smooth, 0.0, 1.0)) {
        return false;
    }

    lfo.shape = static_cast<LfoShape>(api.shape);
    lfo.step_count = api.step_count;
    lfo.steps.fill(0.0f);
    for (int i = 0; i < api.step_count; i++) {
        lfo.steps[i] = api.steps[i];
    }
    lfo.per_note = api.per_note;
    lfo.speed = static_cast<LfoSpeed>(api.speed);
    lfo.hz = api.hz;
    lfo.beats = api.beats;
    lfo.presses = api.presses;
    lfo.start = api.start;
    lfo.spread = api.spread;
    lfo.delay_ms = api.delay_ms;
    lfo.fade_ms = api.fade_ms;
    lfo.delay_beats = api.delay_beats;
    lfo.fade_beats = api.fade_beats;
    lfo.smooth = api.smooth;
    lfo.once = api.once;
    return true;
}

bool is_valid_source(int source) {
    if (source < JELLO_SOURCE_NONE) {
        return false;
    }
    return source <= JELLO_SOURCE_FILTER_ENVELOPE;
}

// A connection from the C API, checked: false if any part is out of range.
bool connection_of_api(const JelloConnection& api, Connection& connection) {
    if (!is_valid_source(api.source)) {
        return false;
    }
    if (!is_valid_source(api.scaled_by)) {
        return false;
    }
    if (api.target < JELLO_TARGET_PITCH) {
        return false;
    }
    if (api.target > JELLO_TARGET_EFFECT) {
        return false;
    }

    RoutingTarget target = static_cast<RoutingTarget>(api.target);
    float most = most_amount(target);
    if (!is_within(api.amount, -most, most)) {
        return false;
    }

    // An effect's slot and value. Whether the effect there has that value
    // is only known when it moves, since the slot may change.
    connection.effect_slot = 0;
    connection.effect_value = 0;
    if (target == RoutingTarget::effect) {
        if (!is_within_int(api.effect_slot, 0, JELLO_EFFECT_SLOTS - 1)) {
            return false;
        }
        if (!is_within_int(api.effect_value, 0, JELLO_EFFECT_MOST_VALUES - 1)) {
            return false;
        }
        connection.effect_slot = api.effect_slot;
        connection.effect_value = api.effect_value;
    }

    connection.source = static_cast<RoutingSource>(api.source);
    connection.target = target;
    connection.amount = api.amount;
    connection.one_way = api.one_way;
    connection.snap = api.snap;
    connection.scaled_by = static_cast<RoutingSource>(api.scaled_by);
    return true;
}

// The C API and the song player agree on how big a song and a chord can be.
static_assert(JELLO_SONG_MAX_CHORDS == SongPlayer::max_chords,
              "the C API and the song player hold the same number of chords");
static_assert(JELLO_CHORD_MAX_NOTES == SongChord::max_notes,
              "the C API and the song player allow the same notes per chord");

bool is_valid_chord(const JelloSongChord& chord) {
    if (chord.note_count < 0) {
        return false;
    }
    if (chord.note_count > JELLO_CHORD_MAX_NOTES) {
        return false;
    }
    if (!is_valid_velocity(chord.velocity)) {
        return false;
    }
    for (int i = 0; i < chord.note_count; i++) {
        if (!is_valid_note(chord.notes[i])) {
            return false;
        }
    }
    return true;
}

// The song player's own copy of a chord from the C API.
SongChord song_chord_of(const JelloSongChord& chord) {
    SongChord copy{};
    for (int i = 0; i < chord.note_count; i++) {
        copy.notes[i] = chord.notes[i];
    }
    copy.note_count = chord.note_count;
    copy.velocity = chord.velocity;
    copy.length = chord.length;
    return copy;
}

// The looper's state as a JELLO_LOOPER_ number. Closing is still recording,
// as far as the player can tell.
int looper_state_number(Looper::State state) {
    if (state == Looper::State::counting_in) {
        return JELLO_LOOPER_COUNTING_IN;
    } else if (state == Looper::State::recording) {
        return JELLO_LOOPER_RECORDING;
    } else if (state == Looper::State::closing) {
        return JELLO_LOOPER_RECORDING;
    } else if (state == Looper::State::looping) {
        return JELLO_LOOPER_LOOPING;
    } else if (state == Looper::State::layering) {
        return JELLO_LOOPER_LAYERING;
    } else if (state == Looper::State::paused) {
        return JELLO_LOOPER_PAUSED;
    }
    return JELLO_LOOPER_IDLE;
}

// The C API's number for a change to the loop's notes.
int loop_change_number(LoopChange::Type type) {
    if (type == LoopChange::Type::started) {
        return JELLO_LOOP_STARTED;
    } else if (type == LoopChange::Type::ended) {
        return JELLO_LOOP_ENDED;
    } else if (type == LoopChange::Type::kept) {
        return JELLO_LOOP_KEPT;
    } else if (type == LoopChange::Type::layer_started) {
        return JELLO_LOOP_LAYER_STARTED;
    } else if (type == LoopChange::Type::layer_stopped) {
        return JELLO_LOOP_LAYER_STOPPED;
    } else if (type == LoopChange::Type::layer_dropped) {
        return JELLO_LOOP_LAYER_DROPPED;
    } else if (type == LoopChange::Type::layer_removed) {
        return JELLO_LOOP_LAYER_REMOVED;
    } else if (type == LoopChange::Type::loaded) {
        return JELLO_LOOP_LOADED;
    }
    return JELLO_LOOP_FORGOT;
}

}  // namespace

// Everything one engine owns.
struct JelloEngine {
    Conductor conductor{sample_rate};
    ma_device device;
    bool audio_running = false;

    // The microphone: its own device, open only while listening, so the
    // system's mic light is only on then. Its sound waits in `heard`.
    ma_device mic;
    bool capturing = false;
    CaptureRing heard;

    // The mics found the last time jello_engine_list_inputs asked, and the
    // context their ids belong to, made the first time it asks.
    ma_context context;
    bool has_context = false;
    std::vector<ma_device_info> inputs;

    // The sample sets, by number; empty where there is none. A forgotten
    // set waits here until the audio thread is done with it: until it has
    // acted on the message that last sent the set (its count in
    // `last_sent`) and no longer says it plays it.
    std::array<std::unique_ptr<SampleSet>, sample_set_most> sets;
    std::array<bool, sample_set_most> forgotten{};
    std::array<std::uint64_t, sample_set_most> last_sent{};

    // The kind of effect in every chain's slots, as sent, so values can be
    // checked against it before they go.
    std::array<std::array<EffectKind, Chain::slot_count>, Mixer::chain_count> effect_kinds{};
};

namespace {

// miniaudio calls this on the audio thread every few milliseconds, whenever
// the speakers need more sound. It must never lock, allocate, log or wait.
void on_audio(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)input;  // the speakers' device has no microphone

    JelloEngine* engine = static_cast<JelloEngine*>(device->pUserData);
    engine->conductor.render(static_cast<float*>(output), static_cast<int>(frames));
}

// miniaudio calls this on the mic's own audio thread whenever it has heard
// more. The same rules: never lock, allocate, log or wait.
void on_mic(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)output;  // the mic's device has no speakers

    JelloEngine* engine = static_cast<JelloEngine*>(device->pUserData);
    engine->heard.write(static_cast<const float*>(input), static_cast<int>(frames));
}

// Set `set`'s number is one the engine keeps and has not forgotten.
bool is_sendable_set(const JelloEngine* engine, int set) {
    if (!is_within_int(set, 0, sample_set_most - 1)) {
        return false;
    }
    if (engine->sets[set] == nullptr) {
        return false;
    }
    return !engine->forgotten[set];
}

// The set to send for number `set`, nullptr for -1.
const SampleSet* set_to_send(const JelloEngine* engine, int set) {
    if (set == -1) {
        return nullptr;
    }
    return engine->sets[set].get();
}

}  // namespace

extern "C" {

JelloEngine* jello_engine_create(void) {
    return new (std::nothrow) JelloEngine();
}

void jello_engine_destroy(JelloEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    jello_engine_stop_audio(engine);
    jello_engine_stop_capture(engine);
    if (engine->has_context) {
        ma_context_uninit(&engine->context);
    }
    delete engine;
}

bool jello_engine_start_audio(JelloEngine* engine) {
    if (engine->audio_running) {
        return true;
    }

    // Ask for stereo floats at our sample rate. If the speakers run at another
    // rate, miniaudio converts for us.
    ma_device_config config = ma_device_config_init(ma_device_type_playback);
    config.playback.format = ma_format_f32;
    config.playback.channels = channels;
    config.sampleRate = sample_rate;
    config.dataCallback = on_audio;
    config.pUserData = engine;

    if (ma_device_init(nullptr, &config, &engine->device) != MA_SUCCESS) {
        return false;
    }
    if (ma_device_start(&engine->device) != MA_SUCCESS) {
        ma_device_uninit(&engine->device);
        return false;
    }

    engine->audio_running = true;
    return true;
}

void jello_engine_stop_audio(JelloEngine* engine) {
    if (!engine->audio_running) {
        return;
    }
    ma_device_uninit(&engine->device);
    engine->audio_running = false;
}

void jello_engine_note_on(JelloEngine* engine, int track, int note, int velocity) {
    // Ignore anything MIDI could never send, so nothing odd reaches the synth.
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_played_note(note)) {
        return;
    }
    if (!is_valid_velocity(velocity)) {
        return;
    }
    engine->conductor.note_on(track, note, velocity);
}

void jello_engine_move_string(JelloEngine* engine, int track, int from, int to) {
    if (!is_valid_track(track)) {
        return;
    }
    if (from < 0 || from > 127 || to < 0 || to > 127) {
        return;
    }
    engine->conductor.move_string(track, from, to);
}

void jello_engine_pluck_string_after(JelloEngine* engine, int track, int from, int to, int velocity) {
    if (!is_valid_track(track)) {
        return;
    }
    if (from < 0 || from > 127 || to < 0 || to > 127) {
        return;
    }
    if (velocity < 1 || velocity > 127) {
        return;
    }
    engine->conductor.pluck_string_after(track, from, to, velocity);
}

void jello_engine_note_off(JelloEngine* engine, int track, int note) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_played_note(note)) {
        return;
    }
    engine->conductor.note_off(track, note);
}

void jello_engine_all_notes_off(JelloEngine* engine) {
    engine->conductor.all_notes_off();
}

void jello_engine_set_track_volume(JelloEngine* engine, int track, float volume) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_volume(volume)) {
        return;
    }
    engine->conductor.set_track_volume(track, volume);
}

void jello_engine_set_track_pan(JelloEngine* engine, int track, float pan) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_pan(pan)) {
        return;
    }
    engine->conductor.set_track_pan(track, pan);
}

void jello_engine_set_send(JelloEngine* engine, int track, int bus, float amount) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_bus(bus)) {
        return;
    }
    if (!is_valid_volume(amount)) {
        return;
    }
    engine->conductor.set_send(track, bus, amount);
}

void jello_engine_set_bus_volume(JelloEngine* engine, int bus, float volume) {
    if (!is_valid_bus(bus)) {
        return;
    }
    if (!is_valid_volume(volume)) {
        return;
    }
    engine->conductor.set_bus_volume(bus, volume);
}

void jello_engine_set_bus_pan(JelloEngine* engine, int bus, float pan) {
    if (!is_valid_bus(bus)) {
        return;
    }
    if (!is_valid_pan(pan)) {
        return;
    }
    engine->conductor.set_bus_pan(bus, pan);
}

void jello_engine_set_limiter(JelloEngine* engine, float boost_db, float ceiling_db, int release_ms) {
    if (!is_within(boost_db, 0.0f, limiter_most_boost_db)) {
        return;
    }
    if (!is_within(ceiling_db, limiter_lowest_ceiling_db, limiter_highest_ceiling_db)) {
        return;
    }
    if (!is_within_int(release_ms, limiter_shortest_release_ms, limiter_longest_release_ms)) {
        return;
    }
    LimiterSettings settings;
    settings.boost_db = boost_db;
    settings.ceiling_db = ceiling_db;
    settings.release_ms = release_ms;
    engine->conductor.set_limiter(settings);
}

int jello_engine_use_lookahead(JelloEngine* engine) {
    return engine->conductor.use_lookahead();
}

float jello_engine_limiter_reduction(JelloEngine* engine) {
    return engine->conductor.limiter_reduction_db();
}

void jello_engine_take_meters(JelloEngine* engine, float* peaks, float* true_peak, float* limiter_reduction_db,
                              float* short_term_lufs) {
    Conductor::Meters meters = engine->conductor.take_meters();
    for (int chain = 0; chain < JELLO_CHAIN_COUNT; chain++) {
        peaks[chain] = meters.peaks[chain];
    }
    *true_peak = meters.true_peak;
    *limiter_reduction_db = meters.limiter_reduction_db;
    *short_term_lufs = meters.short_term_lufs;
}

bool jello_engine_set_effect(JelloEngine* engine, int chain, int slot, int kind) {
    if (!is_valid_chain(chain)) {
        return false;
    }
    if (!is_valid_effect_slot(slot)) {
        return false;
    }
    if (!is_within_int(kind, JELLO_EFFECT_NONE, JELLO_EFFECT_REVERB)) {
        return false;
    }
    EffectKind effect_kind = static_cast<EffectKind>(kind);
    if (!engine->conductor.set_effect(chain, slot, effect_kind)) {
        return false;
    }
    engine->effect_kinds[chain][slot] = effect_kind;
    return true;
}

bool jello_engine_move_effect(JelloEngine* engine, int chain, int from, int to) {
    if (!is_valid_chain(chain)) {
        return false;
    }
    if (!is_valid_effect_slot(from)) {
        return false;
    }
    if (!is_valid_effect_slot(to)) {
        return false;
    }
    if (!engine->conductor.move_effect(chain, from, to)) {
        return false;
    }

    // The kinds move the same way as the slots (Chain::move).
    auto first = engine->effect_kinds[chain].begin();
    if (from < to) {
        std::rotate(first + from, first + from + 1, first + to + 1);
    } else if (from > to) {
        std::rotate(first + to, first + from, first + from + 1);
    }
    return true;
}

void jello_engine_set_effect_on(JelloEngine* engine, int chain, int slot, bool on) {
    if (!is_valid_chain(chain)) {
        return;
    }
    if (!is_valid_effect_slot(slot)) {
        return;
    }
    if (engine->effect_kinds[chain][slot] == EffectKind::none) {
        return;
    }
    engine->conductor.set_effect_on(chain, slot, on);
}

void jello_engine_set_effect_value(JelloEngine* engine, int chain, int slot, int value, float number) {
    if (!is_valid_chain(chain)) {
        return;
    }
    if (!is_valid_effect_slot(slot)) {
        return;
    }
    EffectKind kind = engine->effect_kinds[chain][slot];
    if (!is_valid_effect_value(kind, value, number)) {
        return;
    }
    if (is_listens_to_value(kind, value)) {
        if (!can_listen_to(chain, static_cast<int>(number))) {
            return;
        }
    }
    engine->conductor.set_effect_value(chain, slot, value, number);
}

int jello_engine_effect_count(JelloEngine* engine) {
    return engine->conductor.effect_count();
}

int jello_engine_message_room(JelloEngine* engine) {
    return static_cast<int>(engine->conductor.message_room());
}

void jello_engine_take_messages(JelloEngine* engine) {
    if (engine->audio_running) {
        return;
    }
    engine->conductor.take_waiting_messages();
}

uint64_t jello_engine_dropped_messages(JelloEngine* engine) {
    return engine->conductor.dropped_messages();
}

bool jello_engine_set_chain_order(JelloEngine* engine, const int* order, int count) {
    if (order == nullptr) {
        return false;
    }
    if (count != JELLO_MASTER_CHAIN) {
        return false;
    }
    std::array<int, Mixer::master_chain> sent{};
    std::array<bool, Mixer::master_chain> seen{};
    for (int place = 0; place < count; place++) {
        int chain = order[place];
        if (!is_within_int(chain, 0, JELLO_MASTER_CHAIN - 1)) {
            return false;
        }
        if (seen[chain]) {
            return false;
        }
        // Every track before every bus, since a bus needs all the tracks
        // send it.
        if (chain < JELLO_FIRST_BUS_CHAIN) {
            if (place >= JELLO_FIRST_BUS_CHAIN) {
                return false;
            }
        }
        seen[chain] = true;
        sent[place] = chain;
    }
    return engine->conductor.set_chain_order(sent);
}

float jello_engine_gain_reduction(JelloEngine* engine, int chain, int slot) {
    if (!is_valid_chain(chain)) {
        return 0.0f;
    }
    if (!is_valid_effect_slot(slot)) {
        return 0.0f;
    }
    return engine->conductor.effect_reduction_db(chain, slot);
}

void jello_engine_set_oscillator(JelloEngine* engine, int track, int index, int kind, float level, int octaves,
                                 int half_steps, float cents, float width, int touch, float decay_seconds, float tone,
                                 float pick, float pickup, float body, float pickup_tone) {
    // Anything out of range is ignored, so nothing odd reaches the synth.
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(index, 0, JELLO_OSCILLATOR_COUNT - 1)) {
        return;
    }
    if (!is_within_int(kind, JELLO_KIND_SINE, JELLO_KIND_STRING)) {
        return;
    }
    if (!is_within(level, 0.0f, 1.0f)) {
        return;
    }
    if (!is_within_int(octaves, -JELLO_OSCILLATOR_MOST_OCTAVES, JELLO_OSCILLATOR_MOST_OCTAVES)) {
        return;
    }
    if (!is_within_int(half_steps, -JELLO_OSCILLATOR_MOST_HALF_STEPS, JELLO_OSCILLATOR_MOST_HALF_STEPS)) {
        return;
    }
    if (!is_within(cents, -JELLO_OSCILLATOR_MOST_CENTS, JELLO_OSCILLATOR_MOST_CENTS)) {
        return;
    }
    if (!is_within(width, oscillator_narrowest_width, oscillator_widest_width)) {
        return;
    }
    if (!is_within_int(touch, JELLO_TOUCH_SOFTEST, JELLO_TOUCH_HARDEST)) {
        return;
    }
    if (!is_within(decay_seconds, string_shortest_decay_seconds, string_longest_decay_seconds)) {
        return;
    }
    if (!is_within(tone, 0.0f, 1.0f) || !is_within(pick, 0.02f, 0.5f) || !is_within(body, 0.0f, 1.0f)) {
        return;
    }
    if (!is_within(pickup_tone, 0.0f, 1.0f)) {
        return;
    }
    if (!is_within(pickup, 0.0f, 0.5f)) {
        return;
    }

    OscillatorSettings oscillator;
    oscillator.kind = static_cast<Kind>(kind);
    oscillator.level = level;
    oscillator.octaves = octaves;
    oscillator.half_steps = half_steps;
    oscillator.cents = cents;
    oscillator.width = width;
    oscillator.touch = touch;
    oscillator.string.decay_seconds = decay_seconds;
    oscillator.string.tone = tone;
    oscillator.string.pick = pick;
    oscillator.string.pickup = pickup;
    oscillator.string.pickup_tone = pickup_tone;
    oscillator.string.body = body;
    engine->conductor.set_oscillator(track, index, oscillator);
}

void jello_engine_start_click(JelloEngine* engine) {
    engine->conductor.start_click();
}

void jello_engine_stop_click(JelloEngine* engine) {
    engine->conductor.stop_click();
}

void jello_engine_set_tempo(JelloEngine* engine, int tempo) {
    if (!is_valid_tempo(tempo)) {
        return;
    }
    engine->conductor.set_tempo(tempo);
}

void jello_engine_press_looper(JelloEngine* engine, int track) {
    if (!is_valid_track(track)) {
        return;
    }
    engine->conductor.press_looper(track);
}

void jello_engine_clear_loop(JelloEngine* engine) {
    engine->conductor.clear_loop();
}

void jello_engine_remove_layer(JelloEngine* engine, int layer) {
    if (layer == JELLO_NEWEST_LAYER) {
        engine->conductor.remove_layer(layer);
        return;
    }
    if (!is_valid_layer(layer)) {
        return;
    }
    engine->conductor.remove_layer(layer);
}

void jello_engine_set_layer_volume(JelloEngine* engine, int layer, float volume) {
    if (!is_valid_layer(layer)) {
        return;
    }
    if (!is_valid_volume(volume)) {
        return;
    }
    engine->conductor.set_layer_volume(layer, volume);
}

void jello_engine_pause_loop(JelloEngine* engine) {
    engine->conductor.pause_loop();
}

void jello_engine_play_loop(JelloEngine* engine) {
    engine->conductor.play_loop();
}

void jello_engine_set_layer_playing(JelloEngine* engine, int layer, bool playing) {
    if (!is_valid_layer(layer)) {
        return;
    }
    engine->conductor.set_layer_playing(layer, playing);
}

bool jello_engine_play_song(JelloEngine* engine, int track, const JelloSongChord* chords, int count) {
    if (!is_valid_track(track)) {
        return false;
    }
    if (count < 0) {
        return false;
    }
    if (count > JELLO_SONG_MAX_CHORDS) {
        return false;
    }

    // Every chord is checked before any is sent, so a song with one odd
    // chord plays none of them. The copies live here, on the calling
    // thread, until they are sent.
    std::array<SongChord, JELLO_SONG_MAX_CHORDS> song{};
    for (int i = 0; i < count; i++) {
        if (!is_valid_chord(chords[i])) {
            return false;
        }
        song[i] = song_chord_of(chords[i]);
    }
    return engine->conductor.play_song(track, song.data(), count);
}

void jello_engine_stop_song(JelloEngine* engine) {
    engine->conductor.stop_song();
}

void jello_engine_set_filter(JelloEngine* engine, int track, int type, float cutoff_hz, float resonance) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(type, JELLO_FILTER_LOW_PASS, JELLO_FILTER_NOTCH)) {
        return;
    }
    if (!is_within(cutoff_hz, JELLO_FILTER_LOWEST_HZ, JELLO_FILTER_OPEN_HZ)) {
        return;
    }
    if (!is_within(resonance, 0.0f, 1.0f)) {
        return;
    }

    FilterSettings settings;
    settings.type = static_cast<FilterType>(type);
    settings.cutoff_hz = cutoff_hz;
    settings.resonance = resonance;
    engine->conductor.set_filter(track, settings);
}

void jello_engine_set_filter_envelope(JelloEngine* engine, int track, float amount, int attack_ms, int decay_ms,
                                      float sustain, int release_ms) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within(amount, 0.0f, JELLO_FILTER_MOST_OCTAVES)) {
        return;
    }
    if (!is_valid_stage_ms(attack_ms)) {
        return;
    }
    if (!is_valid_stage_ms(decay_ms)) {
        return;
    }
    if (!is_within(sustain, 0.0f, 1.0f)) {
        return;
    }
    if (!is_valid_stage_ms(release_ms)) {
        return;
    }

    FilterEnvelopeSettings envelope;
    envelope.amount = amount;
    envelope.attack_ms = attack_ms;
    envelope.decay_ms = decay_ms;
    envelope.sustain = sustain;
    envelope.release_ms = release_ms;
    engine->conductor.set_filter_envelope(track, envelope);
}

void jello_engine_set_amp(JelloEngine* engine, int track, int on, float gain, float bass, float middle, float treble,
                          float presence, float speaker, float volume) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(on, 0, 1)) {
        return;
    }
    const float knobs[] = {gain, bass, middle, treble, presence, speaker, volume};
    for (float knob : knobs) {
        if (!is_within(knob, 0.0f, 1.0f)) {
            return;
        }
    }
    AmpSettings amp;
    amp.on = on == 1;
    amp.gain = gain;
    amp.bass = bass;
    amp.middle = middle;
    amp.treble = treble;
    amp.presence = presence;
    amp.speaker = speaker;
    amp.volume = volume;
    engine->conductor.set_amp(track, amp);
}

void jello_engine_set_unison(JelloEngine* engine, int track, int copies, float detune_cents, float spread) {
    if (!is_valid_track(track)) {
        return;
    }
    if (copies < 1) {
        return;
    }
    if (copies > JELLO_UNISON_MOST_COPIES) {
        return;
    }
    if (!is_within(detune_cents, 0.0f, JELLO_UNISON_MOST_DETUNE_CENTS)) {
        return;
    }
    if (!is_within(spread, 0.0f, 1.0f)) {
        return;
    }

    UnisonSettings unison;
    unison.copies = copies;
    unison.detune_cents = detune_cents;
    unison.spread = spread;
    engine->conductor.set_unison(track, unison);
}

void jello_engine_set_loudness_envelope(JelloEngine* engine, int track, int attack_ms, int decay_ms, float sustain,
                                        int release_ms) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_loudness_ms(attack_ms)) {
        return;
    }
    if (!is_valid_loudness_ms(decay_ms)) {
        return;
    }
    if (!is_within(sustain, 0.0f, 1.0f)) {
        return;
    }
    if (!is_valid_loudness_ms(release_ms)) {
        return;
    }

    LoudnessEnvelopeSettings envelope;
    envelope.attack_ms = attack_ms;
    envelope.decay_ms = decay_ms;
    envelope.sustain = sustain;
    envelope.release_ms = release_ms;
    engine->conductor.set_loudness_envelope(track, envelope);
}

void jello_engine_set_key_tracking(JelloEngine* engine, int track, float key_tracking) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within(key_tracking, 0.0f, 1.0f)) {
        return;
    }
    engine->conductor.set_key_tracking(track, key_tracking);
}

void jello_engine_set_tuning(JelloEngine* engine, int track, float a4_hz) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within(a4_hz, JELLO_TUNING_LOWEST_HZ, JELLO_TUNING_HIGHEST_HZ)) {
        return;
    }
    engine->conductor.set_tuning(track, a4_hz);
}

void jello_engine_set_lfo(JelloEngine* engine, int track, int index, const JelloLfo* lfo) {
    // A bus's or the master's chain number for its own LFOs.
    if (!is_valid_chain(track)) {
        return;
    }
    if (!is_within_int(index, 0, JELLO_LFO_COUNT - 1)) {
        return;
    }
    LfoSettings settings;
    if (!lfo_of_api(*lfo, settings)) {
        return;
    }
    engine->conductor.set_lfo(track, index, settings);
}

void jello_engine_set_connection(JelloEngine* engine, int track, int index, const JelloConnection* connection) {
    if (!is_valid_chain(track)) {
        return;
    }
    if (!is_within_int(index, 0, JELLO_CONNECTION_COUNT - 1)) {
        return;
    }
    Connection checked;
    if (!connection_of_api(*connection, checked)) {
        return;
    }
    engine->conductor.set_connection(track, index, checked);
}

void jello_engine_set_vibrato_key_times(JelloEngine* engine, int track, int rise_ms, int fall_ms) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(rise_ms, 0, JELLO_VIBRATO_KEY_LONGEST_MS)) {
        return;
    }
    if (!is_within_int(fall_ms, 0, JELLO_VIBRATO_KEY_LONGEST_MS)) {
        return;
    }
    engine->conductor.set_vibrato_key_times(track, rise_ms, fall_ms);
}

void jello_engine_set_vibrato_key(JelloEngine* engine, int track, bool held) {
    if (!is_valid_track(track)) {
        return;
    }
    engine->conductor.set_vibrato_key(track, held);
}

void jello_engine_set_glide_key(JelloEngine* engine, int track, bool held) {
    if (!is_valid_track(track)) {
        return;
    }
    engine->conductor.set_glide_key(track, held);
}

void jello_engine_set_pedal_key(JelloEngine* engine, int track, bool held) {
    if (!is_valid_track(track)) {
        return;
    }
    engine->conductor.set_pedal_key(track, held);
}

void jello_engine_set_glide_time(JelloEngine* engine, int track, int glide_ms) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(glide_ms, 0, JELLO_GLIDE_LONGEST_MS)) {
        return;
    }
    engine->conductor.set_glide_time(track, glide_ms);
}

void jello_engine_time_next(JelloEngine* engine, uint64_t sample) {
    engine->conductor.time_next(sample);
}

void jello_engine_send_now(JelloEngine* engine) {
    engine->conductor.send_now();
}

void jello_engine_play_sequence(JelloEngine* engine) {
    engine->conductor.play_sequence();
}

void jello_engine_stop_sequence(JelloEngine* engine) {
    engine->conductor.stop_sequence();
}

int64_t jello_engine_sequence_position(JelloEngine* engine) {
    std::uint64_t position = engine->conductor.sequence_position();
    if (position == Sequencer::not_playing) {
        return -1;
    }
    return static_cast<int64_t>(position);
}

int jello_engine_sequence_room(JelloEngine* engine) {
    return static_cast<int>(engine->conductor.sequence_room());
}

void jello_engine_layer_note_on(JelloEngine* engine, int track, int slot, int note, int velocity) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_slot(slot)) {
        return;
    }
    if (!is_valid_loop_pitch(note)) {
        return;
    }
    if (!is_valid_velocity(velocity)) {
        return;
    }
    engine->conductor.layer_note_on(track, slot, note, velocity);
}

void jello_engine_layer_note_off(JelloEngine* engine, int track, int slot, int note) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_slot(slot)) {
        return;
    }
    if (!is_valid_loop_pitch(note)) {
        return;
    }
    engine->conductor.layer_note_off(track, slot, note);
}

void jello_engine_set_slot_volume(JelloEngine* engine, int track, int slot, float volume) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_valid_slot(slot)) {
        return;
    }
    if (!is_valid_volume(volume)) {
        return;
    }
    engine->conductor.set_slot_volume(track, slot, volume);
}

JelloLooperStatus jello_engine_looper_status(JelloEngine* engine) {
    LooperStatus status = engine->conductor.looper_status();

    JelloLooperStatus answer;
    answer.state = looper_state_number(status.state);
    answer.number = status.number;
    answer.position = status.position;
    answer.waiting = status.waiting;
    return answer;
}

bool jello_engine_next_loop_change(JelloEngine* engine, JelloLoopChange* change) {
    LoopChange taken;
    if (!engine->conductor.next_loop_change(taken)) {
        return false;
    }
    change->type = loop_change_number(taken.type);
    change->layer = taken.layer;
    change->index = taken.index;
    change->note = taken.note;
    change->sample = taken.sample;
    change->track = taken.track;
    return true;
}

bool jello_engine_plan_loop(JelloEngine* engine, const JelloLoopNote* played, int count, JelloLoopNote* planned) {
    (void)engine;  // the plan needs nothing from the engine
    if (count < 0) {
        return false;
    }
    if (count > JELLO_LOOP_MAX_NOTES) {
        return false;
    }

    // The looper's own copies of the notes. This runs on the calling
    // thread, not the audio thread, so it may ask for memory.
    std::vector<LoopNote> notes(static_cast<std::size_t>(count));
    for (int i = 0; i < count; i++) {
        if (!is_valid_loop_pitch(played[i].note)) {
            return false;
        }
        notes[i].note = played[i].note;
        notes[i].velocity = 0;  // not needed to plan
        notes[i].start = played[i].start;
        notes[i].end = played[i].end;
    }

    std::vector<LoopNote> plan(static_cast<std::size_t>(count));
    plan_loop(notes.data(), count, plan.data());
    for (int i = 0; i < count; i++) {
        planned[i].note = plan[i].note;
        planned[i].start = plan[i].start;
        planned[i].end = plan[i].end;
    }
    return true;
}

bool jello_engine_load_loop(JelloEngine* engine, const JelloLoadedNote* notes, int count, int layers, int bars) {
    if (layers < 1) {
        return false;
    }
    if (layers > JELLO_LOOP_MAX_LAYERS) {
        return false;
    }
    if (count < 1) {
        return false;
    }
    if (count > JELLO_LOOP_MAX_NOTES) {
        return false;
    }
    if (bars < 1) {
        return false;
    }

    // Every note is checked before any is sent, so a loop with one odd note
    // never half loads. The layers come in order with none skipped: each
    // note's layer is the last one's, or the next.
    // All of a layer's notes are on its track, and no track has more layers
    // than it holds.
    std::vector<LoadedNote> loaded(static_cast<std::size_t>(count));
    std::array<int, JELLO_TRACK_COUNT> track_layers{};
    int last_layer = 0;
    for (int i = 0; i < count; i++) {
        const JelloLoadedNote& note = notes[i];
        if (!is_valid_track(note.track)) {
            return false;
        }
        bool starts_layer = false;
        if (i == 0) {
            if (note.layer != 0) {
                return false;
            }
            starts_layer = true;
        } else if (note.layer == last_layer + 1) {
            last_layer = note.layer;
            starts_layer = true;
        } else if (note.layer != last_layer) {
            return false;
        } else if (note.track != notes[i - 1].track) {
            return false;
        }
        if (starts_layer) {
            track_layers[note.track] += 1;
            if (track_layers[note.track] > JELLO_TRACK_MAX_LAYERS) {
                return false;
            }
        }
        if (!is_valid_loop_pitch(note.note)) {
            return false;
        }
        if (!is_valid_velocity(note.velocity)) {
            return false;
        }
        if (note.end < note.start) {
            return false;
        }
        // Within a layer, in the order they start.
        if (i > 0) {
            if (notes[i - 1].layer == note.layer) {
                if (note.start < notes[i - 1].start) {
                    return false;
                }
            }
        }

        loaded[i].layer = note.layer;
        loaded[i].track = note.track;
        loaded[i].note.note = note.note;
        loaded[i].note.velocity = note.velocity;
        loaded[i].note.start = note.start;
        loaded[i].note.end = note.end;
    }
    // Every layer has a note.
    if (last_layer != layers - 1) {
        return false;
    }

    return engine->conductor.load_loop(loaded.data(), count, layers, bars);
}

int64_t jello_engine_song_position(JelloEngine* engine) {
    std::uint64_t position = engine->conductor.song_position();
    if (position == SongPlayer::not_playing) {
        return -1;
    }
    return static_cast<int64_t>(position);
}

int jello_engine_sample_rate(JelloEngine* engine) {
    (void)engine;  // every engine runs at the same rate, for now
    return sample_rate;
}

int jello_engine_list_inputs(JelloEngine* engine) {
    if (engine->capturing) {
        return static_cast<int>(engine->inputs.size());
    }
    if (!engine->has_context) {
        if (ma_context_init(nullptr, 0, nullptr, &engine->context) != MA_SUCCESS) {
            return 0;
        }
        engine->has_context = true;
    }

    ma_device_info* playback = nullptr;
    ma_uint32 playback_count = 0;
    ma_device_info* capture = nullptr;
    ma_uint32 capture_count = 0;
    engine->inputs.clear();
    if (ma_context_get_devices(&engine->context, &playback, &playback_count, &capture, &capture_count) != MA_SUCCESS) {
        return 0;
    }
    for (ma_uint32 i = 0; i < capture_count; i++) {
        engine->inputs.push_back(capture[i]);
    }
    return static_cast<int>(engine->inputs.size());
}

bool jello_engine_input_name(JelloEngine* engine, int index, char* name, int length) {
    if (!is_within_int(index, 0, static_cast<int>(engine->inputs.size()) - 1)) {
        return false;
    }
    if (length < 1) {
        return false;
    }
    const char* found = engine->inputs[static_cast<std::size_t>(index)].name;
    std::size_t room = static_cast<std::size_t>(length) - 1;
    std::size_t size = std::strlen(found);
    if (size > room) {
        size = room;
    }
    std::memcpy(name, found, size);
    name[size] = '\0';
    return true;
}

bool jello_engine_start_capture(JelloEngine* engine, int input) {
    if (engine->capturing) {
        return true;
    }
    if (input != JELLO_DEFAULT_INPUT) {
        if (!is_within_int(input, 0, static_cast<int>(engine->inputs.size()) - 1)) {
            return false;
        }
    }

    // One channel of floats at the engine's rate. If the mic runs at another
    // rate, or in stereo, miniaudio converts for us.
    ma_device_config config = ma_device_config_init(ma_device_type_capture);
    config.capture.format = ma_format_f32;
    config.capture.channels = 1;
    config.sampleRate = sample_rate;
    config.dataCallback = on_mic;
    config.pUserData = engine;

    ma_context* context = nullptr;
    if (input != JELLO_DEFAULT_INPUT) {
        config.capture.pDeviceID = &engine->inputs[static_cast<std::size_t>(input)].id;
        context = &engine->context;
    }

    engine->heard.clear();
    if (ma_device_init(context, &config, &engine->mic) != MA_SUCCESS) {
        return false;
    }
    if (ma_device_start(&engine->mic) != MA_SUCCESS) {
        ma_device_uninit(&engine->mic);
        return false;
    }
    engine->capturing = true;
    return true;
}

void jello_engine_stop_capture(JelloEngine* engine) {
    if (!engine->capturing) {
        return;
    }
    ma_device_uninit(&engine->mic);
    engine->capturing = false;
}

int jello_engine_read_capture(JelloEngine* engine, float* out, int most) {
    return engine->heard.read(out, most);
}

float jello_engine_capture_peak(JelloEngine* engine) {
    return engine->heard.take_peak();
}

uint64_t jello_engine_capture_dropped(JelloEngine* engine) {
    return engine->heard.dropped();
}

void jello_engine_feed_capture(JelloEngine* engine, const float* samples, int frames) {
    if (engine->capturing) {
        return;
    }
    if (frames < 0) {
        return;
    }
    engine->heard.write(samples, frames);
}

void jello_engine_render(JelloEngine* engine, float* out, int frames) {
    engine->conductor.render(out, frames);
}

int jello_engine_add_sample_set(JelloEngine* engine, const JelloSampleZone* zones, int count) {
    jello_engine_tidy_sample_sets(engine);
    if (count < 0) {
        return -1;
    }
    int id = -1;
    for (int i = 0; i < sample_set_most; i++) {
        if (engine->sets[i] == nullptr) {
            id = i;
            break;
        }
    }
    if (id == -1) {
        return -1;
    }

    // Made here, on the calling thread, so it may ask for memory.
    auto set = std::make_unique<SampleSet>(id);
    for (int i = 0; i < count; i++) {
        const JelloSampleZone& given = zones[i];
        if (!is_within_int(given.lowest_note, 0, 127)) {
            return -1;
        }
        if (!is_within_int(given.highest_note, 0, 127)) {
            return -1;
        }
        if (!is_within_int(given.root_note, 0, 127)) {
            return -1;
        }
        if (!is_within(given.tune_cents, -100.0f, 100.0f)) {
            return -1;
        }
        if (given.loop_start > given.loop_end) {
            return -1;
        }
        if (!is_within_int(given.lowest_touch, JELLO_TOUCH_SOFTEST, JELLO_TOUCH_HARDEST)) {
            return -1;
        }
        if (!is_within_int(given.highest_touch, JELLO_TOUCH_SOFTEST, JELLO_TOUCH_HARDEST)) {
            return -1;
        }
        if (!is_within_int(given.trigger, JELLO_TRIGGER_ATTACK, JELLO_TRIGGER_PEDAL_UP)) {
            return -1;
        }
        if (!is_within(given.fade_db_per_second, 0.0f, 1000.0f)) {
            return -1;
        }
        if (!is_within(given.touch_share, 0.0f, 1.0f)) {
            return -1;
        }
        SampleZone zone;
        zone.lowest_note = given.lowest_note;
        zone.highest_note = given.highest_note;
        zone.root_note = given.root_note;
        zone.tune_cents = given.tune_cents;
        zone.channels = given.channels;
        zone.frame_count = given.frame_count;
        zone.loop_start = given.loop_start;
        zone.loop_end = given.loop_end;
        zone.lowest_touch = given.lowest_touch;
        zone.highest_touch = given.highest_touch;
        zone.trigger = static_cast<SampleTrigger>(given.trigger);
        zone.fade_db_per_second = given.fade_db_per_second;
        zone.touch_share = given.touch_share;
        if (!set->add_zone(zone, given.frames)) {
            return -1;
        }
    }
    engine->sets[id] = std::move(set);
    engine->forgotten[id] = false;
    engine->last_sent[id] = 0;
    return id;
}

void jello_engine_forget_sample_set(JelloEngine* engine, int set) {
    if (!is_sendable_set(engine, set)) {
        return;
    }
    engine->forgotten[set] = true;
    jello_engine_tidy_sample_sets(engine);
}

void jello_engine_tidy_sample_sets(JelloEngine* engine) {
    // The count first: the sets in use read after it are at least as new.
    std::uint64_t handled = engine->conductor.sample_messages_handled();
    std::uint32_t in_use = engine->conductor.sets_in_use();
    for (int i = 0; i < sample_set_most; i++) {
        if (engine->sets[i] == nullptr) {
            continue;
        }
        if (!engine->forgotten[i]) {
            continue;
        }
        if (handled < engine->last_sent[i]) {
            continue;
        }
        if ((in_use & (1u << i)) != 0) {
            continue;
        }
        engine->sets[i].reset();
        engine->forgotten[i] = false;
    }
}

int jello_engine_sample_sets_kept(JelloEngine* engine) {
    int kept = 0;
    for (const auto& set : engine->sets) {
        if (set != nullptr) {
            kept += 1;
        }
    }
    return kept;
}

bool jello_engine_set_instrument(JelloEngine* engine, int track, int index, int set) {
    if (!is_valid_track(track)) {
        return false;
    }
    if (!is_within_int(index, 0, JELLO_OSCILLATOR_COUNT - 1)) {
        return false;
    }
    if (set != -1) {
        if (!is_sendable_set(engine, set)) {
            return false;
        }
    }
    if (!engine->conductor.set_instrument(track, index, set_to_send(engine, set))) {
        return false;
    }
    if (set != -1) {
        engine->last_sent[set] = engine->conductor.sample_messages_sent();
    }
    return true;
}

bool jello_engine_set_drum_kit(JelloEngine* engine, int track, int set) {
    if (!is_valid_track(track)) {
        return false;
    }
    if (set != -1) {
        if (!is_sendable_set(engine, set)) {
            return false;
        }
    }
    if (!engine->conductor.set_drum_kit(track, set_to_send(engine, set))) {
        return false;
    }
    if (set != -1) {
        engine->last_sent[set] = engine->conductor.sample_messages_sent();
    }
    return true;
}

void jello_engine_set_drum_pad(JelloEngine* engine, int track, int pad, float level, int half_steps, float pan,
                               int group) {
    if (!is_valid_track(track)) {
        return;
    }
    if (!is_within_int(pad, 0, JELLO_DRUM_PAD_COUNT - 1)) {
        return;
    }
    if (!is_within(level, 0.0f, 1.0f)) {
        return;
    }
    if (!is_within_int(half_steps, -JELLO_DRUM_MOST_HALF_STEPS, JELLO_DRUM_MOST_HALF_STEPS)) {
        return;
    }
    if (!is_within(pan, -1.0f, 1.0f)) {
        return;
    }
    if (!is_within_int(group, 0, JELLO_DRUM_MOST_GROUP)) {
        return;
    }
    DrumPad settings;
    settings.level = level;
    settings.half_steps = half_steps;
    settings.pan = pan;
    settings.group = group;
    engine->conductor.set_drum_pad(track, pad, settings);
}

}  // extern "C"
