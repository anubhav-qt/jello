# 0035: Oscillators with kinds

- Status: Accepted
- Date: 2026-10-22

Every note is played by 3 oscillators instead of one wave, added up before
the filter. Each picks a kind, plays at its own level (0 to 100%) and is
tuned its own distance from the note: up to 3 octaves, 12 half steps and
100 cents either way. The kinds are the four waves and noise, a new random
height every sample, which has no pitch. Later kinds, like wavetables, FM
and plucked strings, join the same list instead of rebuilding the voice.
Every unison copy plays all three, and each copy's noise is its own, so
spread makes the hiss wide. A copy's oscillators start their cycles
together. An oscillator at 0% is skipped, and a new song starts with one
triangle at 100% and two silent ones, so it sounds exactly as before. The
routing list gains each oscillator's level and pitch (live) and kind (read
as a note starts), which replaces the wave target; snap works on every
pitch. The sound panel opens on a new Oscillators page, which takes the
wave tiles from Notes, and the \ key steps the first oscillator's kind.
Details are in [design.md](../design.md#engine).
