#include "synth.h"

#include <algorithm>

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
    let_go(note, source, 0);
}

void Synth::all_notes_off(Source source) {
    for (Voice& voice : voices_) {
        if (voice.source() == source) {
            voice.release();
        }
    }

    // Its vibrato and glide keys go up too, and its notes still waiting
    // never start: every layer's, for the loop.
    if (source == Source::loop) {
        for (int layer = 0; layer < layer_count; layer++) {
            int player = player_of(source, layer);
            routing_.set_vibrato_key(player, false);
            forget_melody(player);
        }
    } else {
        int player = player_of(source, 0);
        routing_.set_vibrato_key(player, false);
        forget_melody(player);
    }
}

void Synth::loop_note_on(int note, int velocity, int layer) {
    int player = player_of(Source::loop, layer);
    if (note == vibrato_key_note) {
        routing_.set_vibrato_key(player, true);
        return;
    }
    if (note == glide_key_note) {
        melodies_[player].glide_key = true;
        return;
    }
    start_note(note, velocity, Source::loop, layer);
}

void Synth::loop_note_off(int note, int layer) {
    int player = player_of(Source::loop, layer);
    if (note == vibrato_key_note) {
        routing_.set_vibrato_key(player, false);
        return;
    }
    if (note == glide_key_note) {
        melodies_[player].glide_key = false;
        return;
    }
    let_go(note, Source::loop, layer);
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
    int player = player_of(Source::loop, layer);
    routing_.set_vibrato_key(player, false);
    forget_melody(player);
}

void Synth::set_vibrato_key(bool held) {
    routing_.set_vibrato_key(live_player, held);
}

void Synth::set_glide_key(bool held) {
    melodies_[live_player].glide_key = held;
}

void Synth::set_glide_time(int glide_ms) {
    note_sound_.glide_ms = glide_ms;
}

void Synth::set_layer_volume(int layer, float volume) {
    layer_volumes_[layer] = volume;
}

void Synth::start_note(int note, int velocity, Source source, int layer) {
    // While its player holds the glide key, a note waits until every note
    // of its moment has come, so settle can tell a melody note from a
    // chord. It keeps how notes sound now. Once every voice's worth of
    // notes is waiting, any more start right away.
    int player = player_of(source, layer);
    bool waits = melodies_[player].glide_key;
    bool has_room = waiting_count_ < voice_count;
    if (waits && has_room) {
        Waiting& waiting = waiting_[waiting_count_];
        waiting.note = note;
        waiting.velocity = velocity;
        waiting.source = source;
        waiting.layer = layer;
        waiting.sound = note_sound_;
        waiting.let_go = false;
        waiting_count_ += 1;
        return;
    }

    // Kept, so settle knows which notes of the moment came on their own.
    Voice* voice = play_note(note, velocity, source, layer, note_sound_);
    if (started_count_ < voice_count) {
        started_[started_count_] = static_cast<int>(voice - voices_.data());
        started_count_ += 1;
    }
}

Voice* Synth::play_note(int note, int velocity, Source source, int layer, const NoteSound& sound) {
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
    voice->start(note, velocity, start, volume, filter_targets_, sound, routing_);
    return voice;
}

void Synth::settle() {
    if (waiting_count_ == 0 && started_count_ == 0) {
        return;
    }

    for (int player = 0; player < player_count; player++) {
        int first_started = 0;
        int first_waiting = 0;
        int started = started_by(player, first_started);
        int waiting = waiting_by(player, first_waiting);
        int notes = started + waiting;

        if (notes == 1) {
            // A note on its own is a melody note. One that waited glides.
            if (waiting == 1) {
                glide(player, waiting_[first_waiting]);
            } else {
                melodies_[player].taken_count = 0;
                remember_melody(player, voices_[started_[first_started]]);
            }
        } else {
            // A chord, or no note at all: its waiting notes start as any
            // note does, and the melody stays as it was.
            for (int w = 0; w < waiting_count_; w++) {
                const Waiting& note = waiting_[w];
                if (player_of(note.source, note.layer) == player) {
                    start_waiting(note);
                }
            }
        }
    }

    waiting_count_ = 0;
    started_count_ = 0;
}

int Synth::started_by(int player, int& first) const {
    int count = 0;
    for (int s = 0; s < started_count_; s++) {
        const Voice& voice = voices_[started_[s]];
        if (player_of(voice.source(), voice.layer()) != player) {
            continue;
        }
        if (count == 0) {
            first = s;
        }
        count += 1;
    }
    return count;
}

int Synth::waiting_by(int player, int& first) const {
    int count = 0;
    for (int w = 0; w < waiting_count_; w++) {
        const Waiting& waiting = waiting_[w];
        if (player_of(waiting.source, waiting.layer) != player) {
            continue;
        }
        if (count == 0) {
            first = w;
        }
        count += 1;
    }
    return count;
}

void Synth::glide(int player, const Waiting& waiting) {
    Melody& melody = melodies_[player];
    Voice* last = melody_voice(melody);
    Voice* ringing = find_ringing_voice(waiting.note, waiting.source, waiting.layer);

    bool last_held = false;
    if (last != nullptr) {
        last_held = !last->is_releasing();
    }

    // The last melody key is still held, so its voice bends to the new
    // note, as one note, and keeps the note it bent away from to come back
    // to. Not when the new note still rings on a voice of its own, so a
    // note never sounds on two voices. A bend counts as a key press, so the
    // notes after it count it, though the voice's LFOs carry on.
    if (last_held && ringing == nullptr) {
        if (melody.taken_count < most_taken) {
            melody.taken[melody.taken_count] = last->note();
            melody.taken_count += 1;
        }
        press_count_ += 1;
        routing_.press(player, now_);
        last->bend_to(waiting.note, press_count_, now_, waiting.sound.a4_hz, waiting.sound.glide_ms, routing_);
        remember_melody(player, *last);
        if (waiting.let_go) {
            let_go_now(waiting.note, waiting.source, waiting.layer);
        }
        return;
    }

    // Otherwise the note starts as any note does, and its pitch slides in
    // from the last melody note's: from wherever it is along its own slide
    // if it still sounds, or from its pitch if it has fallen silent. The
    // very first melody note has nothing to slide from.
    bool slides = melody.has_last;
    double from_hz = melody.last_hz;
    if (last != nullptr) {
        from_hz = last->pitch_at(now_);
    }

    Voice* voice = play_note(waiting.note, waiting.velocity, waiting.source, waiting.layer, waiting.sound);
    if (slides) {
        voice->slide_in(from_hz, waiting.sound.glide_ms, routing_);
    }
    melody.taken_count = 0;
    remember_melody(player, *voice);
    if (waiting.let_go) {
        let_go_now(waiting.note, waiting.source, waiting.layer);
    }
}

void Synth::start_waiting(const Waiting& waiting) {
    play_note(waiting.note, waiting.velocity, waiting.source, waiting.layer, waiting.sound);
    if (waiting.let_go) {
        let_go_now(waiting.note, waiting.source, waiting.layer);
    }
}

Voice* Synth::melody_voice(const Melody& melody) {
    if (!melody.has_last) {
        return nullptr;
    }

    // A voice taken over by another press has a new press number.
    Voice& voice = voices_[melody.last_voice];
    if (voice.is_free()) {
        return nullptr;
    }
    if (voice.press_number() != melody.last_press) {
        return nullptr;
    }
    return &voice;
}

void Synth::remember_melody(int player, const Voice& voice) {
    Melody& melody = melodies_[player];
    melody.has_last = true;
    melody.last_voice = static_cast<int>(&voice - voices_.data());
    melody.last_press = voice.press_number();
    melody.last_hz = voice.note_hz();
}

void Synth::let_go(int note, Source source, int layer) {
    // A note still waiting is let go as soon as it starts: its newest
    // press, if it was pressed more than once this moment.
    for (int w = waiting_count_ - 1; w >= 0; w--) {
        Waiting& waiting = waiting_[w];
        if (waiting.let_go) {
            continue;
        }
        if (waiting.note != note) {
            continue;
        }
        if (waiting.source != source) {
            continue;
        }
        if (waiting.layer == layer) {
            waiting.let_go = true;
            return;
        }
    }
    let_go_now(note, source, layer);
}

void Synth::let_go_now(int note, Source source, int layer) {
    int player = player_of(source, layer);
    Melody& melody = melodies_[player];

    // A note the melody's voice bent away from no longer comes back.
    int kept = 0;
    for (int t = 0; t < melody.taken_count; t++) {
        if (melody.taken[t] != note) {
            melody.taken[kept] = melody.taken[t];
            kept += 1;
        }
    }
    melody.taken_count = kept;

    // A note never has more than one voice for each player, since pressing
    // it again reuses the one it has. If it has none, there is nothing to
    // let go.
    Voice* voice = find_ringing_voice(note, source, layer);
    if (voice == nullptr) {
        return;
    }

    // The melody's voice bends back to the newest note still held instead
    // of fading out, so a key held down always sounds. It glides there
    // while the glide key is held, and jumps once it is let go.
    bool bends_back = false;
    if (voice == melody_voice(melody)) {
        bends_back = melody.taken_count > 0;
    }
    if (!bends_back) {
        voice->release();
        return;
    }

    melody.taken_count -= 1;
    int back_to = melody.taken[melody.taken_count];
    int glide_ms = 0;
    if (melody.glide_key) {
        glide_ms = note_sound_.glide_ms;
    }
    voice->bend_to(back_to, voice->press_number(), now_, note_sound_.a4_hz, glide_ms, routing_);
    remember_melody(player, *voice);
}

void Synth::forget_melody(int player) {
    melodies_[player] = Melody{};

    int kept = 0;
    for (int w = 0; w < waiting_count_; w++) {
        if (player_of(waiting_[w].source, waiting_[w].layer) != player) {
            waiting_[kept] = waiting_[w];
            kept += 1;
        }
    }
    waiting_count_ = kept;

    kept = 0;
    for (int s = 0; s < started_count_; s++) {
        const Voice& voice = voices_[started_[s]];
        if (player_of(voice.source(), voice.layer()) != player) {
            started_[kept] = started_[s];
            kept += 1;
        }
    }
    started_count_ = kept;
}

void Synth::set_oscillator(int index, const OscillatorSettings& oscillator) {
    // Only notes started from here on play it. Each voice keeps the
    // oscillators its note started with.
    note_sound_.oscillators[index] = oscillator;
}

void Synth::set_filter(const FilterSettings& settings) {
    filter_settings_ = settings;
    update_filter_targets();
}

void Synth::set_filter_envelope(const FilterEnvelopeSettings& envelope) {
    note_sound_.filter_envelope = envelope;
}

void Synth::set_unison(const UnisonSettings& unison) {
    // Like the oscillators, only notes started from here on hear it. Each voice
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
    // Once sound is rendered past the moment, every note of it has come.
    // The conductor renders up to each note it passes on, so a render of no
    // frames leaves the moment open.
    if (frames > 0) {
        settle();
    }

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
