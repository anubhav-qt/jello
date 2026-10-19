// The C API functions (include/jello_engine.h) and the audio device.
// All talking to the speakers happens in this file, so adding another audio
// system later (like ASIO on Windows) stays in one place.

#include "jello_engine.h"

#include <new>

#include "miniaudio_settings.h"
#include "synth.h"

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

}  // namespace

// Everything one engine owns.
struct JelloEngine {
    Synth synth{sample_rate};
    ma_device device;
    bool audio_running = false;
};

namespace {

// miniaudio calls this on the audio thread every few milliseconds, whenever
// the speakers need more sound. It must never lock, allocate, log or wait.
void on_audio(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)input;  // playback only, no microphone

    JelloEngine* engine = static_cast<JelloEngine*>(device->pUserData);
    engine->synth.render(static_cast<float*>(output), static_cast<int>(frames));
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
    engine->synth.note_on(note, velocity);
}

void jello_engine_note_off(JelloEngine* engine, int note) {
    if (!is_valid_note(note)) {
        return;
    }
    engine->synth.note_off(note);
}

void jello_engine_all_notes_off(JelloEngine* engine) {
    engine->synth.all_notes_off();
}

int jello_engine_sample_rate(JelloEngine* engine) {
    (void)engine;  // every engine runs at the same rate, for now
    return sample_rate;
}

void jello_engine_render(JelloEngine* engine, float* out, int frames) {
    engine->synth.render(out, frames);
}

}  // extern "C"
