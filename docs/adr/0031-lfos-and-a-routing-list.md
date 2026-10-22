# 0031: LFOs and a routing list

- Status: Accepted
- Date: 2026-10-22

Every song gets 8 LFOs (low frequency oscillators: slow waves that move a
setting up and down) and a routing list of 64 connections, so the sound can
move while it plays and every song can be tweaked as deeply as it needs. A
connection takes a source (an LFO, the vibrato key, the note's height,
a random number per note, or either envelope), moves a target by a signed
amount in that target's own units, both ways or one way, and can be scaled
by a second source. Targets are every sound setting: pitch, loudness and pan
(new), the filter, unison, both envelopes and every LFO's speed move live;
the wave, the copies and the envelopes' times are read as each note starts.
Each LFO has 8 shapes, a per-note or shared switch, a speed in Hz, beats or
key presses, a start point and random spread, delay and fade-in, smoothing,
and a "once" switch. Everything is the song's own (0030), worked out every 32
samples and joined with straight lines, and a render is always the same. A
new song has vibrato ready on the vibrato key (0032) and otherwise sounds
exactly as before. Details are in [design.md](../design.md#engine).
