#include "conductor.h"

#include <cmath>

namespace {

// How many beats pass each sample at the tempo `beat` says.
double beats_per_sample(BeatLength beat) {
    return static_cast<double>(beat.tempo) / static_cast<double>(beat.samples_per_minute);
}

// How hard the vibrato and glide keys are pressed when the looper records
// them. Only that they are held matters, so any velocity would do.
constexpr int vibrato_key_velocity = 127;
constexpr int glide_key_velocity = 127;
constexpr int pedal_key_velocity = 127;

// How hard a string's move is when the looper records it, as a note of its own.
constexpr int moved_string_velocity = 100;

}  // namespace

Conductor::Conductor(int sample_rate)
    : sample_rate_(sample_rate),
      tracks_(sample_rate),
      mixer_(sample_rate),
      limiter_(sample_rate),
      clock_(sample_rate),
      click_(sample_rate),
      looper_(loop_changes_),
      sequencer_(timed_events_) {
    count_beats_while_stopped();
}

Conductor::~Conductor() {
    // Effects the audio thread never took in. The ones in the chains go
    // with the mixer.
    Event event{};
    while (mailbox_.receive(event)) {
        if (event.type == EventType::set_effect) {
            delete event.effect;
        }
    }
    free_replaced_effects();
}

void Conductor::note_on(int track, int note, int velocity) {
    Event event{EventType::note_on, note, velocity};
    event.track = track;
    send(event);
}

void Conductor::note_off(int track, int note) {
    Event event{EventType::note_off, note, 0};
    event.track = track;
    send(event);
}

void Conductor::move_string(int track, int from, int to) {
    Event event{EventType::move_string, to, 0};
    event.track = track;
    event.from_note = from;
    send(event);
}

void Conductor::pluck_string_after(int track, int from, int to, int velocity) {
    Event event{EventType::pluck_string_after, to, velocity};
    event.track = track;
    event.from_note = from;
    send(event);
}

void Conductor::all_notes_off() {
    send_counted(Event{EventType::all_notes_off, 0, 0});
}

void Conductor::set_oscillator(int track, int index, const OscillatorSettings& oscillator) {
    Event event{EventType::set_oscillator, 0, 0};
    event.track = track;
    event.index = index;
    event.oscillator = oscillator;
    send(event);
}

void Conductor::start_click() {
    send_counted(Event{EventType::start_click, 0, 0});
}

void Conductor::stop_click() {
    send_counted(Event{EventType::stop_click, 0, 0});
}

void Conductor::set_tempo(int tempo) {
    Event event{EventType::set_tempo, 0, 0};
    event.tempo = tempo;
    send(event);
}

void Conductor::press_looper(int track) {
    Event event{EventType::press_looper, 0, 0};
    event.track = track;
    bool sent = send_counted(event);
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::clear_loop() {
    bool sent = send_counted(Event{EventType::clear_loop, 0, 0});
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::remove_layer(int layer) {
    Event event{EventType::remove_layer, 0, 0};
    event.layer = layer;
    bool sent = send_counted(event);
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::set_layer_volume(int layer, float volume) {
    Event event{EventType::set_layer_volume, 0, 0};
    event.layer = layer;
    event.volume = volume;
    send_counted(event);
}

void Conductor::pause_loop() {
    bool sent = send_counted(Event{EventType::pause_loop, 0, 0});
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::play_loop() {
    bool sent = send_counted(Event{EventType::play_loop, 0, 0});
    if (sent) {
        looper_messages_sent_ += 1;
    }
}

void Conductor::set_layer_playing(int layer, bool playing) {
    Event event{EventType::set_layer_playing, 0, 0};
    event.layer = layer;
    event.playing = playing;
    send_counted(event);
}

bool Conductor::load_loop(const LoadedNote* notes, int count, int layers, int bars) {
    // Every note, then the one event that takes them in.
    if (loaded_notes_.room() < static_cast<std::uint64_t>(count)) {
        return false;
    }
    if (mailbox_.room() < 1) {
        return false;
    }

    for (int i = 0; i < count; i++) {
        loaded_notes_.send(notes[i]);
    }
    Event event{EventType::load_loop, 0, 0};
    event.loaded_count = count;
    event.loaded_layers = layers;
    event.loaded_bars = bars;
    send_counted(event);
    looper_messages_sent_ += 1;
    return true;
}

void Conductor::set_snap(bool snap) {
    Event event{EventType::set_snap, 0, 0};
    event.snap = snap;
    send_counted(event);
}

void Conductor::set_filter(int track, const FilterSettings& settings) {
    Event event{EventType::set_filter, 0, 0};
    event.track = track;
    event.filter = settings;
    send(event);
}

void Conductor::set_filter_envelope(int track, const FilterEnvelopeSettings& envelope) {
    Event event{EventType::set_filter_envelope, 0, 0};
    event.track = track;
    event.filter_envelope = envelope;
    send(event);
}

void Conductor::set_unison(int track, const UnisonSettings& unison) {
    Event event{EventType::set_unison, 0, 0};
    event.track = track;
    event.unison = unison;
    send(event);
}

void Conductor::set_amp(int track, const AmpSettings& amp) {
    Event event{EventType::set_amp, 0, 0};
    event.track = track;
    event.amp = amp;
    send(event);
}

void Conductor::set_loudness_envelope(int track, const LoudnessEnvelopeSettings& envelope) {
    Event event{EventType::set_loudness_envelope, 0, 0};
    event.track = track;
    event.loudness_envelope = envelope;
    send(event);
}

void Conductor::set_key_tracking(int track, float key_tracking) {
    Event event{EventType::set_key_tracking, 0, 0};
    event.track = track;
    event.key_tracking = key_tracking;
    send(event);
}

void Conductor::set_tuning(int track, float a4_hz) {
    Event event{EventType::set_tuning, 0, 0};
    event.track = track;
    event.a4_hz = a4_hz;
    send(event);
}

void Conductor::set_lfo(int track, int index, const LfoSettings& settings) {
    Event event{EventType::set_lfo, 0, 0};
    // A bus's or the master's are the mixer's.
    if (track < Tracks::count) {
        event.track = track;
    } else {
        event.type = EventType::set_chain_lfo;
        event.chain = track;
    }
    event.index = index;
    event.lfo = settings;
    send(event);
}

void Conductor::set_connection(int track, int index, const Connection& connection) {
    Event event{EventType::set_connection, 0, 0};
    if (track < Tracks::count) {
        event.track = track;
    } else {
        event.type = EventType::set_chain_connection;
        event.chain = track;
    }
    event.index = index;
    event.connection = connection;
    send(event);
}

void Conductor::set_vibrato_key_times(int track, int rise_ms, int fall_ms) {
    Event event{EventType::set_vibrato_key_times, 0, 0};
    event.track = track;
    event.rise_ms = rise_ms;
    event.fall_ms = fall_ms;
    send(event);
}

void Conductor::set_vibrato_key(int track, bool held) {
    Event event{EventType::set_vibrato_key, 0, 0};
    event.track = track;
    event.held = held;
    send(event);
}

void Conductor::set_glide_key(int track, bool held) {
    Event event{EventType::set_glide_key, 0, 0};
    event.track = track;
    event.held = held;
    send(event);
}

void Conductor::set_pedal_key(int track, bool held) {
    Event event{EventType::set_pedal_key, 0, 0};
    event.track = track;
    event.held = held;
    send(event);
}

void Conductor::set_glide_time(int track, int glide_ms) {
    Event event{EventType::set_glide_time, 0, 0};
    event.track = track;
    event.glide_ms = glide_ms;
    send(event);
}

bool Conductor::set_instrument(int track, int index, const SampleSet* set) {
    Event event{EventType::set_instrument, 0, 0};
    event.track = track;
    event.index = index;
    event.set = set;
    bool sent = send_counted(event);
    if (sent) {
        sample_messages_sent_ += 1;
    }
    return sent;
}

bool Conductor::set_drum_kit(int track, const SampleSet* set) {
    Event event{EventType::set_drum_kit, 0, 0};
    event.track = track;
    event.set = set;
    bool sent = send_counted(event);
    if (sent) {
        sample_messages_sent_ += 1;
    }
    return sent;
}

void Conductor::set_drum_pad(int track, int pad, const DrumPad& settings) {
    Event event{EventType::set_drum_pad, 0, 0};
    event.track = track;
    event.index = pad;
    event.pad = settings;
    send(event);
}

void Conductor::set_track_volume(int track, float volume) {
    Event event{EventType::set_track_volume, 0, 0};
    event.track = track;
    event.volume = volume;
    send(event);
}

void Conductor::set_track_pan(int track, float pan) {
    Event event{EventType::set_track_pan, 0, 0};
    event.track = track;
    event.pan = pan;
    send(event);
}

void Conductor::set_send(int track, int bus, float amount) {
    Event event{EventType::set_send, 0, 0};
    event.track = track;
    event.bus = bus;
    event.volume = amount;
    send(event);
}

void Conductor::set_bus_volume(int bus, float volume) {
    Event event{EventType::set_bus_volume, 0, 0};
    event.bus = bus;
    event.volume = volume;
    send(event);
}

void Conductor::set_bus_pan(int bus, float pan) {
    Event event{EventType::set_bus_pan, 0, 0};
    event.bus = bus;
    event.pan = pan;
    send(event);
}

void Conductor::set_limiter(const LimiterSettings& settings) {
    Event event{EventType::set_limiter, 0, 0};
    event.limiter = settings;
    send(event);
}

bool Conductor::set_effect(int chain, int slot, EffectKind kind) {
    free_replaced_effects();

    Event event{EventType::set_effect, 0, 0};
    event.chain = chain;
    event.slot = slot;
    bool on_bus = false;
    if (chain >= Mixer::first_bus_chain) {
        on_bus = chain < Mixer::master_chain;
    }
    event.effect = make_effect(kind, sample_rate_, on_bus);
    bool sent = send_counted(event);
    if (!sent) {
        delete event.effect;
        return false;
    }
    if (event.effect != nullptr) {
        effects_made_ += 1;
    }
    return true;
}

bool Conductor::move_effect(int chain, int from, int to) {
    Event event{EventType::move_effect, 0, 0};
    event.chain = chain;
    event.slot = from;
    event.to = to;
    return send_counted(event);
}

void Conductor::set_effect_on(int chain, int slot, bool on) {
    Event event{EventType::set_effect_on, 0, 0};
    event.chain = chain;
    event.slot = slot;
    event.on = on;
    send(event);
}

void Conductor::set_effect_value(int chain, int slot, int value, float number) {
    Event event{EventType::set_effect_value, 0, 0};
    event.chain = chain;
    event.slot = slot;
    event.value = value;
    event.number = number;
    send(event);
}

bool Conductor::set_chain_order(const std::array<int, Mixer::master_chain>& order) {
    Event event{EventType::set_chain_order, 0, 0};
    event.order = order;
    return send_counted(event);
}

std::uint64_t Conductor::message_room() const {
    return mailbox_.room();
}

std::uint64_t Conductor::dropped_messages() const {
    return dropped_messages_;
}

void Conductor::take_waiting_messages() {
    take_messages();
}

float Conductor::effect_reduction_db(int chain, int slot) const {
    return shown_reductions_[chain][slot].load();
}

int Conductor::effect_count() {
    free_replaced_effects();
    return effects_made_ - effects_freed_;
}

void Conductor::free_replaced_effects() {
    Effect* effect = nullptr;
    while (freed_effects_.receive(effect)) {
        delete effect;
        effects_freed_ += 1;
    }
}

int Conductor::use_lookahead() {
    limiter_.use_lookahead();
    return limiter_.lookahead_frames();
}

float Conductor::limiter_reduction_db() const {
    return shown_limiter_reduction_.load();
}

void Conductor::layer_note_on(int track, int slot, int note, int velocity) {
    Event event{EventType::layer_note_on, note, velocity};
    event.track = track;
    event.layer = slot;
    send(event);
}

void Conductor::layer_note_off(int track, int slot, int note) {
    Event event{EventType::layer_note_off, note, 0};
    event.track = track;
    event.layer = slot;
    send(event);
}

void Conductor::set_slot_volume(int track, int slot, float volume) {
    Event event{EventType::set_slot_volume, 0, 0};
    event.track = track;
    event.layer = slot;
    event.volume = volume;
    send(event);
}

void Conductor::time_next(std::uint64_t sample) {
    timing_ = true;
    timed_sample_ = sample;
}

void Conductor::send_now() {
    timing_ = false;
}

void Conductor::play_sequence() {
    Event event{EventType::play_sequence, 0, 0};
    event.sequence = sequence_sent_;
    bool sent = send_counted(event);
    if (sent) {
        sequence_messages_sent_ += 1;
        sequence_will_play_ = true;
    }
}

void Conductor::stop_sequence() {
    Event event{EventType::stop_sequence, 0, 0};
    event.sequence = sequence_sent_;
    bool sent = send_counted(event);
    if (sent) {
        sequence_messages_sent_ += 1;
        sequence_will_play_ = false;
    }
    // The timed messages from here on belong to the next sequence, even if
    // the stop did not fit: its messages would only wait, never play.
    sequence_sent_ += 1;
}

std::uint64_t Conductor::sequence_position() const {
    // Read the count first, as in looper_status.
    std::uint64_t handled = shown_sequence_handled_.load();
    std::uint64_t position = shown_sequence_position_.load();
    if (handled != sequence_messages_sent_) {
        if (sequence_will_play_) {
            return 0;
        }
        return Sequencer::not_playing;
    }
    return position;
}

std::uint64_t Conductor::sequence_room() const {
    return timed_events_.room();
}

bool Conductor::send_counted(const Event& event) {
    bool sent = mailbox_.send(event);
    if (!sent) {
        dropped_messages_ += 1;
    }
    return sent;
}

void Conductor::send(const Event& event) {
    if (!timing_) {
        send_counted(event);
        return;
    }
    TimedEvent timed;
    timed.sequence = sequence_sent_;
    timed.sample = timed_sample_;
    timed.event = event;
    bool sent = timed_events_.send(timed);
    if (!sent) {
        dropped_messages_ += 1;
    }
}

std::uint32_t Conductor::sets_in_use() const {
    return shown_sets_in_use_.load();
}

std::uint64_t Conductor::sample_messages_handled() const {
    return shown_sample_handled_.load();
}

std::uint64_t Conductor::sample_messages_sent() const {
    return sample_messages_sent_;
}

bool Conductor::play_song(int track, const SongChord* chords, int count) {
    // A stop, every chord, then the play.
    std::uint64_t needed = static_cast<std::uint64_t>(count) + 2;
    if (mailbox_.room() < needed) {
        return false;
    }

    send_counted(Event{EventType::stop_song, 0, 0});
    for (int i = 0; i < count; i++) {
        Event event{EventType::add_song_chord, 0, 0};
        event.chord = chords[i];
        send_counted(event);
    }
    Event play{EventType::play_song, 0, 0};
    play.track = track;
    send_counted(play);

    song_messages_sent_ += needed;
    song_will_play_ = true;
    return true;
}

void Conductor::stop_song() {
    bool sent = send_counted(Event{EventType::stop_song, 0, 0});
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
    // Start from silence: the tracks and the click all add on top.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = 0.0f;
    }

    // Sample by sample, so each tick starts on its beat's exact sample, and
    // each loop note, song chord and timed message on its own, wherever they
    // fall in the chunk. Before the looper, the song player or the sequencer
    // starts or ends a note, the tracks render the stretch up to that
    // sample, so the note is heard from there on.
    int rendered = 0;
    for (int i = 0; i < frames; i++) {
        if (looper_.has_something_now(clock_)) {
            mixer_.render(tracks_, out + 2 * rendered, i - rendered);
            rendered = i;
            looper_.act_now(clock_, tracks_);
        }
        if (song_player_.has_something_now(clock_.now())) {
            mixer_.render(tracks_, out + 2 * rendered, i - rendered);
            rendered = i;
            song_player_.act_now(clock_.now(), tracks_.synth(song_track_));
        }

        // Everything sent since the last chunk is heard from its first
        // sample. The looper has already acted on that sample, so a note
        // played on the sample recording starts is recorded. While the beat
        // is stopped, the LFOs in beats count at the tempo asked for.
        if (i == 0) {
            take_messages();
            count_beats_while_stopped();
        }

        // The sequencer's messages due by now, each like a message from the
        // mailbox, heard from this sample.
        Event due{};
        bool rendered_here = false;
        while (sequencer_.next_due(clock_.now(), due)) {
            if (!rendered_here) {
                mixer_.render(tracks_, out + 2 * rendered, i - rendered);
                rendered = i;
                rendered_here = true;
            }
            handle(due);
        }
        if (rendered_here) {
            count_beats_while_stopped();
        }

        // The LFOs in beats, and the effects that follow the beat, hear
        // each beat on its own sample, at the tempo it plays at.
        if (clock_.beat_starts_now()) {
            mixer_.render(tracks_, out + 2 * rendered, i - rendered);
            rendered = i;
            double beat_speed = beats_per_sample(clock_.asked_beat_length());
            tracks_.on_beat(clock_.beat_number(), beat_speed);
            mixer_.on_beat(clock_.beat_number(), beat_speed);

            if (wants_click()) {
                click_.start(clock_.bar_starts_now());
            }
        }
        float tick = click_.next_sample();
        out[2 * i] += tick;
        out[2 * i + 1] += tick;

        clock_.step();
    }
    mixer_.render(tracks_, out + 2 * rendered, frames - rendered);

    // The tracks, the buses and the click together can add up to more than
    // speakers can play, so the limiter holds them under its ceiling.
    limiter_.process(out, frames);
    shown_limiter_reduction_.store(limiter_.reduction_db());

    publish_reductions();
    publish_looper_status();
    publish_song_position();
    publish_sequence_position();
    publish_sets_in_use();
}

void Conductor::take_messages() {
    Event event{};
    while (mailbox_.receive(event)) {
        handle(event);
    }
}

void Conductor::handle(const Event& event) {
    // Live notes go to their track's synth, and to the looper in case it is
    // recording. The current sample is the first one they are heard on.
    std::uint64_t now = clock_.now();
    Synth& synth = tracks_.synth(event.track);

    if (event.type == EventType::note_on) {
        synth.note_on(event.note, event.velocity, Source::live);
        looper_.note_on(event.note, event.velocity, now, event.track);
    } else if (event.type == EventType::note_off) {
        synth.note_off(event.note, Source::live);
        looper_.note_off(event.note, now, event.track);
    } else if (event.type == EventType::move_string) {
        synth.move_string(event.from_note, event.note);
        looper_.note_off(event.from_note, now, event.track);
        looper_.note_on(event.note, moved_string_velocity, now, event.track);
    } else if (event.type == EventType::pluck_string_after) {
        synth.pluck_string_after(event.from_note, event.note, event.velocity);
        looper_.note_off(event.from_note, now, event.track);
        looper_.note_on(event.note, event.velocity, now, event.track);
    } else if (event.type == EventType::all_notes_off) {
        tracks_.all_notes_off(Source::live);
        looper_.all_notes_off(now);
    } else if (event.type == EventType::set_oscillator) {
        synth.set_oscillator(event.index, event.oscillator);
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
        handle_looper_press(event.track);
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::clear_loop) {
        looper_.clear(tracks_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::remove_layer) {
        // Removing the only layer clears the loop.
        looper_.remove_layer(event.layer, tracks_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::set_layer_volume) {
        looper_.set_layer_volume(event.layer, event.volume, tracks_);
    } else if (event.type == EventType::set_layer_playing) {
        looper_.set_layer_playing(event.layer, event.playing, now, tracks_);
    } else if (event.type == EventType::pause_loop) {
        looper_.pause(now, tracks_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::play_loop) {
        // Like Right Shift while paused: the loop starts on a new bar.
        if (looper_.is_paused()) {
            clock_.start_beat();
            looper_.play(now, tracks_);
        }
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::load_loop) {
        // The tempo is the one asked for last, sent after the clear that
        // comes before a load, while the looper was idle.
        looper_.load(loaded_notes_, event.loaded_count, event.loaded_layers, event.loaded_bars,
                     clock_.asked_beat_length(), tracks_);
        stop_beat_if_unused();
        looper_messages_handled_ += 1;
    } else if (event.type == EventType::set_snap) {
        looper_.set_snap(event.snap, now, tracks_);
    } else if (event.type == EventType::set_filter) {
        synth.set_filter(event.filter);
    } else if (event.type == EventType::set_filter_envelope) {
        synth.set_filter_envelope(event.filter_envelope);
    } else if (event.type == EventType::set_unison) {
        synth.set_unison(event.unison);
    } else if (event.type == EventType::set_amp) {
        synth.set_amp(event.amp);
    } else if (event.type == EventType::set_loudness_envelope) {
        synth.set_loudness_envelope(event.loudness_envelope);
    } else if (event.type == EventType::set_key_tracking) {
        synth.set_key_tracking(event.key_tracking);
    } else if (event.type == EventType::set_tuning) {
        synth.set_tuning(event.a4_hz);
    } else if (event.type == EventType::set_lfo) {
        synth.set_lfo(event.index, event.lfo);
    } else if (event.type == EventType::set_connection) {
        synth.set_connection(event.index, event.connection);
    } else if (event.type == EventType::set_vibrato_key_times) {
        synth.set_vibrato_key_times(event.rise_ms, event.fall_ms);
    } else if (event.type == EventType::set_vibrato_key) {
        // Like a live note: the synth plays it, and the looper records it.
        synth.set_vibrato_key(event.held);
        if (event.held) {
            looper_.note_on(Synth::vibrato_key_note, vibrato_key_velocity, now, event.track);
        } else {
            looper_.note_off(Synth::vibrato_key_note, now, event.track);
        }
    } else if (event.type == EventType::set_glide_key) {
        // The same for the glide key.
        synth.set_glide_key(event.held);
        if (event.held) {
            looper_.note_on(Synth::glide_key_note, glide_key_velocity, now, event.track);
        } else {
            looper_.note_off(Synth::glide_key_note, now, event.track);
        }
    } else if (event.type == EventType::set_pedal_key) {
        // The same for the pedal.
        synth.set_pedal_key(event.held);
        if (event.held) {
            looper_.note_on(Synth::pedal_key_note, pedal_key_velocity, now, event.track);
        } else {
            looper_.note_off(Synth::pedal_key_note, now, event.track);
        }
    } else if (event.type == EventType::set_glide_time) {
        synth.set_glide_time(event.glide_ms);
    } else if (event.type == EventType::set_instrument) {
        synth.set_instrument(event.index, event.set);
        sample_messages_handled_ += 1;
    } else if (event.type == EventType::set_drum_kit) {
        synth.set_drum_kit(event.set);
        sample_messages_handled_ += 1;
    } else if (event.type == EventType::set_drum_pad) {
        synth.set_drum_pad(event.index, event.pad);
    } else if (event.type == EventType::set_track_volume) {
        mixer_.set_volume(event.track, event.volume);
    } else if (event.type == EventType::set_track_pan) {
        mixer_.set_pan(event.track, event.pan);
    } else if (event.type == EventType::set_send) {
        mixer_.set_send(event.track, event.bus, event.volume);
    } else if (event.type == EventType::set_bus_volume) {
        mixer_.set_bus_volume(event.bus, event.volume);
    } else if (event.type == EventType::set_bus_pan) {
        mixer_.set_bus_pan(event.bus, event.pan);
    } else if (event.type == EventType::set_limiter) {
        limiter_.set(event.limiter);
    } else if (event.type == EventType::set_effect) {
        // The replaced effect goes back to be freed off the audio thread.
        Effect* replaced = mixer_.chain(event.chain).swap(event.slot, event.effect);
        if (replaced != nullptr) {
            freed_effects_.send(replaced);
        }
    } else if (event.type == EventType::move_effect) {
        mixer_.chain(event.chain).move(event.slot, event.to);
    } else if (event.type == EventType::set_effect_on) {
        mixer_.chain(event.chain).set_on(event.slot, event.on);
    } else if (event.type == EventType::set_effect_value) {
        mixer_.chain(event.chain).set_value(event.slot, event.value, event.number);
    } else if (event.type == EventType::set_chain_order) {
        mixer_.set_order(event.order);
    } else if (event.type == EventType::set_chain_lfo) {
        mixer_.set_lfo(event.chain, event.index, event.lfo);
    } else if (event.type == EventType::set_chain_connection) {
        mixer_.set_connection(event.chain, event.index, event.connection);
    } else if (event.type == EventType::layer_note_on) {
        synth.loop_note_on(event.note, event.velocity, event.layer);
    } else if (event.type == EventType::layer_note_off) {
        synth.loop_note_off(event.note, event.layer);
    } else if (event.type == EventType::set_slot_volume) {
        synth.set_layer_volume(event.layer, event.volume);
    } else if (event.type == EventType::play_sequence) {
        sequencer_.play(event.sequence, now);
        sequence_messages_handled_ += 1;
    } else if (event.type == EventType::stop_sequence) {
        // Its layers' notes stop. Only while it plays: the loop plays on the
        // same slots once the sequence is over.
        if (sequencer_.is_playing()) {
            tracks_.all_notes_off(Source::loop);
        }
        sequencer_.stop(event.sequence);
        sequence_messages_handled_ += 1;
    } else if (event.type == EventType::stop_song) {
        song_player_.stop(tracks_.synth(song_track_));
        song_messages_handled_ += 1;
    } else if (event.type == EventType::add_song_chord) {
        song_player_.add(event.chord);
        song_messages_handled_ += 1;
    } else if (event.type == EventType::play_song) {
        // play_song always sends a stop first, so nothing is playing here.
        // Its presses count from 0 again, so it moves the same each time.
        song_track_ = event.track;
        Synth& song_synth = tracks_.synth(song_track_);
        song_synth.restart_song_presses();
        song_player_.play(now, song_synth);
        song_messages_handled_ += 1;
    }
}

void Conductor::handle_looper_press(int track) {
    // The count-in starts a new bar right away, and so does playing a
    // paused loop.
    if (looper_.is_idle()) {
        clock_.start_beat();
    } else if (looper_.is_paused()) {
        clock_.start_beat();
    }
    looper_.press(clock_, tracks_, track);

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
    if (looper_.needs_beat()) {
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

void Conductor::publish_sets_in_use() {
    shown_sets_in_use_.store(tracks_.sets_in_use());
    // Last: the engine reads the count first, so the sets read after it
    // are at least as new.
    shown_sample_handled_.store(sample_messages_handled_);
}

void Conductor::publish_song_position() {
    shown_song_position_.store(song_player_.position(clock_.now()));
    // Last: see song_position.
    shown_song_handled_.store(song_messages_handled_);
}

void Conductor::publish_sequence_position() {
    shown_sequence_position_.store(sequencer_.position(clock_.now()));
    // Last: see sequence_position.
    shown_sequence_handled_.store(sequence_messages_handled_);
}

void Conductor::count_beats_while_stopped() {
    if (clock_.is_beat_running()) {
        return;
    }
    double beat_speed = beats_per_sample(clock_.asked_beat_length());
    tracks_.set_beats_per_sample(beat_speed);
    mixer_.set_beats_per_sample(beat_speed);
}

void Conductor::publish_reductions() {
    for (int chain = 0; chain < Mixer::chain_count; chain++) {
        for (int slot = 0; slot < Chain::slot_count; slot++) {
            float reduction = mixer_.chain(chain).take_reduction_db(slot);
            shown_reductions_[chain][slot].store(reduction);
        }
    }
}
