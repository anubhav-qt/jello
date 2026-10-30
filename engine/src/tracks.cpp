#include "tracks.h"

Tracks::Tracks(int sample_rate) : synths_(count, Synth(sample_rate)) {}

Synth& Tracks::synth(int track) {
    return synths_[track];
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
