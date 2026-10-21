# 0022: Compose as a tape

- Status: Accepted
- Date: 2026-10-20

Compose is written by feel, not on the beat. Each chord in the song keeps
the length it was held, and Compose never snaps it to the beat (the
looper's snap, ADR 0021, is separate). The song is drawn as a tape: one
block per chord, as wide as it was held, in its root's note color, with
its name and its length in seconds. Every block is at least as wide as its
name, so the name is never cut off. A chord still being held grows at the
end as a dashed block. Clicking a block plays that chord for its length,
and a play button plays the whole song. The key is picked from two rows of
12, then folds into a readout. Details are in
[design.md](../design.md#compose-screen).
