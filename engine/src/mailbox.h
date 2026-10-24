#pragma once

#include <array>
#include <atomic>
#include <cstdint>

#include "drum_kit.h"
#include "envelope.h"
#include "filter.h"
#include "lfo.h"
#include "oscillator.h"
#include "routing.h"
#include "sample_set.h"
#include "song_player.h"
#include "unison.h"

// What another thread can ask the audio thread to do.
enum class EventType {
    note_on,
    note_off,
    all_notes_off,
    set_oscillator,
    start_click,
    stop_click,
    set_tempo,
    press_looper,
    clear_loop,
    remove_layer,
    set_layer_volume,
    set_layer_playing,
    pause_loop,
    play_loop,
    load_loop,
    set_snap,
    stop_song,
    add_song_chord,
    play_song,
    set_filter,
    set_filter_envelope,
    set_unison,
    set_loudness_envelope,
    set_key_tracking,
    set_tuning,
    set_lfo,
    set_connection,
    set_vibrato_key,
    set_vibrato_key_times,
    set_glide_key,
    set_glide_time,
    set_instrument,
    set_drum_kit,
    set_drum_pad,
};

// One message for the audio thread.
struct Event {
    EventType type;
    int note;      // which key, 0 to 127; middle C is 60
    int velocity;  // how hard the key was hit, 1 to 127; only for note_on

    // How notes sent after this one play an oscillator; only for
    // set_oscillator, which says which in `index`, below. It has a starting
    // value, so the other messages can leave it out.
    OscillatorSettings oscillator{};

    // The beat's new tempo, in beats per minute; only for set_tempo. Like
    // `oscillator`, the other messages can leave it out.
    int tempo = 0;

    // Whether loops snap to the beat; only for set_snap. The other messages
    // can leave it out too.
    bool snap = true;

    // Which layer of the loop, from 0, or Looper::newest_layer; only for
    // remove_layer, set_layer_volume and set_layer_playing. Its new volume,
    // from 0 to 1; only for set_layer_volume. And whether it plays; only for
    // set_layer_playing.
    int layer = 0;
    float volume = 1.0f;
    bool playing = true;

    // A loaded loop: how many notes wait for it in the loaded notes'
    // mailbox, how many layers they make, and how many bars it lasts; only
    // for load_loop.
    int loaded_count = 0;
    int loaded_layers = 0;
    int loaded_bars = 0;

    // The chord to add to the song's queue; only for add_song_chord. The
    // other messages can leave it out too.
    SongChord chord{};

    // The filter's controls; only for set_filter. And the filter envelope;
    // only for set_filter_envelope. The other messages can leave them out.
    FilterSettings filter{};
    FilterEnvelopeSettings filter_envelope{};

    // Unison's controls; only for set_unison. The other messages can leave
    // them out too.
    UnisonSettings unison{};

    // The loudness envelope; only for set_loudness_envelope. The key
    // tracking, from 0 to 1; only for set_key_tracking. And A4's pitch in
    // Hz; only for set_tuning. The other messages can leave them out too.
    LoudnessEnvelopeSettings loudness_envelope{};
    float key_tracking = 1.0f;
    float a4_hz = 440.0f;

    // Which oscillator, LFO or connection, from 0, for set_oscillator,
    // set_lfo and set_connection, and the LFO's or connection's settings.
    // Whether the vibrato or glide key is held; only for set_vibrato_key and
    // set_glide_key. How long the vibrato key takes to rise and fall; only
    // for set_vibrato_key_times. And how long a glide takes; only for
    // set_glide_time. The other messages can leave them out too.
    int index = 0;
    LfoSettings lfo{};
    Connection connection{};
    bool held = false;
    int rise_ms = 0;
    int fall_ms = 0;
    int glide_ms = 0;

    // A sample set, or nullptr for none; only for set_instrument, which
    // says which oscillator in `index`, and set_drum_kit. A pad's settings;
    // only for set_drum_pad, which says which pad in `index`. The other
    // messages can leave them out too.
    const SampleSet* set = nullptr;
    DrumPad pad{};
};

// Carries messages from one thread to another without locks
// (docs/adr/0009-how-notes-become-sound.md), up to `slot_count` waiting at a
// time. Only one thread may send at a time, and only one may receive.
// Neither ever locks, allocates or waits, so the audio thread can do either:
// it receives events from Rust, and sends Rust the loop's changes.
template <typename Message, std::uint64_t slot_count>
class Mailbox {
public:
    // Puts a message in the mailbox and returns true. If the mailbox is
    // full, the message is dropped and it returns false.
    bool send(const Message& message) {
        std::uint64_t written = written_.load();
        std::uint64_t read = read_.load();

        // Full: every slot holds a message not taken out yet. The new one is
        // dropped.
        if (written - read == slot_count) {
            return false;
        }

        slots_[written % slot_count] = message;

        // Only now count it, so the receiver never sees a half-written slot.
        written_.store(written + 1);
        return true;
    }

    // How many more messages fit right now. Only the sending thread may ask.
    // The receiver only ever takes messages out, so there is at least this
    // much room until the sender sends again.
    std::uint64_t room() const {
        std::uint64_t written = written_.load();
        std::uint64_t read = read_.load();
        return slot_count - (written - read);
    }

    // Takes out the oldest waiting message. Returns false if none is
    // waiting.
    bool receive(Message& message) {
        std::uint64_t read = read_.load();
        std::uint64_t written = written_.load();

        if (read == written) {
            return false;  // nothing is waiting
        }

        message = slots_[read % slot_count];

        // Only now count it, so the sender never reuses a slot still being
        // read.
        read_.store(read + 1);
        return true;
    }

private:
    // Some machines hide a lock inside an atomic. This stops the build there,
    // so the audio thread can never end up waiting on one (design rule 2).
    static_assert(std::atomic<std::uint64_t>::is_always_lock_free,
                  "the mailbox needs lock-free 64-bit atomics");

    std::array<Message, slot_count> slots_{};

    // Both counters only ever go up. Message number n lives in slot
    // n % slot_count, so after the last slot it wraps around to slot 0.
    std::atomic<std::uint64_t> written_{0};  // how many were ever put in
    std::atomic<std::uint64_t> read_{0};     // how many were ever taken out
};

// The mailbox that carries events to the audio thread.
using EventMailbox = Mailbox<Event, 1024>;
