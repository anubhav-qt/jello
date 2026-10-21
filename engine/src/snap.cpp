#include "snap.h"

#include <algorithm>
#include <array>
#include <limits>

namespace {

// How many notes MIDI has, 0 to 127.
constexpr int pitch_count = 128;

// Which half beat `position` is nearest to, counting the loop's first sample
// as half beat 0.
std::uint64_t nearest_half_beat(std::uint64_t position, BeatLength beat) {
    // Half beats are samples times half beats per minute, over samples per
    // minute. Adding half of the divisor first makes the division round to
    // the nearest whole half beat instead of down.
    std::uint64_t half_beats_per_minute = 2 * beat.tempo;
    std::uint64_t rounding = beat.samples_per_minute / 2;
    return (position * half_beats_per_minute + rounding) / beat.samples_per_minute;
}

// The sample half beat `half_beat` starts on, rounded down to a whole
// sample.
std::uint64_t half_beat_start(std::uint64_t half_beat, BeatLength beat) {
    std::uint64_t half_beats_per_minute = 2 * beat.tempo;
    return half_beat * beat.samples_per_minute / half_beats_per_minute;
}

// True if `note` starts nearer the loop's end than any half beat before it,
// so snapping moves it round to beat 1.
bool moves_round(const LoopNote& note, std::uint64_t length, BeatLength beat) {
    return nearest_half_beat(note.start, beat) >= nearest_half_beat(length, beat);
}

// `note` moved to the nearest half beats.
LoopNote snapped(const LoopNote& note, std::uint64_t length, BeatLength beat) {
    LoopNote moved = note;

    if (moves_round(note, length, beat)) {
        moved.start = 0;
        moved.end = half_beat_start(1, beat);
        return moved;
    }

    // At least half a beat, so a quick tap never ends where it starts.
    // Half beats start on the whole sample before, so the last one, the
    // loop's end, never falls after the end the looper keeps.
    std::uint64_t start = nearest_half_beat(note.start, beat);
    std::uint64_t end = nearest_half_beat(note.end, beat);
    if (end < start + 1) {
        end = start + 1;
    }
    moved.start = half_beat_start(start, beat);
    moved.end = half_beat_start(end, beat);
    return moved;
}

// Whether note `a` starts before note `b`. Notes that start together go
// from the one that ends first, then from the lowest, then from the
// quietest, so the plan never depends on the order they were played in.
bool starts_sooner(const LoopNote& a, const LoopNote& b) {
    if (a.start != b.start) {
        return a.start < b.start;
    }
    if (a.end != b.end) {
        return a.end < b.end;
    }
    if (a.note != b.note) {
        return a.note < b.note;
    }
    return a.velocity < b.velocity;
}

// Cuts each note short where the same key starts again, if it is still
// sounding then. `planned` is in the order the notes start.
void end_before_the_same_key_again(LoopNote* planned, int count) {
    // Going from the last note back: where the next note of each key starts.
    constexpr std::uint64_t none = std::numeric_limits<std::uint64_t>::max();
    std::array<std::uint64_t, pitch_count> next_start{};
    next_start.fill(none);

    for (int i = count - 1; i >= 0; i--) {
        LoopNote& note = planned[i];
        if (note.end > next_start[note.note]) {
            note.end = next_start[note.note];
        }
        next_start[note.note] = note.start;
    }
}

}  // namespace

void plan_loop(const LoopNote* played, int count, std::uint64_t length, BeatLength beat, bool snap,
               LoopNote* planned) {
    for (int i = 0; i < count; i++) {
        if (snap) {
            planned[i] = snapped(played[i], length, beat);
        } else {
            planned[i] = played[i];
        }
    }

    // A layer recorded over several times around holds its notes in the
    // order they were played, not the order they start in the loop, and
    // snapping moves some round to beat 1. Sorting puts them in the order
    // they start. It works in place, so it asks for no memory.
    std::sort(planned, planned + count, starts_sooner);

    end_before_the_same_key_again(planned, count);
}
