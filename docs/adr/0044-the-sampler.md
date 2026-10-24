# 0044: The sampler

- Status: Accepted
- Date: 2026-10-24

Jello plays recordings, not only waves. Each oscillator gains a sixth kind,
sample, which plays an instrument through the whole voice: filter,
envelopes, LFOs, unison, glide and vibrato, layered with waves. A drum kit
plays beside the synth with voices of its own: right Option (right Ctrl on
Windows) switches the letters to its 16 pads, each hit plays once to its
end, and choke groups let a closed hat stop an open one. Drum hits are notes
130 to 145, so the looper and song files take them like any note, saved by
pad name.

Instruments are folders in Music/Jello/Instruments: a subset of SFZ, or WAV
and FLAC files named by note or drum. Rust reads them in the background and
sends the engine finished recordings. Nothing comes with Jello: two free
starters (Salamander Grand Piano, CC BY 3.0; AVL Black Pearl drums, CC BY-SA
3.0) are fetched only when the player clicks their Download button. This
answers ADR 0019's open question of where recordings come from. Details in
[design.md](../design.md#sampler).
