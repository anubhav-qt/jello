#pragma once

#include <cstdint>
#include <limits>

#include "mailbox.h"

// One message for the sequencer: an ordinary message for the audio thread,
// due `sample` samples after sequence `sequence` starts playing.
struct TimedEvent {
    std::uint64_t sequence = 0;
    std::uint64_t sample = 0;
    Event event{};
};

// The mailbox that carries timed messages to the sequencer. Rust keeps it
// about 2 seconds ahead of the sound, so a song of any length never needs all
// its notes in memory at once.
using TimedEvents = Mailbox<TimedEvent, 8192>;

// Plays the arrangement's timed messages (docs/design.md#tracks-in-the-engine),
// each on its own sample. Sequences are numbered: a stop drops every message
// of the sequences up to it still waiting, and a message of a later sequence
// waits for that sequence to play. Only the audio thread uses it, so nothing
// in here locks, allocates or waits.
class Sequencer {
public:
    // What position says while nothing plays.
    static constexpr std::uint64_t not_playing = std::numeric_limits<std::uint64_t>::max();

    // The sequencer takes its messages from `events`.
    explicit Sequencer(TimedEvents& events);

    // Starts sequence `sequence` on sample `now`. Messages of the sequences
    // before it still waiting are dropped.
    void play(std::uint64_t sequence, std::uint64_t now);

    // Stops sequence `sequence` and every one before it, and drops their
    // messages still waiting.
    void stop(std::uint64_t sequence);

    // True while a sequence plays.
    bool is_playing() const;

    // Takes the next message due by sample `now` into `event` and returns
    // true, or returns false if none is due yet. A message that came late
    // is due at once.
    bool next_due(std::uint64_t now, Event& event);

    // How many samples the sequence playing has played by sample `now`, or
    // not_playing.
    std::uint64_t position(std::uint64_t now) const;

private:
    // Takes the oldest waiting message into next_ if it holds none, and
    // drops those of stopped sequences. Returns false if none is waiting.
    bool look_ahead();

    TimedEvents& events_;

    // The oldest message taken out and not played yet.
    bool has_next_ = false;
    TimedEvent next_{};

    bool playing_ = false;
    std::uint64_t sequence_ = 0;         // the sequence playing, or played last
    std::uint64_t stopped_through_ = 0;  // every sequence up to this one is over
    std::uint64_t start_ = 0;            // the sample it started on
};
