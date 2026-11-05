#include "loop_plan.h"

#include <algorithm>
#include <array>
#include <limits>

namespace {

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
    std::array<std::uint64_t, loop_pitch_count> next_start{};
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

void plan_loop(const LoopNote* played, int count, LoopNote* planned) {
    for (int i = 0; i < count; i++) {
        planned[i] = played[i];
    }

    // A layer recorded over several times around holds its notes in the
    // order they were played, not the order they start in the loop.
    // Sorting puts them in the order they start. It works in place, so it
    // asks for no memory.
    std::sort(planned, planned + count, starts_sooner);

    end_before_the_same_key_again(planned, count);
}
