# 0028: A low-pass filter

- Status: Accepted; values by ear and fixed key tracking superseded by 0030; extended with high-pass, band-pass and notch types (design.md, filter types)
- Date: 2026-10-21

Step 4 starts with a low-pass filter on every voice, before the loudness
envelope: a 12 dB per octave state variable filter, built the
zero-delay-feedback way. Its cutoff goes from 20 Hz up to 20,000 Hz, which
means open: the filter is off and the sound is exactly as it was before.
The cutoff is set for middle C, and every other note's follows it, an
octave up for each octave up, so every note is equally bright. Resonance
goes from no bump (Q 0.71) to a strong peak (Q 10). Every note hears a
moved cutoff or resonance, even notes already sounding, and it glides there
over 20 ms so it never clicks. A filter envelope (attack, decay, sustain,
release) opens each note's cutoff by up to 8 octaves; a change to it
reaches the notes started after. A new engine starts open, with no
envelope. The filter belongs to the instrument Compose plays. No screen
sets it yet, and its values get picked by ear once one does. Details are in
[design.md](../design.md#engine).
