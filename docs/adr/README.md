# Architecture decision records

One file per decision, numbered in order. An accepted ADR is never rewritten.
To change a decision, write a new ADR and mark the old one as
"Superseded by NNNN".

## Index

| # | Decision | Status |
|---|---|---|
| [0001](0001-three-layers.md) | Three layers: webview, Rust core, C++ engine | Accepted |
| [0002](0002-one-note-event.md) | One note event for every input | Accepted |
| [0003](0003-every-action-is-a-command.md) | Every action is a named command | Accepted |
| [0004](0004-songs-are-text.md) | Songs are text files | Accepted |
| [0005](0005-cpu-for-audio-gpu-for-visuals-and-ai.md) | CPU for audio, GPU for visuals and AI | Accepted |
| [0006](0006-macos-and-windows.md) | macOS and Windows from day one | Accepted |
| [0007](0007-gpl-3-license.md) | License Jello under GPL-3.0 | Accepted |
| [0008](0008-miniaudio-for-audio-devices.md) | miniaudio for audio devices | Accepted |
| [0009](0009-how-notes-become-sound.md) | How notes become sound | Accepted |

## Template

```markdown
# NNNN: Title

- Status: Proposed | Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context

Why this decision is needed.

## Decision

What we decided, in plain words.

## Consequences

What gets easier, what gets harder, what we must remember.
```
