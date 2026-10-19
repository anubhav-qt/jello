#pragma once

#include <array>
#include <atomic>
#include <cstdint>

#include "wave.h"

// What another thread can ask the audio thread to do.
enum class EventType {
    note_on,
    note_off,
    all_notes_off,
    set_wave,
};

// One message for the audio thread.
struct Event {
    EventType type;
    int note;      // which key, 0 to 127; middle C is 60
    int velocity;  // how hard the key was hit, 1 to 127; only for note_on

    // Which wave notes sent after this one play; only for set_wave. It has a
    // starting value, so the other messages can leave it out.
    Wave wave = Wave::triangle;
};

// Carries events to the audio thread without locks
// (docs/adr/0009-how-notes-become-sound.md). Only one thread may send at a
// time, and only the audio thread may receive.
class Mailbox {
public:
    // Puts an event in the mailbox. If the mailbox is full, the event is
    // dropped.
    void send(const Event& event);

    // Takes out the oldest waiting event. Returns false if none is waiting.
    // Runs on the audio thread, so it never locks, allocates or waits.
    bool receive(Event& event);

private:
    static constexpr std::uint64_t slot_count = 1024;

    // Some machines hide a lock inside an atomic. This stops the build there,
    // so the audio thread can never end up waiting on one (design rule 2).
    static_assert(std::atomic<std::uint64_t>::is_always_lock_free,
                  "the mailbox needs lock-free 64-bit atomics");

    std::array<Event, slot_count> slots_{};

    // Both counters only ever go up. Event number n lives in slot
    // n % slot_count, so after the last slot it wraps around to slot 0.
    std::atomic<std::uint64_t> written_{0};  // how many events were ever put in
    std::atomic<std::uint64_t> read_{0};     // how many were ever taken out
};
