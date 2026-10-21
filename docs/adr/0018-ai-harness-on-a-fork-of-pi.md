# 0018: AI harness on a fork of pi

- Status: Accepted
- Date: 2026-10-20

The in-app AI harness is built on pi (earendil-works/pi, MIT, which fits
GPL-3.0), as in PDE's Seelie: a public fork of our own, carried as a git
submodule and pinned to an upstream commit, with each change to pi as one
numbered commit on top. pi's coding agent is used as it is: songs are code,
so it reads and edits song files with pi's own tools, and Jello adds tools
to render, listen and measure. The harness helps compose along the way, and
turns what the user found by trial and error into the final song. Where it
runs and how it reaches models are decided when it is built, after the
engine can play a whole song.
