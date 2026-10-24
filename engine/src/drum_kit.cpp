#include "drum_kit.h"

#include <algorithm>
#include <cmath>

namespace {

// How loud one hit is at full velocity, before its pad's level. Like the
// synth's voice level, so a drum and a note sit together. A starting value,
// to be tuned by ear.
constexpr float drum_level = 0.5f;

// How long a choked hit takes to fade out: short enough to sound cut off,
// long enough not to click.
constexpr int choke_fade_ms = 5;

// How long a hit's layer volume takes to ease from silent to full, as a
// voice's does.
constexpr int volume_ease_ms = 10;

// How hard the key was hit, turned into loudness, as a voice does it:
// squared, so half the velocity gives a quarter of the loudness.
float velocity_to_peak(int velocity) {
    float amount = static_cast<float>(velocity) / 127.0f;
    return amount * amount;
}

}  // namespace

bool is_drum_note(int note) {
    if (note < drum_first_note) {
        return false;
    }
    return note < drum_first_note + drum_pad_count;
}

DrumKit::DrumKit(int sample_rate)
    : fade_samples_(sample_rate * choke_fade_ms / 1000),
      volume_step_(1000.0f / static_cast<float>(sample_rate * volume_ease_ms)) {}

void DrumKit::set_kit(const SampleSet* set) {
    kit_ = set;
}

void DrumKit::set_pad(int pad, const DrumPad& settings) {
    pads_[pad] = settings;
}

void DrumKit::hit(int pad, int velocity, Source source, int layer) {
    if (kit_ == nullptr) {
        return;
    }
    const SampleZone* zone = kit_->exact_zone_for(pad);
    if (zone == nullptr) {
        return;
    }
    const DrumPad& settings = pads_[pad];

    // Every other hit sounding in its choke group fades out.
    if (settings.group != 0) {
        for (Hit& other : hits_) {
            if (!other.sounding) {
                continue;
            }
            if (other.group != settings.group) {
                continue;
            }
            if (other.fade_left < 0) {
                other.fade_left = fade_samples_;
            }
        }
    }

    Hit& hit = free_hit();
    hit_count_ += 1;
    hit.sounding = true;
    hit.pad = pad;
    hit.group = settings.group;
    hit.source = source;
    hit.layer = layer;
    hit.set = kit_;
    hit.zone = zone;
    hit.place = 0.0;
    hit.step = std::exp2(settings.half_steps / 12.0);
    hit.fade_left = -1;
    hit.number = hit_count_;

    // The same square-root pan as a unison copy's: exactly 1 on each side
    // in the middle.
    float loudness = velocity_to_peak(velocity) * settings.level * drum_level;
    hit.left = loudness * std::sqrt(1.0f - settings.pan);
    hit.right = loudness * std::sqrt(1.0f + settings.pan);
    hit.volume = -1.0f;
}

void DrumKit::render(float* out, int frames, const std::array<float, 8>& layer_volumes) {
    for (Hit& hit : hits_) {
        if (!hit.sounding) {
            continue;
        }
        float volume = volume_for(hit, layer_volumes);
        // A new hit starts right at its volume.
        if (hit.volume < 0.0f) {
            hit.volume = volume;
        }
        for (int i = 0; i < frames; i++) {
            // A hit plays once to its end, even from a zone with a loop.
            if (hit.place >= static_cast<double>(hit.zone->frame_count)) {
                hit.sounding = false;
                break;
            }
            if (hit.fade_left == 0) {
                hit.sounding = false;
                break;
            }

            if (hit.volume < volume) {
                hit.volume = std::min(hit.volume + volume_step_, volume);
            } else if (hit.volume > volume) {
                hit.volume = std::max(hit.volume - volume_step_, volume);
            }
            float fade = 1.0f;
            if (hit.fade_left > 0) {
                fade = static_cast<float>(hit.fade_left) / static_cast<float>(fade_samples_);
                hit.fade_left -= 1;
            }

            Stereo height = read_zone(*hit.zone, hit.place);
            float gain = hit.volume * fade;
            out[2 * i] += static_cast<float>(height.left) * hit.left * gain;
            out[2 * i + 1] += static_cast<float>(height.right) * hit.right * gain;
            hit.place += hit.step;
        }
    }
}

void DrumKit::add_sets_in_use(std::uint32_t& sets) const {
    if (kit_ != nullptr) {
        sets |= 1u << kit_->id();
    }
    for (const Hit& hit : hits_) {
        if (hit.sounding) {
            sets |= 1u << hit.set->id();
        }
    }
}

DrumKit::Hit& DrumKit::free_hit() {
    Hit* oldest = &hits_[0];
    for (Hit& hit : hits_) {
        if (!hit.sounding) {
            return hit;
        }
        if (hit.number < oldest->number) {
            oldest = &hit;
        }
    }
    return *oldest;
}

float DrumKit::volume_for(const Hit& hit, const std::array<float, 8>& layer_volumes) {
    if (hit.source == Source::loop) {
        return layer_volumes[hit.layer];
    }
    return 1.0f;
}
