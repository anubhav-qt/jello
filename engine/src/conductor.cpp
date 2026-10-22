#include "conductor.h"

#include <cmath>

namespace {

// The soft clip leaves samples between -0.8 and 0.8 exactly as they are.
constexpr float clip_start = 0.8f;

// The room between clip_start and 1.0, the most speakers can play.
constexpr float clip_room = 1.0f - clip_start;

// Keeps the added-up sound below 1.0, so it never clips (gets cut flat at
// the speakers' limit, which sounds harsh and crackly). Quiet samples pass
// through untouched. Louder ones are squeezed into the room above
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

// How many beats pass each sample at the tempo `beat` says.
double beats_per_sample(BeatLength beat) {
    return static_cast<double>(beat.tempo) / static_cast<double>(beat.samples_per_minute);
}

// How hard the vibrato key is pressed when the looper records it. Only that
// it is held matters, so any velocity would do.
constexpr int vibrato_key_velocity = 127;

}  // namespace

Conductor::Conductor(int sample_rate)
    : synth_(sample_rate), clock_(sample_rate), click_(sample_rate), looper_(loop_changes_) {
    synth_.set_beats_per_sample(beats_per_sample(clock_.asked_beat_length()));
}

void Conductor::note_on(int note, int velocity) {
    mailbox_.send(Event{EventType::note_on, note, velocity});
}

void Conductor::note_off(int note) {
    mailbox_.send(Event{EventType::note_off, note, 0});
}

void Conductor::all_notes_off() {
    mailbox_.send(Event{EventType::all_notes_off, 0, 0});
}

void Conductor::set_oscillator(int index, const OscillatorSettings& oscillator) {
    Event event{EventType::set_oscillator, 0, 0};
    event.index = index;
    event.oscillator = oscillator;
    mailbox_.send(event);
}

void Conductor::start_click() {
    mailbox_.send(Event{EventType::start_click, 0, 0});
}

void Conductor::stop_click() {
    mailbox_.send(Event{EventType::stop_click, 0, 0});
}

void Conductor::set_tempo(int tempo) {
    Event event{EventType::set_tempo, 0, 0};
    event.tempo = tempo;
    mailbox_.send(event);
}

void Conductor::press_looper() {
    bool sent = mailbox_.send(Event{EventType::press_looper, 0, 0});
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::clear_loop() {
    bool sent = mailbox_.send(Event{EventType::clear_loop, 0, 0});
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::remove_layer(int layer) {
    Event event{EventType::remove_layer, 0, 0};
    event.layer = layer;
    bool sent = mailbox_.send(event);
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::set_layer_volume(int layer, float volume) {
    Event event{EventType::set_layer_volume, 0, 0};
    event.layer = layer;
    event.volume = volume;
    mailbox_.send(event);
}

void Conductor::set_snap(bool snap) {
    Event event{EventType::set_snap, 0, 0};
    event.snap = snap;
    mailbox_.send(event);
}

void Conductor::set_filter(const FilterSettings& settings) {
    Event event{EventType::set_filter, 0, 0};
    event.filter = settings;
    mailbox_.send(event);
}

void Conductor::set_filter_envelope(const FilterEnvelopeSettings& envelope) {
    Event event{EventType::set_filter_envelope, 0, 0};
    event.filter_envelope = envelope;
    mailbox_.send(event);
}

void Conductor::set_unison(const UnisonSettings& unison) {
    Event event{EventType::set_unison, 0, 0};
    event.unison = unison;
    mailbox_.send(event);
}

void Conductor::set_loudness_envelope(const LoudnessEnvelopeSettings& envelope) {
    Event event{EventType::set_loudness_envelope, 0, 0};
    event.loudness_envelope = envelope;
    mailbox_.send(event);
}

void Conductor::set_key_tracking(float key_tracking) {
    Event event{EventType::set_key_tracking, 0, 0};
    event.key_tracking = key_tracking;
    mailbox_.send(event);
}

void Conductor::set_tuning(float a4_hz) {
    Event event{EventType::set_tuning, 0, 0};
    event.a4_hz = a4_hz;
    mailbox_.send(event);
}

void Conductor::set_lfo(int index, const LfoSettings& settings) {
    Event event{EventType::set_lfo, 0, 0};
    event.index = index;
    event.lfo = settings;
    mailbox_.send(event);
}

void Conductor::set_connection(int index, const Connection& connection) {
    Event event{EventType::set_connection, 0, 0};
    event.index = index;
    event.connection = connection;
    mailbox_.send(event);
}

void Conductor::set_vibrato_key_times(int rise_ms, int fall_ms) {
    Event event{EventType::set_vibrato_key_times, 0, 0};
    event.rise_ms = rise_ms;
    event.fall_ms = fall_ms;
    mailbox_.send(event);
}

void Conductor::set_vibrato_key(bool held) {
    Event event{EventType::set_vibrato_key, 0, 0};
    event.held = held;
    mailbox_.send(event);
}

bool Conductor::play_song(const SongChord* chords, int count) {
    // A stop, every chord, then the play.
    std::uint64_t needed = static_cast<std::uint64_t>(count) + 2;
    if (mailbox_.room() < needed) {
        return false;
    }

    mailbox_.send(Event{EventType::stop_song, 0, 0});
    for (int i = 0; i < count; i++) {
        Event event{EventType::add_song_chord, 0, 0};
        event.chord = chords[i];
        mailbox_.send(event);
    }
    mailbox_.send(Event{EventType::play_song, 0, 0});

    song_messages_sent_ += needed;
    song_will_play_ = true;
    return true;
}

void Conductor::stop_song() {
    bool sent = mailbox_.send(Event{EventType::stop_song, 0, 0});
    if (sent) {
        song_messages_sent_ += 1;
        song_will_play_ = false;
    }
}

std::uint64_t Conductor::song_position() const {
    // Read the count first, as in looper_status.
    std::uint64_t handled = shown_song_handled_.load();
    std::uint64_t position = shown_song_position_.load();

    // Still on its way: say what it will do once the audio thread acts on
    // it, so the screen never sees a song stop just before it starts.
    if (handled != song_messages_sent_) {
        if (song_will_play_) {
            return 0;
        }
        return SongPlayer::not_playing;
    }
    return position;
}

LooperStatus Conductor::looper_status() const {
    // The audio thread tells the count last, so reading it first means the
    // state read after it is at least as new.
    std::uint64_t handled = shown_handled_.load();

    LooperStatus status;
    status.state = shown_state_.load();
    status.number = shown_number_.load();
    status.position = shown_position_.load();
    status.waiting = handled != looper_messages_sent_;
    return status;
}

bool Conductor::next_loop_change(LoopChange& change) {
    return loop_changes_.receive(change);
}

void Conductor::render(float* out, int frames) {
    // Start from silence: the synth and the click both add on top.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = 0.0f;
    }

    // Sample by sample, so each tick starts on its beat's exact sample, and
    // each loop note and song chord on its own, wherever they fall in the
    // chunk. Before the looper or the song player starts or ends a note, the
    // synth renders the stretch up to that sample, so the note is heard from
    // there on.
    int rendered = 0;
    for (int i = 0; i < frames; i++) {
        if (looper_.has_something_now(clock_)) {
            synth_.render(out + 2 * rendered, i - rendered);
            rendered = i;
            looper_.act_now(clock_, synth_);
        }
        if (song_player_.has_something_now(clock_.now())) {
            synth_.render(out + 2 * rendered, i - rendered);
            rendered = i;
            song_player_.act_now(clock_.now(), synth_);
        }

        // Everything sent since the last chunk is heard from its first
        // sample. The looper has already acted on that sample, so a note
        // played on the sample recording starts is recorded. While the beat
        // is stopped, the LFOs in beats count at the tempo asked for.
        if (i == 0) {
            take_messages();
            if (!clock_.is_beat_running()) {
                synth_.set_beats_per_sample(beats_per_sample(clock_.asked_beat_length()));
            }
        }

        // The LFOs in beats hear each beat on its own sample, at the tempo
        // it plays at.
        if (clock_.beat_starts_now()) {
            synth_.render(out + 2 * rendered, i - rendered);
            rendered = i;
            synth_.on_beat(clock_.beat_number(), beats_per_sample(clock_.asked_beat_length()));

            if (wants_click()) {
                click_.start(clock_.bar_starts_now());
            }
        }
        float tick = click_.next_sample();
        out[2 * i] += tick;
        out[2 * i + 1] += tick;

        clock_.step();
    }
    synth_.render(out + 2 * rendered, frames - rendered);

    // Notes and click together can add up to more than speakers can play.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = soft_clip(out[i]);
    }

    publish_looper_status();
    publish_song_position();
}

void Conductor::take_messages() {
    Event event{};
    while (mailbox_.receive(event)) {
        handle(event);
    }
}

void Conductor::handle(const Event& event) {
    // Live notes go to the synth, and to the looper in case it is recording.
    // The current sample is the first one they are heard on.
    std::uint64_t now = clock_.now();

    if (event.type == EventType::note_on) {
        synth_.note_on(event.note, event.velocity, Source::live);
        looper_.note_on(event.note, event.velocity, now);
    } else if (event.type == EventType::note_off) {
        synth_.note_off(event.note, Source::live);
        looper_.note_off(event.note, now);
    } else if (event.type == EventType::all_notes_off) {
        synth_.all_notes_off(Source::live);
        looper_.all_notes_off(now);
    } else if (event.type == EventType::set_oscillator) {
        synth_.set_oscillator(event.index, event.oscillator);
    } else if (event.type == EventType::start_click) {
        click_on_ = true;
        // Starting again while the beat runs would jump back to beat 1.
        if (!clock_.is_beat_running()) {
            clock_.start_beat();
        }
    } else if (event.type == EventType::stop_click) {
        click_on_ = false;
        stop_beat_if_unused();
    } else if (event.type == EventType::set_tempo) {
        // A loop is recorded and played on the beat, so the tempo is locked
        // from the looper's first press until it is cleared.
        if (looper_.is_idle()) {
            clock_.set_tempo(event.tempo);
        }
    } else if (event.type == EventType::press_looper) {
        handle_looper_press();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::clear_loop) {
        looper_.clear(synth_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::remove_layer) {
        // Removing the only layer clears the loop.
        looper_.remove_layer(event.layer, synth_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::set_layer_volume) {
        looper_.set_layer_volume(event.layer, event.volume, synth_);
    } else if (event.type == EventType::set_snap) {
        looper_.set_snap(event.snap, now, synth_);
    } else if (event.type == EventType::set_filter) {
        synth_.set_filter(event.filter);
    } else if (event.type == EventType::set_filter_envelope) {
        synth_.set_filter_envelope(event.filter_envelope);
    } else if (event.type == EventType::set_unison) {
        synth_.set_unison(event.unison);
    } else if (event.type == EventType::set_loudness_envelope) {
        synth_.set_loudness_envelope(event.loudness_envelope);
    } else if (event.type == EventType::set_key_tracking) {
        synth_.set_key_tracking(event.key_tracking);
    } else if (event.type == EventType::set_tuning) {
        synth_.set_tuning(event.a4_hz);
    } else if (event.type == EventType::set_lfo) {
        synth_.set_lfo(event.index, event.lfo);
    } else if (event.type == EventType::set_connection) {
        synth_.set_connection(event.index, event.connection);
    } else if (event.type == EventType::set_vibrato_key_times) {
        synth_.set_vibrato_key_times(event.rise_ms, event.fall_ms);
    } else if (event.type == EventType::set_vibrato_key) {
        // Like a live note: the synth plays it, and the looper records it.
        synth_.set_vibrato_key(event.held);
        if (event.held) {
            looper_.note_on(Synth::vibrato_key_note, vibrato_key_velocity, now);
        } else {
            looper_.note_off(Synth::vibrato_key_note, now);
        }
    } else if (event.type == EventType::stop_song) {
        song_player_.stop(synth_);
        song_messages_handled_ += 1;
    } else if (event.type == EventType::add_song_chord) {
        song_player_.add(event.chord);
        song_messages_handled_ += 1;
    } else if (event.type == EventType::play_song) {
        // play_song always sends a stop first, so nothing is playing here.
        // Its presses count from 0 again, so it moves the same each time.
        synth_.restart_song_presses();
        song_player_.play(now, synth_);
        song_messages_handled_ += 1;
    }
}

void Conductor::handle_looper_press() {
    // The count-in starts a new bar right away.
    if (looper_.is_idle()) {
        clock_.start_beat();
    }
    looper_.press(clock_, synth_);

    // A press during the count-in cancels it, and stopping a layer with no
    // notes when it is the only one clears the loop.
    stop_beat_if_unused();
}

bool Conductor::wants_click() const {
    if (click_on_) {
        return true;
    }
    return looper_.wants_click();
}

void Conductor::stop_beat_if_unused() {
    if (click_on_) {
        return;
    }
    if (!looper_.is_idle()) {
        return;
    }
    clock_.stop_beat();
}

void Conductor::publish_looper_status() {
    shown_state_.store(looper_.state());
    shown_number_.store(looper_.number());
    shown_position_.store(looper_.position(clock_.now()));
    // Last: see looper_status.
    shown_handled_.store(looper_messages_handled_);
}

void Conductor::publish_song_position() {
    shown_song_position_.store(song_player_.position(clock_.now()));
    // Last: see song_position.
    shown_song_handled_.store(song_messages_handled_);
}
