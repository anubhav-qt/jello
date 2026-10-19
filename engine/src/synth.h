#pragma once

#include <cstdint>
#include <vector>

#include "mailbox.h"
#include "voice.h"

// The instrument as a whole. Other threads send it note events, and the audio
// thread turns them into sound (docs/adr/0009-how-notes-become-sound.md).
// It has 32 voices, so up to 32 notes sound at once.
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
    // (left, right, left, ...), every sample between -1.0 and 1.0. Runs on
    // the audio thread, so it never locks, allocates or waits.
    void render(float* out, int frames);

private:
    // Does what one event asks.
    void handle(const Event& event);

    // Starts `note` on a voice.
    void play(int note, int velocity);

    // The voice still sounding `note`, held or fading, or nullptr if none is.
    Voice* find_ringing_voice(int note);

    // A silent voice, or nullptr if every voice is busy.
    Voice* find_free_voice();

    // The voice fading out that is quietest right now, or nullptr if no voice
    // is fading.
    Voice* find_quietest_fading_voice();

    // The voice whose note was pressed longest ago. Only asked when every
    // voice is held, so that is the note held the longest.
    Voice* find_oldest_voice();

    Mailbox mailbox_;

    // Made once, when the synth is created, and never resized, so the audio
    // thread never allocates.
    std::vector<Voice> voices_;

    // How many notes have been pressed so far. Each press gets the next
    // number, so a smaller number means pressed earlier. A counter, not a
    // clock: two notes pressed in the same chunk still get different numbers.
    std::uint64_t press_count_ = 0;
};
