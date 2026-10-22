#pragma once

// Unison (docs/adr/0029-unison.md): every note played by several copies of
// its wave at once, each tuned a little apart and placed between the
// speakers. They drift in and out of step, which makes the sound thick and
// shimmering. A saw in unison is the supersaw.

// The most copies a note can have.
constexpr int unison_most_copies = 7;

// The most the highest copy can be tuned above the note, and the lowest
// below it, in cents: hundredths of a half step.
constexpr float unison_most_detune_cents = 50.0f;

// Unison's controls. Changing them changes the notes started after.
struct UnisonSettings {
    // How many copies play each note, from 1 to unison_most_copies. At 1,
    // unison is off and a note sounds exactly as it would without it.
    int copies = 1;
    // How far the highest copy is tuned above the note, and the lowest
    // below it, in cents, from 0 to unison_most_detune_cents. The others
    // are spaced evenly between.
    float detune_cents = 20.0f;
    // How far apart the copies sit between the speakers, from 0 (all in
    // the middle) to 1 (the lowest fully left, the highest fully right).
    float spread = 1.0f;
};
