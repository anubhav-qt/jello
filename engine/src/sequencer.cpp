#include "sequencer.h"

Sequencer::Sequencer(TimedEvents& events) : events_(events) {}

void Sequencer::play(std::uint64_t sequence, std::uint64_t now) {
    playing_ = true;
    sequence_ = sequence;
    if (sequence > 0) {
        if (stopped_through_ < sequence - 1) {
            stopped_through_ = sequence - 1;
        }
    }
    start_ = now;
}

void Sequencer::stop(std::uint64_t sequence) {
    if (sequence == sequence_) {
        playing_ = false;
    }
    if (stopped_through_ < sequence) {
        stopped_through_ = sequence;
    }
}

bool Sequencer::is_playing() const {
    return playing_;
}

bool Sequencer::next_due(std::uint64_t now, Event& event) {
    if (!playing_) {
        return false;
    }
    if (!look_ahead()) {
        return false;
    }
    // A later sequence's message waits for it to play.
    if (next_.sequence != sequence_) {
        return false;
    }
    if (start_ + next_.sample > now) {
        return false;
    }
    event = next_.event;
    has_next_ = false;
    return true;
}

std::uint64_t Sequencer::position(std::uint64_t now) const {
    if (!playing_) {
        return not_playing;
    }
    return now - start_;
}

bool Sequencer::look_ahead() {
    while (true) {
        if (!has_next_) {
            if (!events_.receive(next_)) {
                return false;
            }
            has_next_ = true;
        }
        if (next_.sequence > stopped_through_) {
            return true;
        }
        // A stopped sequence's message is dropped.
        has_next_ = false;
    }
}
