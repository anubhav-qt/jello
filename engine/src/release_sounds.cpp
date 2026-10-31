#include "release_sounds.h"

namespace {

// How long a pedal's last sound takes to fade out under its next one.
constexpr int pedal_fade_ms = 100;

}  // namespace

ReleaseSounds::ReleaseSounds(int sample_rate) : fade_samples_(sample_rate * pedal_fade_ms / 1000) {}

void ReleaseSounds::play(const SampleSet* set, const SampleZone* zone, double step, float gain, Source source,
                         int layer) {
    Sound& sound = free_sound();
    started_ += 1;
    sound.sounding = true;
    sound.from_pedal = false;
    sound.source = source;
    sound.layer = layer;
    sound.set = set;
    sound.zone = zone;
    sound.place = 0.0;
    sound.step = step;
    sound.gain = gain;
    sound.fade_left = -1;
    sound.number = started_;
}

void ReleaseSounds::play_pedal(const SampleSet* set, const SampleZone* zone, float gain, Source source, int layer) {
    for (Sound& other : sounds_) {
        if (!other.sounding || !other.from_pedal) {
            continue;
        }
        if (other.source != source || other.layer != layer) {
            continue;
        }
        if (other.fade_left < 0) {
            other.fade_left = fade_samples_;
        }
    }
    play(set, zone, 1.0, gain, source, layer);
    // The sound just started is the newest of all.
    for (Sound& sound : sounds_) {
        if (sound.number == started_) {
            sound.from_pedal = true;
        }
    }
}

void ReleaseSounds::render(float* out, int frames, const std::array<float, 8>& layer_volumes) {
    for (Sound& sound : sounds_) {
        if (!sound.sounding) {
            continue;
        }
        float volume = 1.0f;
        if (sound.source == Source::loop) {
            volume = layer_volumes[sound.layer];
        }
        for (int i = 0; i < frames; i++) {
            if (sound.place >= static_cast<double>(sound.zone->frame_count)) {
                sound.sounding = false;
                break;
            }
            if (sound.fade_left == 0) {
                sound.sounding = false;
                break;
            }
            float fade = 1.0f;
            if (sound.fade_left > 0) {
                fade = static_cast<float>(sound.fade_left) / static_cast<float>(fade_samples_);
                sound.fade_left -= 1;
            }

            Stereo height = read_zone(*sound.zone, sound.place);
            float gain = sound.gain * volume * fade;
            out[2 * i] += static_cast<float>(height.left) * gain;
            out[2 * i + 1] += static_cast<float>(height.right) * gain;
            sound.place += sound.step;
        }
    }
}

void ReleaseSounds::add_sets_in_use(std::uint32_t& sets) const {
    for (const Sound& sound : sounds_) {
        if (sound.sounding) {
            sets |= 1u << sound.set->id();
        }
    }
}

ReleaseSounds::Sound& ReleaseSounds::free_sound() {
    Sound* oldest = &sounds_[0];
    for (Sound& sound : sounds_) {
        if (!sound.sounding) {
            return sound;
        }
        if (sound.number < oldest->number) {
            oldest = &sound;
        }
    }
    return *oldest;
}
