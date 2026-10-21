# 0021: A looper on a beat

- Status: Accepted
- Date: 2026-10-20

Right Shift is a looper, in every tab. Loops follow a beat: 120 beats per
minute to start, `-` and `=` change it by 5, and a bar has 4 beats. The
first press counts in one bar of clicks, then records from beat 1. The
second press closes the loop at the nearest bar line, so a loop is always a
whole number of bars, and it repeats without the click while the player
plays live over it. The third press records a new layer on top, the fourth
stops. Esc clears the loop, and the tempo is locked while a loop exists.
A loop is notes, not sound, kept in the C++ engine and played at their
exact sample. Recorded notes snap to the nearest half beat, by a "snap to
beat" toggle that is on by default. Live sound is never moved. Details are
in [design.md](../design.md#looper).
