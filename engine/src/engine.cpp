// The C API functions (include/jello_engine.h) and the audio device.
// All talking to the speakers happens in this file, so adding another audio
// system later (like ASIO on Windows) stays in one place.

#include "jello_engine.h"

#include <array>
#include <cmath>
#include <cstdint>
#include <new>
#include <vector>

#include "miniaudio_settings.h"
#include "conductor.h"

namespace {

constexpr int sample_rate = 48000;
constexpr int channels = 2;  // stereo

// MIDI numbers notes from 0 to 127. Middle C is 60.
bool is_valid_note(int note) {
    return note >= 0 && note <= 127;
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

// A layer's number for the C API: from 0, below JELLO_LOOP_MAX_LAYERS.
bool is_valid_layer(int layer) {
    if (layer < 0) {
        return false;
    }
    return layer < JELLO_LOOP_MAX_LAYERS;
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
static_assert(JELLO_KIND_NOISE + 1 == kind_count, "every kind has a number");
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
static_assert(JELLO_TARGET_GLIDE_TIME + 1 == target_count, "every target has a number");

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
    if (api.target > JELLO_TARGET_GLIDE_TIME) {
        return false;
    }

    RoutingTarget target = static_cast<RoutingTarget>(api.target);
    float most = most_amount(target);
    if (!is_within(api.amount, -most, most)) {
        return false;
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
    }
    return JELLO_LOOP_FORGOT;
}

}  // namespace

// Everything one engine owns.
struct JelloEngine {
    Conductor conductor{sample_rate};
    ma_device device;
    bool audio_running = false;
};

namespace {

// miniaudio calls this on the audio thread every few milliseconds, whenever
// the speakers need more sound. It must never lock, allocate, log or wait.
void on_audio(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)input;  // playback only, no microphone

    JelloEngine* engine = static_cast<JelloEngine*>(device->pUserData);
    engine->conductor.render(static_cast<float*>(output), static_cast<int>(frames));
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

void jello_engine_note_on(JelloEngine* engine, int note, int velocity) {
    // Ignore anything MIDI could never send, so nothing odd reaches the synth.
    if (!is_valid_note(note)) {
        return;
    }
    if (!is_valid_velocity(velocity)) {
        return;
    }
    engine->conductor.note_on(note, velocity);
}

void jello_engine_note_off(JelloEngine* engine, int note) {
    if (!is_valid_note(note)) {
        return;
    }
    engine->conductor.note_off(note);
}

void jello_engine_all_notes_off(JelloEngine* engine) {
    engine->conductor.all_notes_off();
}

void jello_engine_set_oscillator(JelloEngine* engine, int index, int kind, float level, int octaves, int half_steps,
                                 float cents, float width) {
    // Anything out of range is ignored, so nothing odd reaches the synth.
    if (!is_within_int(index, 0, JELLO_OSCILLATOR_COUNT - 1)) {
        return;
    }
    if (!is_within_int(kind, JELLO_KIND_SINE, JELLO_KIND_NOISE)) {
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

    OscillatorSettings oscillator;
    oscillator.kind = static_cast<Kind>(kind);
    oscillator.level = level;
    oscillator.octaves = octaves;
    oscillator.half_steps = half_steps;
    oscillator.cents = cents;
    oscillator.width = width;
    engine->conductor.set_oscillator(index, oscillator);
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

void jello_engine_press_looper(JelloEngine* engine) {
    engine->conductor.press_looper();
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

void jello_engine_set_snap(JelloEngine* engine, bool snap) {
    engine->conductor.set_snap(snap);
}

bool jello_engine_play_song(JelloEngine* engine, const JelloSongChord* chords, int count) {
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
    return engine->conductor.play_song(song.data(), count);
}

void jello_engine_stop_song(JelloEngine* engine) {
    engine->conductor.stop_song();
}

void jello_engine_set_filter(JelloEngine* engine, int type, float cutoff_hz, float resonance) {
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
    engine->conductor.set_filter(settings);
}

void jello_engine_set_filter_envelope(JelloEngine* engine, float amount, int attack_ms, int decay_ms, float sustain,
                                      int release_ms) {
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
    engine->conductor.set_filter_envelope(envelope);
}

void jello_engine_set_unison(JelloEngine* engine, int copies, float detune_cents, float spread) {
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
    engine->conductor.set_unison(unison);
}

void jello_engine_set_loudness_envelope(JelloEngine* engine, int attack_ms, int decay_ms, float sustain,
                                        int release_ms) {
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
    engine->conductor.set_loudness_envelope(envelope);
}

void jello_engine_set_key_tracking(JelloEngine* engine, float key_tracking) {
    if (!is_within(key_tracking, 0.0f, 1.0f)) {
        return;
    }
    engine->conductor.set_key_tracking(key_tracking);
}

void jello_engine_set_tuning(JelloEngine* engine, float a4_hz) {
    if (!is_within(a4_hz, JELLO_TUNING_LOWEST_HZ, JELLO_TUNING_HIGHEST_HZ)) {
        return;
    }
    engine->conductor.set_tuning(a4_hz);
}

void jello_engine_set_lfo(JelloEngine* engine, int index, const JelloLfo* lfo) {
    if (!is_within_int(index, 0, JELLO_LFO_COUNT - 1)) {
        return;
    }
    LfoSettings settings;
    if (!lfo_of_api(*lfo, settings)) {
        return;
    }
    engine->conductor.set_lfo(index, settings);
}

void jello_engine_set_connection(JelloEngine* engine, int index, const JelloConnection* connection) {
    if (!is_within_int(index, 0, JELLO_CONNECTION_COUNT - 1)) {
        return;
    }
    Connection checked;
    if (!connection_of_api(*connection, checked)) {
        return;
    }
    engine->conductor.set_connection(index, checked);
}

void jello_engine_set_vibrato_key_times(JelloEngine* engine, int rise_ms, int fall_ms) {
    if (!is_within_int(rise_ms, 0, JELLO_VIBRATO_KEY_LONGEST_MS)) {
        return;
    }
    if (!is_within_int(fall_ms, 0, JELLO_VIBRATO_KEY_LONGEST_MS)) {
        return;
    }
    engine->conductor.set_vibrato_key_times(rise_ms, fall_ms);
}

void jello_engine_set_vibrato_key(JelloEngine* engine, bool held) {
    engine->conductor.set_vibrato_key(held);
}

void jello_engine_set_glide_key(JelloEngine* engine, bool held) {
    engine->conductor.set_glide_key(held);
}

void jello_engine_set_glide_time(JelloEngine* engine, int glide_ms) {
    if (!is_within_int(glide_ms, 0, JELLO_GLIDE_LONGEST_MS)) {
        return;
    }
    engine->conductor.set_glide_time(glide_ms);
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
    return true;
}

bool jello_engine_plan_loop(JelloEngine* engine, const JelloLoopNote* played, int count, int bars, int tempo,
                            bool snap, JelloLoopNote* planned) {
    (void)engine;  // every engine runs at the same rate, for now
    if (count < 0) {
        return false;
    }
    if (count > JELLO_LOOP_MAX_NOTES) {
        return false;
    }
    if (bars < 1) {
        return false;
    }
    if (!is_valid_tempo(tempo)) {
        return false;
    }

    // The looper's own copies of the notes. This runs on the calling
    // thread, not the audio thread, so it may ask for memory.
    std::vector<LoopNote> notes(static_cast<std::size_t>(count));
    for (int i = 0; i < count; i++) {
        if (!is_valid_note(played[i].note)) {
            return false;
        }
        notes[i].note = played[i].note;
        notes[i].velocity = 0;  // not needed to plan
        notes[i].start = played[i].start;
        notes[i].end = played[i].end;
    }

    BeatLength beat;
    beat.samples_per_minute = static_cast<std::uint64_t>(sample_rate) * 60;
    beat.tempo = static_cast<std::uint64_t>(tempo);
    std::uint64_t whole_bars = static_cast<std::uint64_t>(bars);
    std::uint64_t length = whole_bars * beats_per_bar * beat.samples_per_minute / beat.tempo;

    std::vector<LoopNote> plan(static_cast<std::size_t>(count));
    plan_loop(notes.data(), count, length, beat, snap, plan.data());
    for (int i = 0; i < count; i++) {
        planned[i].note = plan[i].note;
        planned[i].start = plan[i].start;
        planned[i].end = plan[i].end;
    }
    return true;
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

void jello_engine_render(JelloEngine* engine, float* out, int frames) {
    engine->conductor.render(out, frames);
}

}  // extern "C"
