# 0006: macOS and Windows from day one

- Status: Accepted
- Date: 2026-10-18

## Context

Development happens on a MacBook, but the first real music happens on a
Windows desktop.

## Decision

- Jello builds and runs on macOS and Windows from the first step.
- CI builds both platforms on every push.
- Audio: CoreAudio on macOS; WASAPI low-latency or exclusive mode on Windows.
  The audio device code stays swappable so ASIO can be added when an audio
  interface arrives.

## Consequences

- Platform differences (audio latency, webview engine, key handling) must be
  checked on both machines.
- Nothing may depend on macOS-only or Windows-only features without a
  fallback.
