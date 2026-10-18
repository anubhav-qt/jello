# 0001: Three layers: webview, Rust core, C++ engine

- Status: Accepted
- Date: 2026-10-18

## Context

Jello needs a modern UI, solid app logic and real-time audio with very low
latency. The user wants Tauri with Rust for the app and C++ for the sound,
partly to learn C++ audio programming.

## Decision

- **Webview (TypeScript):** draws the UI and forwards laptop key presses.
  No music logic.
- **Rust core:** commands, input, song files, follow-along clock and scoring,
  file I/O.
- **C++ engine:** everything that makes sound, including the sequencer, so
  notes land on the exact audio sample.
- The live path (key or MIDI to sound) never waits on the webview. MIDI enters
  in Rust (WebKit on macOS has no Web MIDI anyway).
- The audio callback never locks, allocates, logs or waits.
- Rust talks to C++ through a small, plain C API.

## Consequences

- Three languages means two boundaries to maintain (TypeScript to Rust,
  Rust to C++). Keeping the C API small keeps that cost low.
- The C++ engine can be tested and run without the UI.
