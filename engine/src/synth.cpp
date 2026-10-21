#include "synth.h"

namespace {

// How many notes can sound at once.
constexpr int voice_count = 32;

}  // namespace

Synth::Synth(int sample_rate)
    : voices_(voice_count, Voice(sample_rate)) {
    layer_volumes_.fill(1.0f);
}

void Synth::note_on(int note, int velocity, Source source) {
    start_note(note, velocity, source, 0);
}

void Synth::note_off(int note, Source source) {
    // A note never has more than one voice for each source, since pressing
    // it again reuses the one it has. If it has none, there is nothing to
    // let go.
    Voice* voice = find_ringing_voice(note, source, 0);
    if (voice != nullptr) {
        voice->release();
    }
}

void Synth::all_notes_off(Source source) {
    for (Voice& voice : voices_) {
        if (voice.source() == source) {
            voice.release();
        }
    }
}

void Synth::loop_note_on(int note, int velocity, int layer) {
    start_note(note, velocity, Source::loop, layer);
}

void Synth::loop_note_off(int note, int layer) {
    // Like note_off: a layer never has more than one voice for a note.
    Voice* voice = find_ringing_voice(note, Source::loop, layer);
    if (voice != nullptr) {
        voice->release();
    }
}

void Synth::layer_notes_off(int layer) {
    for (Voice& voice : voices_) {
        if (voice.source() != Source::loop) {
            continue;
        }
        if (voice.layer() == layer) {
            voice.release();
        }
    }
}

void Synth::set_layer_volume(int layer, float volume) {
    layer_volumes_[layer] = volume;
}

void Synth::start_note(int note, int velocity, Source source, int layer) {
    // A note that is still ringing keeps its own voice, so it never sounds
    // twice. Otherwise it takes a free voice. When every voice is busy, it
    // takes over the quietest voice fading out, since that one is nearly gone
    // anyway, or else the note held the longest.
    Voice* voice = find_ringing_voice(note, source, layer);
    if (voice == nullptr) {
        voice = find_free_voice();
    }
    if (voice == nullptr) {
        voice = find_quietest_fading_voice();
    }
    if (voice == nullptr) {
        voice = find_oldest_voice();
    }

    // Every press gets the next number, even one that reuses its voice.
    press_count_ += 1;
    float volume = 1.0f;
    if (source == Source::loop) {
        volume = layer_volumes_[layer];
    }
    voice->start(note, velocity, press_count_, wave_, source, layer, volume);
}

void Synth::set_wave(Wave wave) {
    // Only notes started from here on play it. Each voice keeps the wave
    // its note started with.
    wave_ = wave;
}

void Synth::render(float* out, int frames) {
    for (Voice& voice : voices_) {
        voice.render(out, frames, volume_for(voice));
    }
}

Voice* Synth::find_ringing_voice(int note, Source source, int layer) {
    for (Voice& voice : voices_) {
        if (voice.is_free()) {
            continue;
        }
        if (voice.note() != note) {
            continue;
        }
        if (voice.source() != source) {
            continue;
        }
        if (voice.layer() == layer) {
            return &voice;
        }
    }
    return nullptr;
}

float Synth::volume_for(const Voice& voice) const {
    if (voice.source() != Source::loop) {
        return 1.0f;
    }
    return layer_volumes_[voice.layer()];
}

Voice* Synth::find_free_voice() {
    for (Voice& voice : voices_) {
        if (voice.is_free()) {
            return &voice;
        }
    }
    return nullptr;
}

Voice* Synth::find_quietest_fading_voice() {
    Voice* quietest = nullptr;
    for (Voice& voice : voices_) {
        if (!voice.is_releasing()) {
            continue;
        }
        if (quietest == nullptr) {
            quietest = &voice;
        } else if (voice.level() < quietest->level()) {
            quietest = &voice;
        }
    }
    return quietest;
}

Voice* Synth::find_oldest_voice() {
    // The smallest press number was pressed longest ago.
    Voice* oldest = &voices_[0];
    for (Voice& voice : voices_) {
        if (voice.press_number() < oldest->press_number()) {
            oldest = &voice;
        }
    }
    return oldest;
}
