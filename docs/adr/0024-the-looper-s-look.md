# 0024: The looper's look

- Status: Accepted; lane superseded by 0025
- Date: 2026-10-21

The looper shows in two places, in every tab. Right Shift itself says what
the looper is doing: four beat lights, a short state like "rec bar 2" or
"loop 2 bars", and a thin bar along its bottom edge that fills as the loop
goes round, red while recording and lavender while looping. Under the
keyboard, from below N to the end of right Shift, a lane draws the loop:
one row per layer, each note as a row of dots in its note color, and one
playhead through every row. Rows get thinner as layers are added, so every
layer stays in view, and the lane scrolls only past what fits. Layers of
one loop are shown, not separate loops of different lengths (ADR 0021).
To build it, Rust must tell the page where the beat is and the engine must
hand over the loop's notes. Details are in
[design.md](../design.md#looper).
