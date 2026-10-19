#pragma once

#include "mailbox.h"
#include "voice.h"

// The instrument as a whole. Other threads send it note events, and the audio
// thread turns them into sound (docs/adr/0009-how-notes-become-sound.md).
// For now it has one voice. Slice 3 gives it 32.
class Synth {
public:
    explicit Synth(int sample_rate);

    // These only put an event in the mailbox. The audio thread acts on it at
    // the start of its next chunk of sound. Only one thread may call them at
    // a time.
    void note_on(int note, int velocity);
    void note_off(int note);
    void all_notes_off();

    // Acts on every waiting event, then fills `frames` frames of stereo
    // (left, right, left, ...). Runs on the audio thread, so it never locks,
    // allocates or waits.
    void render(float* out, int frames);

private:
    // Does what one event asks.
    void handle(const Event& event);

    Mailbox mailbox_;
    Voice voice_;
};
