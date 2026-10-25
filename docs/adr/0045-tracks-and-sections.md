# 0045: Tracks and sections

- Status: Accepted
- Date: 2026-10-24

A song has up to 9 tracks. A track is an instrument: one sound of its own (a
synth or sample profile, or a drum kit) and a volume. Left Cmd and a number
(left Alt on Windows) picks a track, or makes the next one, and the keys
play the picked track. Drum mode follows the picked track, and right Option
jumps to the drums track and back. Each track holds up to 8 loop layers, so
the looper records, removes and lists the picked track's layers.

A section, like a verse or a chorus, is a named set of layers that play
together for some bars, with its own tempo, key shift and automation (a
value moving from a start to an end across it). The song is its sections in
order, and one section can be used again. Arranging happens on the Compose
screen. Rust turns the sections into timed messages, and a new sequencer in
the engine plays them on their exact samples. Old one-sound song files are
not read. Details in [design.md](../design.md#tracks-and-sections).
