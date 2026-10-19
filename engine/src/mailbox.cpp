#include "mailbox.h"

void Mailbox::send(const Event& event) {
    std::uint64_t written = written_.load();
    std::uint64_t read = read_.load();

    // Full: every slot holds an event the audio thread has not taken yet.
    // The new event is dropped.
    if (written - read == slot_count) {
        return;
    }

    slots_[written % slot_count] = event;

    // Only now count it, so the audio thread never sees a half-written slot.
    written_.store(written + 1);
}

bool Mailbox::receive(Event& event) {
    std::uint64_t read = read_.load();
    std::uint64_t written = written_.load();

    if (read == written) {
        return false;  // nothing is waiting
    }

    event = slots_[read % slot_count];

    // Only now count it, so the writer never reuses a slot still being read.
    read_.store(read + 1);
    return true;
}
