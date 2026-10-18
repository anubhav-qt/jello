// The C API functions (include/jello_engine.h) and the audio device.
// All talking to the speakers happens in this file, so adding another audio
// system later (like ASIO on Windows) stays in one place.

#include "jello_engine.h"

#include <new>

#include "miniaudio_settings.h"
#include "tone.h"

namespace {

constexpr int sample_rate = 48000;
constexpr int channels = 2;  // stereo

}  // namespace

// Everything one engine owns.
struct JelloEngine {
    Tone tone{sample_rate};
    ma_device device;
    bool audio_running = false;
};

namespace {

// miniaudio calls this on the audio thread every few milliseconds, whenever
// the speakers need more sound. It must never lock, allocate, log or wait.
void on_audio(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)input;  // playback only, no microphone

    JelloEngine* engine = static_cast<JelloEngine*>(device->pUserData);
    engine->tone.render(static_cast<float*>(output), static_cast<int>(frames));
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

void jello_engine_set_tone(JelloEngine* engine, bool on) {
    engine->tone.set_on(on);
}

int jello_engine_sample_rate(JelloEngine* engine) {
    (void)engine;  // every engine runs at the same rate, for now
    return sample_rate;
}

void jello_engine_render(JelloEngine* engine, float* out, int frames) {
    engine->tone.render(out, frames);
}

}  // extern "C"
