# 0048: The grand piano

- Status: Accepted; extends 0044, changes Space from 0047's spare chord keys
- Date: 2026-10-31

A piano needs more than one recording per note. Jello now plays what makes a
sampled grand sound like one, on keyboards that can't feel how hard a key is
hit.

**Sustain pedal.** Space, under the thumbs, holds it: a note whose key comes up
keeps ringing until the pedal goes up, on any sound. Space was a spare key
holding a leftover chord voicing, so a mapping no longer uses it, and a song
that put notes there drops them. In drum mode it stays the kick. The looper
records the pedal like the vibrato and glide keys, as pitch 146.

**Touch.** Each sample oscillator has a touch, 1 to 127, kept with the song's
sound: how hard its keys are struck, which picks the recording. We keep 6 of a
note's velocity layers, spread evenly, since each of a piano's is about 135 MB
and the keys can only play one touch at a time.

**Sounds around the notes.** SFZ release regions play as a key comes up, or as
the pedal lifts, with `rt_decay` and `amp_veltrack`. The pedal's own recordings
play as it moves. They play beside the voices, not through the filter or the
envelope, so they never take a note's voice.

**Lead-ins.** A controller the file labels "offset" is read at its full value,
so a chord's notes speak together and right as the key goes down.

**Preset.** Fetching the piano adds a Grand Piano preset: a place to start, with
every value still the song's.

More voices follow: 64, so a pedal can hold big chords. When all are busy, a new
note takes one only the pedal holds before one whose key is down.

Left out: true round robin, half pedal, sympathetic resonance and a routing
target for touch. Details: [design](../design.md#sampler).
