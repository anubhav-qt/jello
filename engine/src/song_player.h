#pragma once

#include <array>
#include <cstdint>
#include <limits>

#include "synth.h"

// One chord of a song: its notes, how hard they are hit, and how long it
// plays.
struct SongChord {
    // The most notes a chord can have. Today's chords have 3, so there is
    // room for richer ones later.
    static constexpr int max_notes = 4;

    std::array<int, max_notes> notes;  // the first note_count of them
    int note_count;
    int velocity;
    std::uint64_t length;  // how many samples it plays
};

// Plays Compose's song (docs/adr/0022-compose-as-a-tape.md). It is given a
// queue of chords and plays them one after another, each for its own length,
// with no gap between them, the way the tape keeps them. It ignores the beat
// and the tempo: Compose is timed in real seconds. Its notes are the song's
// own (Source::song), so live notes and the loop never cut them off, and the
// looper never records them. Only the audio thread uses it, so nothing in
// here locks, allocates or waits.
class SongPlayer {
public:
    // The most chords the queue holds. The room for them is set aside when
    // the engine is created, so playing never asks for more memory. Chords
    // added once it is full are left out.
    static constexpr int max_chords = 512;

    // What position says while no song is playing.
    static constexpr std::uint64_t not_playing = std::numeric_limits<std::uint64_t>::max();

    // Lets go of the chord playing and empties the queue.
    void stop(Synth& synth);

    // Adds `chord` to the end of the queue.
    void add(const SongChord& chord);

    // Starts the queue's first chord on sample `now`. The queue must not be
    // playing already: stop it first.
    void play(std::uint64_t now, Synth& synth);

    // True when the chord playing ends on sample `now`.
    bool has_something_now(std::uint64_t now) const;

    // Ends the chord playing on sample `now` and starts the next one there.
    void act_now(std::uint64_t now, Synth& synth);

    // How many samples the song has played by sample `now`, counted from its
    // first chord's start, or not_playing.
    std::uint64_t position(std::uint64_t now) const;

private:
    // The sample the chord playing ends on.
    std::uint64_t chord_end() const;

    // Ends every chord that ends on sample `now`, and starts the one after
    // each. More than one when a chord is 0 samples long.
    void play_due(std::uint64_t now, Synth& synth);

    // Starts, or lets go of, the notes of the chord playing.
    void start_chord(Synth& synth);
    void let_go_of_chord(Synth& synth);

    std::array<SongChord, max_chords> chords_{};
    int chord_count_ = 0;

    bool playing_ = false;
    std::uint64_t song_start_ = 0;   // the sample the first chord started on
    int chord_playing_ = 0;          // its place in chords_
    std::uint64_t chord_start_ = 0;  // the sample it started on
};
