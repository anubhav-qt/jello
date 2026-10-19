#include "synth.h"

#include <cmath>

namespace {

// How many notes can sound at once.
constexpr int voice_count = 32;

// The soft clip leaves samples between -0.8 and 0.8 exactly as they are.
constexpr float clip_start = 0.8f;

// The room between clip_start and 1.0, the most speakers can play.
constexpr float clip_room = 1.0f - clip_start;

// Keeps the voices' added-up sound below 1.0, so it never clips (gets cut
// flat at the speakers' limit, which sounds harsh and crackly). Quiet samples
// pass through untouched. Louder ones are squeezed into the room above
// clip_start, harder the further they go, so they get ever closer to 1.0
// without reaching it: 1.0 comes out as 0.9, 1.4 as 0.95, and even all 32
// voices at their loudest, 11.2, only reach about 0.996. The master limiter
// takes over this job in step 6.
float soft_clip(float sample) {
    // How far the sample is from zero, whether above or below it.
    float size = std::fabs(sample);
    if (size <= clip_start) {
        return sample;
    }

    // How much of the room the sample gets: none right at clip_start, then
    // more and more the further past it the sample goes, but never all of it.
    float over = size - clip_start;
    float share = over / (over + clip_room);
    float squeezed = clip_start + share * clip_room;

    // Below zero it works the same way, mirrored.
    if (sample < 0.0f) {
        return -squeezed;
    } else {
        return squeezed;
    }
}

}  // namespace

Synth::Synth(int sample_rate)
    : voices_(voice_count, Voice(sample_rate)) {}

void Synth::note_on(int note, int velocity) {
    mailbox_.send(Event{EventType::note_on, note, velocity});
}

void Synth::note_off(int note) {
    mailbox_.send(Event{EventType::note_off, note, 0});
}

void Synth::all_notes_off() {
    mailbox_.send(Event{EventType::all_notes_off, 0, 0});
}

void Synth::set_wave(Wave wave) {
    mailbox_.send(Event{EventType::set_wave, 0, 0, wave});
}

void Synth::render(float* out, int frames) {
    // Act on everything sent since the last chunk, oldest first.
    Event event{};
    while (mailbox_.receive(event)) {
        handle(event);
    }

    // Start from silence, since voices add themselves on top. Each frame is
    // two samples: left and right.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = 0.0f;
    }

    for (Voice& voice : voices_) {
        voice.render(out, frames);
    }

    // Many voices can add up to more than speakers can play.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = soft_clip(out[i]);
    }
}

void Synth::handle(const Event& event) {
    if (event.type == EventType::note_on) {
        play(event.note, event.velocity);
    } else if (event.type == EventType::note_off) {
        // A note never has more than one voice, since pressing it again
        // reuses the one it has. If it has none, there is nothing to let go.
        Voice* voice = find_ringing_voice(event.note);
        if (voice != nullptr) {
            voice->release();
        }
    } else if (event.type == EventType::all_notes_off) {
        for (Voice& voice : voices_) {
            voice.release();
        }
    } else if (event.type == EventType::set_wave) {
        // Only notes started from here on play it. Each voice keeps the wave
        // its note started with.
        wave_ = event.wave;
    }
}

void Synth::play(int note, int velocity) {
    // A note that is still ringing keeps its own voice, so it never sounds
    // twice. Otherwise it takes a free voice. When every voice is busy, it
    // takes over the quietest voice fading out, since that one is nearly gone
    // anyway, or else the note held the longest.
    Voice* voice = find_ringing_voice(note);
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
    voice->start(note, velocity, press_count_, wave_);
}

Voice* Synth::find_ringing_voice(int note) {
    for (Voice& voice : voices_) {
        if (!voice.is_free() && voice.note() == note) {
            return &voice;
        }
    }
    return nullptr;
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
