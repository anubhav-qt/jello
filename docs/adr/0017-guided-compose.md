# 0017: Guided Compose

- Status: Accepted
- Date: 2026-10-20

Compose starts with picking the song's key from all 24. Trying a chord only
plays it: Return adds the chord played last to the progression, and Delete
removes the last one. Guidance has two levels, drawn on the chord keys:
chords built from the key's notes, and chords from outside it that can
work, for a named reason (a lead-in that pulls toward a chord in the key,
or a chord borrowed from the key's twin, like Fm in C major). The rest are
outside but still playable. Each chord gets a short reason. Guidance
follows the key, not the last chord, so it does not rank what comes next.
Supersedes the three explanation groups of ADR 0013; the rest of 0013
stands. Details are in [design.md](../design.md#chord-guidance).
