#include "tracks.h"

#include <algorithm>

namespace {

// How long a track's volume takes to ease all the way from 0 to 1.
constexpr float volume_ease_seconds = 0.01f;

}  // namespace

Tracks::Tracks(int sample_rate)
    : synths_(count, Synth(sample_rate)), buffer_(2 * buffer_frames, 0.0f) {
    volumes_.fill(1.0f);
    heard_.fill(1.0f);
    volume_step_ = 1.0f / (volume_ease_seconds * static_cast<float>(sample_rate));
}

Synth& Tracks::synth(int track) {
    return synths_[track];
}

void Tracks::set_volume(int track, float volume) {
    volumes_[track] = volume;
}

void Tracks::all_notes_off(Source source) {
    for (Synth& synth : synths_) {
        synth.all_notes_off(source);
    }
}

void Tracks::set_beats_per_sample(double beats_per_sample) {
    for (Synth& synth : synths_) {
        synth.set_beats_per_sample(beats_per_sample);
    }
}

void Tracks::on_beat(std::uint64_t number, double beats_per_sample) {
    for (Synth& synth : synths_) {
        synth.on_beat(number, beats_per_sample);
    }
}

std::uint32_t Tracks::sets_in_use() const {
    std::uint32_t in_use = 0;
    for (const Synth& synth : synths_) {
        in_use |= synth.sets_in_use();
    }
    return in_use;
}

void Tracks::render(float* out, int frames) {
    // A piece at a time, as much as one track's buffer holds.
    int done = 0;
    while (done < frames) {
        int piece = std::min(buffer_frames, frames - done);
        float* piece_out = out + 2 * done;

        for (int track = 0; track < count; track++) {
            for (int i = 0; i < 2 * piece; i++) {
                buffer_[i] = 0.0f;
            }
            synths_[track].render(buffer_.data(), piece);

            // The volume heard eases a step each sample toward the one
            // asked for.
            float wanted = volumes_[track];
            float heard = heard_[track];
            for (int i = 0; i < piece; i++) {
                if (heard < wanted) {
                    heard = std::min(wanted, heard + volume_step_);
                } else if (heard > wanted) {
                    heard = std::max(wanted, heard - volume_step_);
                }
                piece_out[2 * i] += buffer_[2 * i] * heard;
                piece_out[2 * i + 1] += buffer_[2 * i + 1] * heard;
            }
            heard_[track] = heard;
        }
        done += piece;
    }
}
