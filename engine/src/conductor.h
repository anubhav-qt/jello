#pragma once

#include <array>
#include <atomic>
#include <cstdint>

#include "click.h"
#include "clock.h"
#include "limiter.h"
#include "looper.h"
#include "mailbox.h"
#include "mixer.h"
#include "sequencer.h"
#include "song_player.h"
#include "synth.h"
#include "tracks.h"

// What the looper is doing, for the screen.
struct LooperStatus {
    Looper::State state;
    // The count-in's beat, the bar being recorded, or how many bars the loop
    // lasts (Looper::number).
    int number;
    // How many samples into the count-in, the recording or the repeat
    // playing now (Looper::position).
    std::uint64_t position;
    // True while a looper press, clear or removal is still on its way to the
    // audio thread, so the state above doesn't show it yet.
    bool waiting;
};

// Runs everything the audio thread does (docs/adr/0021-a-looper-on-a-beat.md).
// Like an orchestra's conductor, it keeps the beat and cues the players: it
// takes the messages other threads leave in the mailbox, passes notes on to
// the tracks and the looper, gives Compose's song to the song player and the
// arrangement's timed messages to the sequencer, mixes the tracks, plays the
// click on the beat, and puts the sum through the master limiter, the last
// step before the speakers.
class Conductor {
public:
    explicit Conductor(int sample_rate);

    // Frees every effect still on its way to the audio thread or back.
    // Only once the audio has stopped.
    ~Conductor();

    Conductor(const Conductor&) = delete;
    Conductor& operator=(const Conductor&) = delete;

    // These only put a message in the mailbox. The audio thread acts on it at
    // the start of its next chunk of sound. Only one thread may call them at
    // a time. `track`, from 0 to Tracks::count - 1, says which track's synth
    // a note or a sound setting is for (docs/design.md#tracks-in-the-engine);
    // the caller makes sure it is in range.
    //
    // Between time_next and send_now, the ones marked "timed" go to the
    // sequencer instead, due on the sample time_next gave.
    void note_on(int track, int note, int velocity);  // timed
    void note_off(int track, int note);               // timed
    // Lets go of every live note on every track.
    void all_notes_off();
    // Oscillator `index`, from 0 (Synth::set_oscillator). Timed.
    void set_oscillator(int track, int index, const OscillatorSettings& oscillator);
    // Starts the click on beat 1 of a bar. Does nothing if it is already on.
    void start_click();
    // Stops the click. A tick already sounding rings out.
    void stop_click();
    // Plays the beat at `tempo` beats per minute from the next beat on.
    // Ignored while the looper is busy, so a loop keeps its beat. Timed.
    void set_tempo(int tempo);
    // Right Shift on track `track` (Looper::press), and clearing the loop
    // (Looper::clear).
    void press_looper(int track);
    void clear_loop();
    // Esc (Looper::remove_layer), and a layer's volume knob
    // (Looper::set_layer_volume).
    void remove_layer(int layer);
    void set_layer_volume(int layer, float volume);
    // Play and pause for the whole loop and for each layer (Looper::pause,
    // play and set_layer_playing).
    void pause_loop();
    void play_loop();
    void set_layer_playing(int layer, bool playing);
    // Opens a loop from a song file (Looper::load): `count` notes in
    // `layers` layers, `bars` bars long at the tempo asked for last. Returns
    // false and sends nothing if either mailbox has no room for all of it.
    bool load_loop(const LoadedNote* notes, int count, int layers, int bars);
    // The "snap to beat" switch (Looper::set_snap).
    void set_snap(bool snap);
    // The filter's controls (Synth::set_filter), and the filter envelope
    // (Synth::set_filter_envelope). Timed, like every sound setting from here
    // to set_glide_time, and set_drum_pad.
    void set_filter(int track, const FilterSettings& settings);
    void set_filter_envelope(int track, const FilterEnvelopeSettings& envelope);
    // Unison's copies (Synth::set_unison).
    void set_unison(int track, const UnisonSettings& unison);
    // The amp's knobs (Synth::set_amp).
    void set_amp(int track, const AmpSettings& amp);
    // The loudness envelope, the key tracking and the tuning
    // (Synth::set_loudness_envelope, set_key_tracking and set_tuning).
    void set_loudness_envelope(int track, const LoudnessEnvelopeSettings& envelope);
    void set_key_tracking(int track, float key_tracking);
    void set_tuning(int track, float a4_hz);
    // The routing list's LFOs and connections, and how long the vibrato key
    // takes to rise and fall (Synth::set_lfo, set_connection and
    // set_vibrato_key_times). The LFOs and connections also take a bus's or
    // the master's chain number for `track`, for its own (Mixer::set_lfo
    // and set_connection).
    void set_lfo(int track, int index, const LfoSettings& settings);
    void set_connection(int track, int index, const Connection& connection);
    void set_vibrato_key_times(int track, int rise_ms, int fall_ms);
    // Holds or lets go of the live vibrato key (Synth::set_vibrato_key). The
    // looper records it like a note.
    void set_vibrato_key(int track, bool held);
    // Holds or lets go of the live glide key (Synth::set_glide_key), which
    // the looper records like a note too, and how long a glide takes
    // (Synth::set_glide_time).
    void set_glide_key(int track, bool held);
    // Holds or lets go of the live pedal (Synth::set_pedal_key), which the
    // looper records like a note too.
    void set_pedal_key(int track, bool held);
    void set_glide_time(int track, int glide_ms);
    // Oscillator `index`'s instrument (Synth::set_instrument), the drum
    // kit's recordings (Synth::set_drum_kit) and pad `pad`'s settings
    // (Synth::set_drum_pad). The first two return false and send nothing if
    // the mailbox is full, so the sender knows the set is not on its way.
    // They are never timed.
    bool set_instrument(int track, int index, const SampleSet* set);
    bool set_drum_kit(int track, const SampleSet* set);
    void set_drum_pad(int track, int pad, const DrumPad& settings);

    // How loud track `track` plays, from 0 to 1 (Mixer::set_volume), where
    // it sits between the speakers, from -1 to 1 (Mixer::set_pan), and how
    // much of it goes to bus `bus`, from 0 to Mixer::bus_count - 1
    // (Mixer::set_send). A bus's own volume and pan (Mixer::set_bus_volume
    // and set_bus_pan). The limiter's settings (Limiter::set). All timed.
    void set_track_volume(int track, float volume);
    void set_track_pan(int track, float pan);
    void set_send(int track, int bus, float amount);
    void set_bus_volume(int bus, float volume);
    void set_bus_pan(int bus, float pan);
    void set_limiter(const LimiterSettings& settings);

    // Effects (docs/design.md#mix-in-the-engine), in chain `chain`, from 0
    // to Mixer::chain_count - 1, slot `slot`, from 0 to Chain::slot_count -
    // 1, which the caller makes sure of. set_effect puts a new effect of
    // `kind`, at its starting values, or none, in the slot, switched on
    // (Chain::swap). The effect is made here, on the calling thread. The one
    // it replaces comes back from the audio thread and is freed on a later
    // call. move_effect moves a slot's effect to slot `to` (Chain::move).
    // Both return false and send nothing if the mailbox is full, and are
    // never timed. set_effect_on and set_effect_value switch the effect on
    // or off and set its values (Chain::set_on and set_value). Both timed.
    bool set_effect(int chain, int slot, EffectKind kind);
    bool move_effect(int chain, int from, int to);
    void set_effect_on(int chain, int slot, bool on);
    void set_effect_value(int chain, int slot, int value, float number);

    // The order the chains before the master run in, for sidechains
    // (Mixer::set_order), which the caller makes sure of. Returns false and
    // sends nothing if the mailbox is full. Never timed.
    bool set_chain_order(const std::array<int, Mixer::master_chain>& order);

    // The most slot `slot`'s effect in chain `chain` turned the sound down
    // in the last chunk, in dB, 0 for an empty slot
    // (Chain::take_reduction_db).
    float effect_reduction_db(int chain, int slot) const;

    // How many effects the engine holds, in chains or on their way, after
    // freeing the ones that have come back. Only the sending thread may ask.
    int effect_count();

    // Makes the limiter look 1 ms ahead, for WAV renders, and returns how
    // many frames that holds the sound back (Limiter::use_lookahead). Only
    // before the first render, and never with the speakers running.
    int use_lookahead();

    // How many more messages the mailbox holds, and how many were ever lost
    // to it being full. Only the sending thread may ask.
    std::uint64_t message_room() const;
    std::uint64_t dropped_messages() const;

    // Acts on every waiting message on the calling thread, without moving
    // the sound on. Only with no audio running.
    void take_waiting_messages();

    // The most the limiter turned the sound down in the last chunk, in dB.
    float limiter_reduction_db() const;

    // A layer's notes and volume for the sequencer, which plays the
    // arrangement's layers on the tracks' layer slots, from 0 to
    // Synth::layer_count - 1, while the loop is cleared
    // (Synth::loop_note_on, loop_note_off and set_layer_volume). Timed.
    void layer_note_on(int track, int slot, int note, int velocity);
    void layer_note_off(int track, int slot, int note);
    void set_slot_volume(int track, int slot, float volume);

    // From here until send_now, the timed messages go to the sequencer, due
    // `sample` samples after the next sequence to play starts.
    void time_next(std::uint64_t sample);
    void send_now();

    // Starts the sequence that the timed messages since the last stop belong
    // to, from the start of the next chunk. Stopping it lets go of every
    // note its layers play, and drops its messages still waiting; the timed
    // messages after it belong to the next sequence. Play a sequence once:
    // stop it before playing again.
    void play_sequence();
    void stop_sequence();

    // How many samples the sequence has played as of the last chunk, or
    // Sequencer::not_playing; a play still on its way counts as 0, and a
    // stop as not playing. And how many more timed messages fit right now.
    // Only the thread that sends the messages may ask.
    std::uint64_t sequence_position() const;
    std::uint64_t sequence_room() const;

    // Which sample sets the audio thread still plays, one bit each, and how
    // many of the instrument and kit messages it has acted on. A set is
    // free to go once it is not in use and every message sent with it has
    // been acted on. Only the thread that sends the messages may ask.
    std::uint32_t sets_in_use() const;
    std::uint64_t sample_messages_handled() const;
    std::uint64_t sample_messages_sent() const;

    // What the looper is doing. Only the thread that sends the messages may
    // ask.
    LooperStatus looper_status() const;

    // Takes out the oldest change to the loop's notes. Returns false if none
    // is waiting. Only the thread that sends the messages may ask.
    bool next_loop_change(LoopChange& change);

    // Plays `chords`, `count` of them, one after another, each for its own
    // length, on track `track`, from the start of the next chunk
    // (docs/adr/0022-compose-as-a-tape.md). Whatever the song was playing
    // stops first. Returns false and sends nothing if the mailbox has no room
    // for all of it, so the audio thread never plays half a song.
    bool play_song(int track, const SongChord* chords, int count);

    // Lets go of what the song is playing and forgets it.
    void stop_song();

    // How many samples the song has played, counted from its first chord's
    // start, as of the last chunk, or SongPlayer::not_playing. A play still
    // on its way counts as 0, and a stop on its way as not playing. Only the
    // thread that sends the messages may ask.
    std::uint64_t song_position() const;

    // Fills `frames` frames of stereo (left, right, left, ...), every sample
    // between -1.0 and 1.0, and acts on every waiting message on the first
    // of them. Runs on the audio thread, so it never locks, allocates or
    // waits.
    void render(float* out, int frames);

private:
    // Puts a timed message in the mailbox, or in the sequencer's while
    // time_next is in force.
    void send(const Event& event);
    // Sends now, counting the message as lost if the mailbox is full.
    bool send_counted(const Event& event);

    // Acts on every message waiting in the mailbox, oldest first.
    void take_messages();

    // Does what one message asks.
    void handle(const Event& event);

    // Frees every effect the audio thread has swapped out since the last
    // time. Only the sending thread may call it.
    void free_replaced_effects();

    // Right Shift's press on track `track`, on the audio thread.
    void handle_looper_press(int track);

    // True while someone needs to hear the beat.
    bool wants_click() const;

    // Stops the beat once neither the click nor the looper needs it.
    void stop_beat_if_unused();

    // Tells the effects how long a beat is at the tempo asked for, while
    // the beat is stopped, as it does the tracks.
    void count_beats_while_stopped();

    // Tells the screen what the looper is doing, and how far the song has
    // played, at the end of each chunk.
    void publish_looper_status();
    void publish_song_position();
    void publish_sequence_position();
    void publish_sets_in_use();
    void publish_reductions();

    int sample_rate_;

    EventMailbox mailbox_;

    // Effects the audio thread swapped out, on their way back to be freed,
    // and how many were ever made and freed. Only the sending thread uses
    // the counts.
    FreedEffects freed_effects_;
    int effects_made_ = 0;
    int effects_freed_ = 0;

    // How many messages were lost to a full mailbox. Only the sending thread
    // uses it.
    std::uint64_t dropped_messages_ = 0;

    // The arrangement's timed messages, on their way to the sequencer.
    TimedEvents timed_events_;

    // The loop's changes, on their way to Rust. The looper sends them, so
    // this must be made before the looper, below.
    LoopChanges loop_changes_;

    // A loaded loop's notes, on their way to the looper.
    LoadedNotes loaded_notes_;

    Tracks tracks_;
    Mixer mixer_;
    Limiter limiter_;
    Clock clock_;
    Click click_;
    Looper looper_;
    SongPlayer song_player_;
    Sequencer sequencer_;

    // The track the song plays on.
    int song_track_ = 0;

    // Whether the click was started on its own, without the looper.
    bool click_on_ = false;

    // How many looper messages got into the mailbox. Only the sending thread
    // uses it.
    std::uint64_t looper_messages_sent_ = 0;

    // How many the audio thread has acted on. Only the audio thread uses it.
    std::uint64_t looper_messages_handled_ = 0;

    // What the audio thread last told the screen.
    std::atomic<Looper::State> shown_state_{Looper::State::idle};
    std::atomic<int> shown_number_{0};
    std::atomic<std::uint64_t> shown_position_{0};
    std::atomic<std::uint64_t> shown_handled_{0};

    // The same for the song: how many song messages got into the mailbox,
    // and whether the last one was a play, not a stop. Only the sending
    // thread uses these.
    std::uint64_t song_messages_sent_ = 0;
    bool song_will_play_ = false;

    // How many the audio thread has acted on. Only the audio thread uses it.
    std::uint64_t song_messages_handled_ = 0;

    // What the audio thread last told the screen about the song. One number
    // for where it is, so it can never be read half updated.
    std::atomic<std::uint64_t> shown_song_position_{SongPlayer::not_playing};
    std::atomic<std::uint64_t> shown_song_handled_{0};

    // The same for the sequencer. Timed messages go to sequence_sent_ while
    // timing_ is on, due on timed_sample_. Only the sending thread uses
    // these.
    bool timing_ = false;
    std::uint64_t timed_sample_ = 0;
    std::uint64_t sequence_sent_ = 1;
    std::uint64_t sequence_messages_sent_ = 0;
    bool sequence_will_play_ = false;

    // How many play and stop messages the audio thread has acted on, and what
    // it last told about where the sequence is.
    std::uint64_t sequence_messages_handled_ = 0;
    std::atomic<std::uint64_t> shown_sequence_position_{Sequencer::not_playing};
    std::atomic<std::uint64_t> shown_sequence_handled_{0};

    // The same for the sample sets: how many instrument and kit messages
    // got into the mailbox, how many the audio thread has acted on, and
    // what it last told about the sets it plays.
    std::uint64_t sample_messages_sent_ = 0;
    std::uint64_t sample_messages_handled_ = 0;
    std::atomic<std::uint32_t> shown_sets_in_use_{0};

    // What the limiter, and each slot's effect, last told about how far it
    // turned the sound down.
    std::atomic<float> shown_limiter_reduction_{0.0f};
    std::array<std::array<std::atomic<float>, Chain::slot_count>, Mixer::chain_count> shown_reductions_{};
    std::atomic<std::uint64_t> shown_sample_handled_{0};
};
