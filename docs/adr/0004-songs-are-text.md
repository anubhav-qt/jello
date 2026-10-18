# 0004: Songs are text files

- Status: Accepted
- Date: 2026-10-18

## Context

The user wants an AI to compose by writing a "code based sequence" of notes,
sounds and mix settings that Jello turns into music.

## Decision

- A Jello song is a readable text file: tracks, notes, sounds, mix and
  automation. Rust compiles it into timed events for the C++ engine.
- Offline rendering is deterministic: the same file always gives the same
  audio.
- A scripting layer that generates song files can come later (v3).
- The exact format is decided at v1 step 5.

## Consequences

- Songs can be read by humans, diffed and kept in git, and written by AI.
- The engine must be able to render without an audio device, which WAV export
  needs anyway.
