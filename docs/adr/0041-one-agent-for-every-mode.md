# 0041: One agent for every mode

- Status: Accepted
- Date: 2026-10-23

One agent harness on pi works as Tutor in Play, Composer in Compose and
Producer in Produce. Only its instructions change with the mode; its tools
and its conversation stay. It can do everything the player can: change any
sound value, play, record, make loops, edit and open songs. This supersedes
0040's observation-only Tutor and its one pi process per question.

Its tools come from one tool server in Jello's Rust core, run by the same
code as the UI's commands, grouped by area: look, sound, play, looper,
song, presets, practice and listen. It plays timed phrases of key presses
through the same path as the player's keys. It acts without asking, except
to save over a preset or change the starting sound, and every reply can be
undone. It checks its work by measuring silent renders. Its panel docks on
the right in every mode. Jello ships its own Node and pi, and sign-in
happens in the panel. Details are in
[design.md](../design.md#the-agent-harness).
