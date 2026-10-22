# 0034: Best next chords

- Status: Accepted
- Date: 2026-10-22

Compose's guidance now ranks what comes next. After each chord added to the
song, up to three chord keys become the best next, strongest first, each
with a plain reason; before the first chord, the best starts. So the chord
keys have four levels: best next, in the key, can work (0017's two levels,
unchanged) and outside, dimmed. A best next chord is always in the key or
can work, never outside. The rules look only at the song's last chord: a
table in `src-tauri/src/harmony.rs` lists, for every chord the key's
guidance knows, its best next chords. They were picked where three ideas
agree: each chord's job (home, away or tension), how far the root moves (a
fifth down pulls hardest) and what songs do (counts from a rock corpus and
Hooktheory, J-pop's royal road). After a chord outside the key, home is
best. Trying a chord never changes the ranking; adding or deleting one
does. This supersedes 0017's guidance following the key only. The look is
designed next. Details are in [design.md](../design.md#chord-guidance).
