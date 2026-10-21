# 0014: One color per note family

- Status: Accepted
- Date: 2026-10-20

Stage keeps its dark keyboard and dot grid, with a fixed color for each
note letter across octaves: C lavender, D peach, E mint, F butter, G pink,
A ice blue and B pearl. Sharps share their letter's color. Chord keys and
beams use the root's color; the readout colors each note. Supersedes the
amber color choice in ADR 0010; the rest of Stage's design stays in place.
The dev toggle, other themes and saved theme selection are removed. Both
development and packaged builds use these colors, without a palette name
in the app. Exact colors live in [design.md](../design.md#ui-design-process).
