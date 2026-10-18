# 0002: One note event for every input

- Status: Accepted
- Date: 2026-10-18

## Context

Notes come from the laptop keyboard first, then a MIDI keyboard, and later
from AI. The synth, the follow-along scoring and the UI should not care which.

## Decision

Every input is turned into the same note event in Rust (note, velocity, time,
source) as early as possible.

## Consequences

- Plugging in the real keyboard, or letting an AI play, changes only the code
  that creates events.
- The laptop keyboard has no velocity, so its events use a fixed velocity.
