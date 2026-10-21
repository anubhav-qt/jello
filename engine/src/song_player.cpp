#include "song_player.h"

void SongPlayer::stop(Synth& synth) {
    synth.all_notes_off(Source::song);
    playing_ = false;
    chord_count_ = 0;
}

void SongPlayer::add(const SongChord& chord) {
    // A full queue takes no more, so the rest of the song is left out.
    if (chord_count_ == max_chords) {
        return;
    }
    chords_[chord_count_] = chord;
    chord_count_ += 1;
}

void SongPlayer::play(std::uint64_t now, Synth& synth) {
    if (chord_count_ == 0) {
        return;  // nothing to play
    }

    playing_ = true;
    song_start_ = now;
    chord_playing_ = 0;
    chord_start_ = now;
    start_chord(synth);

    // The conductor has already asked about this sample, so a first chord 0
    // samples long ends here.
    play_due(now, synth);
}

bool SongPlayer::has_something_now(std::uint64_t now) const {
    if (!playing_) {
        return false;
    }
    return chord_end() <= now;
}

void SongPlayer::act_now(std::uint64_t now, Synth& synth) {
    play_due(now, synth);
}

std::uint64_t SongPlayer::position(std::uint64_t now) const {
    if (!playing_) {
        return not_playing;
    }
    return now - song_start_;
}

std::uint64_t SongPlayer::chord_end() const {
    return chord_start_ + chords_[chord_playing_].length;
}

void SongPlayer::play_due(std::uint64_t now, Synth& synth) {
    // A chord lets go of its notes before the next one starts, so a note the
    // two share plays again, the way it does when one key is let go just as
    // the next is pressed.
    while (has_something_now(now)) {
        std::uint64_t end = chord_end();
        let_go_of_chord(synth);
        chord_playing_ += 1;

        if (chord_playing_ == chord_count_) {
            playing_ = false;  // the last chord is over, and the song with it
        } else {
            // On the very sample the one before ended, so the chords never
            // drift apart.
            chord_start_ = end;
            start_chord(synth);
        }
    }
}

void SongPlayer::start_chord(Synth& synth) {
    const SongChord& chord = chords_[chord_playing_];
    for (int i = 0; i < chord.note_count; i++) {
        synth.note_on(chord.notes[i], chord.velocity, Source::song);
    }
}

void SongPlayer::let_go_of_chord(Synth& synth) {
    const SongChord& chord = chords_[chord_playing_];
    for (int i = 0; i < chord.note_count; i++) {
        synth.note_off(chord.notes[i], Source::song);
    }
}
