# 0008: miniaudio for audio devices

- Status: Accepted
- Date: 2026-10-18

## Context

The C++ engine has to hand its sound to the operating system: CoreAudio on
macOS and WASAPI on Windows (ADR 0006). Writing that code by hand for each
system is slow and easy to get wrong. The options were miniaudio, RtAudio,
PortAudio and JUCE (a whole framework, far more than we need).

## Decision

Use miniaudio (version in `engine/third_party/README.md`), copied into the
repo as one file.

- It is a single file with no dependencies, public domain or MIT-0, so it is
  GPL-compatible (ADR 0007).
- We only use its device layer. Its decoders, generators and mixing engine
  are switched off in `engine/src/miniaudio_settings.h`; Jello makes its own
  sound.
- On macOS it links Apple's audio frameworks directly instead of loading
  them at runtime, which Apple's notarization needs.
- All device code lives in `engine/src/engine.cpp`.

## Consequences

- One small, readable file to update, and the engine builds the same on both
  platforms.
- miniaudio has no ASIO. If WASAPI is not fast enough once an audio interface
  arrives, ASIO gets added next to it in `engine.cpp`, without touching the
  rest of the engine.
