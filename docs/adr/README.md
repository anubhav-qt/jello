# Architecture decision records

One file per decision, numbered in order. From 0011 on, an ADR is a short
brief of a few lines. An accepted ADR is never rewritten.
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
| [0009](0009-how-notes-become-sound.md) | How notes become sound | Accepted; fixed envelope superseded by 0030, Ctrl as a shortcut key by 0032, one wave per note by 0035 |
| [0010](0010-stage-look-for-the-start-page.md) | Stage look for the start page | Accepted; colors superseded by 0014 |
| [0011](0011-svelte-5-for-the-ui.md) | Svelte 5 for the UI | Accepted |
| [0012](0012-chords-left-melody-right.md) | Chords on the left, melody on the right | Accepted |
| [0013](0013-highlight-next-chords-on-the-existing-layout.md) | Highlight next chords on the existing layout | Accepted; guidance groups superseded by 0017 |
| [0014](0014-one-color-per-note-family.md) | One color per note family | Accepted |
| [0015](0015-minor-on-space-sharp-on-right-shift.md) | Minor on Space, sharp on right Shift | Superseded by 0020 |
| [0016](0016-play-compose-and-produce-tabs.md) | Play, Compose and Produce tabs | Accepted |
| [0017](0017-guided-compose.md) | Guided Compose | Accepted; following the key only superseded by 0034 |
| [0018](0018-ai-harness-on-a-fork-of-pi.md) | AI harness on a fork of pi | Accepted |
| [0019](0019-instruments-built-from-scratch-no-guitar.md) | Instruments built from scratch, no guitar | Accepted |
| [0020](0020-every-chord-on-its-own-key.md) | Every chord on its own key | Accepted; F#m on left Shift superseded by 0032 |
| [0021](0021-a-looper-on-a-beat.md) | A looper on a beat | Accepted; Esc superseded by 0026 |
| [0022](0022-compose-as-a-tape.md) | Compose as a tape | Accepted |
| [0023](0023-guidance-on-the-melody-keys.md) | Guidance on the melody keys | Accepted |
| [0024](0024-the-looper-s-look.md) | The looper's look | Accepted; lane superseded by 0025 |
| [0025](0025-the-looper-s-lane-as-wide-as-the-keyboard.md) | The looper's lane, as wide as the keyboard | Accepted |
| [0026](0026-layers.md) | Layers | Accepted |
| [0027](0027-computer-keyboards-only.md) | Computer keyboards only | Accepted |
| [0028](0028-a-low-pass-filter.md) | A low-pass filter | Accepted; values by ear and fixed key tracking superseded by 0030 |
| [0029](0029-unison.md) | Unison | Accepted; values by ear superseded by 0030 |
| [0030](0030-a-sound-profile-per-song.md) | A sound profile per song | Accepted; also holds the loudness envelope, key tracking and tuning |
| [0031](0031-lfos-and-a-routing-list.md) | LFOs and a routing list | Accepted; the wave target superseded by the oscillators' kinds (0035) |
| [0032](0032-the-vibrato-key.md) | The vibrato key | Accepted |
| [0033](0033-the-sound-panel.md) | The sound panel | Accepted; the wave tiles moved to an Oscillators page by 0035 |
| [0034](0034-best-next-chords.md) | Best next chords | Accepted |
| [0035](0035-oscillators-with-kinds.md) | Oscillators with kinds | Accepted |

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
