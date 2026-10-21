# 0023: Guidance on the melody keys

- Status: Accepted
- Date: 2026-10-21

In Compose, once a key is picked, the melody keys are guided too, but
quietly: they get no outline, so the chord keys' outlines still stand out.
A melody key whose note is outside the song's key dims, like an outside
chord, and still plays. While chord keys are held, the melody keys that
play those chords' notes fill with their color, even outside the key: G#
fills while E is held in C major. The fill comes at once and fades out
slowly when the chord is let go. A major key has 7 notes. A minor key has
8: its own 7 and the raised note below home (G# in A minor), the note its
major five chord is built on, so the melody keys agree with the chord
keys. The rule is in `src-tauri/src/harmony.rs`. Details are in
[design.md](../design.md#chord-guidance).
