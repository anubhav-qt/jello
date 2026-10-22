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

// How long a filter envelope's stage takes, in ms.
bool is_valid_stage_ms(int ms) {
    if (ms < 0) {
        return false;
    }
    return ms <= JELLO_FILTER_LONGEST_MS;
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

void jello_engine_set_wave(JelloEngine* engine, int wave) {
    // Any number that isn't one of the four waves is ignored, so nothing odd
    // reaches the synth.
    if (wave == JELLO_WAVE_SINE) {
        engine->conductor.set_wave(Wave::sine);
    } else if (wave == JELLO_WAVE_TRIANGLE) {
        engine->conductor.set_wave(Wave::triangle);
    } else if (wave == JELLO_WAVE_SQUARE) {
        engine->conductor.set_wave(Wave::square);
    } else if (wave == JELLO_WAVE_SAW) {
        engine->conductor.set_wave(Wave::saw);
    }
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

void jello_engine_set_filter(JelloEngine* engine, float cutoff_hz, float resonance) {
    if (!is_within(cutoff_hz, JELLO_FILTER_LOWEST_HZ, JELLO_FILTER_OPEN_HZ)) {
        return;
    }
    if (!is_within(resonance, 0.0f, 1.0f)) {
        return;
    }

    FilterSettings settings;
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
