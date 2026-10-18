# 0005: CPU for audio, GPU for visuals and AI

- Status: Accepted
- Date: 2026-10-18

## Context

The desktop has a Ryzen 7 9800X3D and an RTX 5070, and the user wants both
used well.

## Decision

- Live audio runs on the CPU only. Every buffer must be ready within a few
  milliseconds, and a GPU round trip adds delay and jitter; one late buffer is
  an audible click.
- The engine processes audio in small blocks with plain loops so the compiler
  can vectorize them. Tracks stay independent so they can later run on
  separate cores.
- The GPU is for visuals and AI work (local models, transcription, pitch
  analysis, voice conversion).
- A system check measures the machine (benchmark plus latency test) instead
  of trusting specs alone, and sets limits from the results.

## Consequences

- No GPU code in the real-time audio path, ever.
- Simple loops stay the default; hand-written SIMD only if a benchmark proves
  it is needed.
