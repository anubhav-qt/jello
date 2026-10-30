# Jello

Jello is a desktop synthesizer and music studio, played on computer keyboards
only, no MIDI, so step 3 is dropped (0027). Later: vocals, learning, AI music.

## Stack

- **Webview UI** (TypeScript, Tauri 2): only draws and forwards laptop keys.
- **Rust core** (Tauri backend): commands, input, song files, scoring, file I/O.
- **C++ engine**: all sound, sequencer, instruments, effects, mixer and WAV.
- macOS and Windows from day one. GPL-3.0, public (anubhav-qt/jello): only
  GPL-3.0-compatible dependencies (ADR 0007).
- Run: `nvm use`, `npm install`, `npm run tauri dev` (restart it after
  `engine/` edits). Tests: `npm run build`, `npm run test:agent`,
  `cargo test` in `src-tauri/` (needs cmake for whisper.cpp).

## Status

- Built through roadmap step 5; [details](docs/design.md). Windows sound
  still needs listening. Produce is an undesigned dev panel.
- Fullscreen workspace (0039): plain page, 24 chords left, 24 melody notes
  right; panels stay fixed, contents scroll without bars.
- Keys: bare arrows shift octaves; left Cmd+arrows edit sound values (left
  Alt Windows); Shift vibrato, Option glide (right Alt Windows); F12 steps
  the wave in Sound. Left Cmd+1..9 picks or makes a track, +Return saves a
  section, +Delete removes the last; right Option jumps to the drums track.
- Agent (0041, 0043): MCP tool server in Rust (`tool_server.rs`,
  `tools.rs`, `listening.rs`, `undo.rs`); steps 1 to 9 done, step 10 asks
  first. Providers: Anthropic, OpenAI, Google. Input box: Type, Speak and
  Hum tabs ([design](docs/design.md#the-input-box)).
- Sampler (0044): `instruments.rs`, `drums.rs`, `starters.rs` (download
  only on click), `engine/src/sample_set.*`, `drum_kit.*`.
- Tracks and sections (0045): built. `tracks.rs`, `sections.rs`,
  `arrange.rs`, `automation.rs`; engine `tracks.*`, `sequencer.*`. Agent:
  `jello_tracks`, `jello_arrange`, `track` on the other tools.
- Mix (0046, step 6): engine slices 1 to 5 (`mixer.*`, `limiter.*`,
  `chain.*`, `effect.*`, `listener.*`, nine effects); slice 6's Rust built
  with backend integration implemented; all tests pass, sound and Windows
  unchecked.
  Files: `effects.rs` (values), `mix.rs`, `mix_file.rs`,
  `mix_tool.rs` (`jello_mix`), `mix_automation.rs`, `loudness.rs` (LUFS,
  true peak). Backend gaps and decisions: [tasks](docs/tasks.md).
  No Mix UI yet; mockups follow backend work and separate approval.
  [Mix design](docs/design.md#mix).
- Song key mappings (0047): approved, not built; replaces the fixed layout.
  [Design](docs/design.md#song-key-mappings).
- Layers (0042): own lengths on a common cycle (`layer_cycles.rs`), 64 bars
  each, 16,384 notes.

## Repo layout

- `ui/`: webview UI (Svelte 5, TypeScript, Vite). Start page: `ui/Stage.svelte`.
- `src-tauri/`: Rust core (Tauri app, `tauri.conf.json`, capabilities).
- `engine/`: C++ engine. C API in `include/jello_engine.h`, built by
  `src-tauri/build.rs`. How it works: [docs/design.md](docs/design.md#engine).
- [docs/design.md](docs/design.md): design and roadmap; [docs/adr/](docs/adr/README.md): ADRs.
- `.github/workflows/ci.yml`: CI, builds and tests on macOS and Windows.
- `tools/jello-drive.swift`: drives the real app on macOS; build steps inside.

## Working rules

- Read [engineering rules](docs/engineering.md) before work; they are mandatory.
- Track active work and pending decisions in [tasks](docs/tasks.md).
- Discuss product and design decisions one topic at a time. Approved designs
  authorize their full implementation; do not reopen settled decisions.
- Keep code simple; use plain if/else and avoid multi-step one-liners.
- Check UI changes in the real app and exercise controls and input edge cases.
- Single-line conventional commits, no attribution and no em dash anywhere.
