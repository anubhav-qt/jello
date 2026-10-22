#pragma once

#include <atomic>
#include <cstdint>

#include "click.h"
#include "clock.h"
#include "looper.h"
#include "mailbox.h"
#include "song_player.h"
#include "synth.h"

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
// the synth and the looper, gives Compose's song to the song player, plays
// the click on the beat, and puts the sum through the soft clip, the last
// step before the speakers.
class Conductor {
public:
    explicit Conductor(int sample_rate);

    // These only put a message in the mailbox. The audio thread acts on it at
    // the start of its next chunk of sound. Only one thread may call them at
    // a time.
    void note_on(int note, int velocity);
    void note_off(int note);
    void all_notes_off();
    // Oscillator `index`, from 0 (Synth::set_oscillator).
    void set_oscillator(int index, const OscillatorSettings& oscillator);
    // Starts the click on beat 1 of a bar. Does nothing if it is already on.
    void start_click();
    // Stops the click. A tick already sounding rings out.
    void stop_click();
    // Plays the beat at `tempo` beats per minute from the next beat on.
    // Ignored while the looper is busy, so a loop keeps its beat.
    void set_tempo(int tempo);
    // Right Shift (Looper::press), and clearing the loop (Looper::clear).
    void press_looper();
    void clear_loop();
    // Esc (Looper::remove_layer), and a layer's volume knob
    // (Looper::set_layer_volume).
    void remove_layer(int layer);
    void set_layer_volume(int layer, float volume);
    // The "snap to beat" switch (Looper::set_snap).
    void set_snap(bool snap);
    // The filter's controls (Synth::set_filter), and the filter envelope
    // (Synth::set_filter_envelope).
    void set_filter(const FilterSettings& settings);
    void set_filter_envelope(const FilterEnvelopeSettings& envelope);
    // Unison's copies (Synth::set_unison).
    void set_unison(const UnisonSettings& unison);
    // The loudness envelope, the key tracking and the tuning
    // (Synth::set_loudness_envelope, set_key_tracking and set_tuning).
    void set_loudness_envelope(const LoudnessEnvelopeSettings& envelope);
    void set_key_tracking(float key_tracking);
    void set_tuning(float a4_hz);
    // The routing list's LFOs and connections, and how long the vibrato key
    // takes to rise and fall (Synth::set_lfo, set_connection and
    // set_vibrato_key_times).
    void set_lfo(int index, const LfoSettings& settings);
    void set_connection(int index, const Connection& connection);
    void set_vibrato_key_times(int rise_ms, int fall_ms);
    // Holds or lets go of the live vibrato key (Synth::set_vibrato_key). The
    // looper records it like a note.
    void set_vibrato_key(bool held);

    // What the looper is doing. Only the thread that sends the messages may
    // ask.
    LooperStatus looper_status() const;

    // Takes out the oldest change to the loop's notes. Returns false if none
    // is waiting. Only the thread that sends the messages may ask.
    bool next_loop_change(LoopChange& change);

    // Plays `chords`, `count` of them, one after another, each for its own
    // length, from the start of the next chunk
    // (docs/adr/0022-compose-as-a-tape.md). Whatever the song was playing
    // stops first. Returns false and sends nothing if the mailbox has no room
    // for all of it, so the audio thread never plays half a song.
    bool play_song(const SongChord* chords, int count);

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
    // Acts on every message waiting in the mailbox, oldest first.
    void take_messages();

    // Does what one message asks.
    void handle(const Event& event);

    // Right Shift's press, on the audio thread.
    void handle_looper_press();

    // True while someone needs to hear the beat.
    bool wants_click() const;

    // Stops the beat once neither the click nor the looper needs it.
    void stop_beat_if_unused();

    // Tells the screen what the looper is doing, and how far the song has
    // played, at the end of each chunk.
    void publish_looper_status();
    void publish_song_position();

    EventMailbox mailbox_;

    // The loop's changes, on their way to Rust. The looper sends them, so
    // this must be made before the looper, below.
    LoopChanges loop_changes_;

    Synth synth_;
    Clock clock_;
    Click click_;
    Looper looper_;
    SongPlayer song_player_;

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
};
