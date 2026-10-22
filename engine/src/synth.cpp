#include "synth.h"

#include <algorithm>

namespace {

// How many notes can sound at once.
constexpr int voice_count = 32;

}  // namespace

// Every layer slot is a player of its own, after live play and the song.
static_assert(player_count == first_layer_player + Synth::layer_count, "every layer slot has a player");

Synth::Synth(int sample_rate)
    : voices_(voice_count, Voice(sample_rate)), routing_(sample_rate) {
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

    // Its vibrato keys go up too: every layer's, for the loop.
    if (source == Source::loop) {
        for (int layer = 0; layer < layer_count; layer++) {
            routing_.set_vibrato_key(player_of(source, layer), false);
        }
    } else {
        routing_.set_vibrato_key(player_of(source, 0), false);
    }
}

void Synth::loop_note_on(int note, int velocity, int layer) {
    if (note == vibrato_key_note) {
        routing_.set_vibrato_key(player_of(Source::loop, layer), true);
        return;
    }
    start_note(note, velocity, Source::loop, layer);
}

void Synth::loop_note_off(int note, int layer) {
    if (note == vibrato_key_note) {
        routing_.set_vibrato_key(player_of(Source::loop, layer), false);
        return;
    }

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
    routing_.set_vibrato_key(player_of(Source::loop, layer), false);
}

void Synth::set_vibrato_key(bool held) {
    routing_.set_vibrato_key(live_player, held);
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
    // Its player counts it as a key press too, the notes of a chord as one.
    press_count_ += 1;
    NoteStart start;
    start.source = source;
    start.layer = layer;
    start.press_number = press_count_;
    start.press = routing_.press(player_of(source, layer), now_);
    start.sample = now_;

    float volume = 1.0f;
    if (source == Source::loop) {
        volume = layer_volumes_[layer];
    }
    voice->start(note, velocity, start, volume, filter_targets_, note_sound_, routing_);
}

void Synth::set_wave(Wave wave) {
    // Only notes started from here on play it. Each voice keeps the wave
    // its note started with.
    note_sound_.wave = wave;
}

void Synth::set_filter(const FilterSettings& settings) {
    filter_settings_ = settings;
    update_filter_targets();
}

void Synth::set_filter_envelope(const FilterEnvelopeSettings& envelope) {
    note_sound_.filter_envelope = envelope;
}

void Synth::set_unison(const UnisonSettings& unison) {
    // Like the wave, only notes started from here on hear it. Each voice
    // keeps the copies its note started with.
    note_sound_.unison = unison;
}

void Synth::set_loudness_envelope(const LoudnessEnvelopeSettings& envelope) {
    note_sound_.loudness_envelope = envelope;
}

void Synth::set_key_tracking(float key_tracking) {
    note_sound_.key_tracking = key_tracking;
}

void Synth::set_tuning(float a4_hz) {
    note_sound_.a4_hz = a4_hz;
}

void Synth::set_lfo(int index, const LfoSettings& settings) {
    routing_.set_lfo(index, settings);
}

void Synth::set_connection(int index, const Connection& connection) {
    routing_.set_connection(index, connection);
    update_filter_targets();
}

void Synth::set_vibrato_key_times(int rise_ms, int fall_ms) {
    routing_.set_vibrato_key_times(rise_ms, fall_ms);
}

void Synth::set_beats_per_sample(double beats_per_sample) {
    routing_.set_beats_per_sample(beats_per_sample);
}

void Synth::on_beat(std::uint64_t number, double beats_per_sample) {
    routing_.on_beat(number, beats_per_sample, now_);
}

void Synth::restart_song_presses() {
    routing_.restart_presses(song_player);
}

void Synth::render(float* out, int frames) {
    // In blocks that end where the next control_samples samples start,
    // counted from the first sample, so the routing list is always worked
    // out on the same samples however the sound is split into chunks.
    int done = 0;
    while (done < frames) {
        int into_block = static_cast<int>(now_ % control_samples);
        if (into_block == 0) {
            tick();
        }
        int count = std::min(control_samples - into_block, frames - done);

        for (Voice& voice : voices_) {
            voice.render(out + 2 * done, count, volume_for(voice), filter_targets_);
        }
        done += count;
        now_ += count;
    }
}

void Synth::tick() {
    std::uint64_t block_end = now_ + control_samples;
    routing_.tick(block_end);
    for (Voice& voice : voices_) {
        if (!voice.is_free()) {
            voice.tick(routing_, block_end);
        }
    }
}

void Synth::update_filter_targets() {
    FilterTargets targets = filter_targets_of(filter_settings_);
    if (routing_.moves_cutoff()) {
        targets.share = 1.0;
    }
    filter_targets_ = targets;
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
