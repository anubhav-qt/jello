# 0047: Song key mappings

- Status: Accepted; supersedes the fixed layout in 0012 and 0020
- Date: 2026-10-30

No layout is fixed between songs: the keyboard arranges itself for the music,
and the screen shows which key to press, so a touch typist plays by reading,
not memorizing. A key plays any set of notes: one note or any chord voicing.

Keys play nothing until a scale is picked. A song keeps its scale; Play asks
once and remembers the last. Each of the 11 scales has a default holding its
own notes plus outside notes that still work, chosen from song data, with
leftover keys repeating the best notes in other octaves. Defaults keep chords
left and melody right, with chords placed by role.

A song has a main mapping, and a section may have its own, switching at the
section boundary. The user maps by hand on a separate page, pressing a laptop
key and clicking notes on a drawn piano. Or the agent optimizes from the
song's notes and chords: a card shows old and new keys and the comfort gain,
Try plays it unsaved, Use saves it. Locked keys never move, and nothing
changes during playback. The score starts from layout-analyzer rules and
learns from the player's own timing and misses, stored only on their machine.

Older songs are not supported; they were placeholders. Details and research
are in [design.md](../design.md#song-key-mappings).
