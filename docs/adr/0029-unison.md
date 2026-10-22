# 0029: Unison

- Status: Accepted
- Date: 2026-10-22

Every note can be played in unison: by up to 7 copies of its wave at once,
which makes the saw a supersaw. It works on every wave, so the wave switch
stays as it is. Detune, from 0 to 50 cents, tunes the highest copy that far
above the note and the lowest as far below, with the rest evenly between.
Spread, from 0 to 100%, places them between the speakers, the lowest copy
left and the highest right, each as loud wherever it sits. The copies'
starting points are spread unevenly when the engine starts, so they never
start lined up, and each carries on from where it was, so every note starts
a little differently. Their sum is turned down by the square root of the
copies, so a note stays about as loud. Voices become truly stereo, so each
voice's filter becomes one per speaker, moved together. A change reaches
the notes started after it, like the wave. A new engine starts at 1 copy,
which is off: notes sound exactly as before. Values are picked by ear in
the dev panel, which becomes "Dev: sound". Details are in
[design.md](../design.md#engine).
