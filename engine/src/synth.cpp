#include "synth.h"

Synth::Synth(int sample_rate)
    : voice_(sample_rate) {}

void Synth::note_on(int note, int velocity) {
    mailbox_.send(Event{EventType::note_on, note, velocity});
}

void Synth::note_off(int note) {
    mailbox_.send(Event{EventType::note_off, note, 0});
}

void Synth::all_notes_off() {
    mailbox_.send(Event{EventType::all_notes_off, 0, 0});
}

void Synth::render(float* out, int frames) {
    // Act on everything sent since the last chunk, oldest first.
    Event event{};
    while (mailbox_.receive(event)) {
        handle(event);
    }

    // Start from silence, since voices add themselves on top. Each frame is
    // two samples: left and right.
    for (int i = 0; i < 2 * frames; i++) {
        out[i] = 0.0f;
    }

    voice_.render(out, frames);
}

void Synth::handle(const Event& event) {
    if (event.type == EventType::note_on) {
        voice_.start(event.note, event.velocity);
    } else if (event.type == EventType::note_off) {
        // Only if the voice still plays this note. Another note may have
        // taken it over since.
        if (voice_.note() == event.note) {
            voice_.release();
        }
    } else if (event.type == EventType::all_notes_off) {
        voice_.release();
    }
}
