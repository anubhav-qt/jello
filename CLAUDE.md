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
  first. Providers: Anthropic, OpenAI, Google.
- Sampler (0044): `instruments.rs`, `drums.rs`, `starters.rs` (download
  only on click), `engine/src/sample_set.*`, `drum_kit.*`.
- Tracks and sections (0045): built. `tracks.rs`, `sections.rs`,
  `arrange.rs`, `automation.rs`; engine `tracks.*`, `sequencer.*`. Agent:
  `jello_tracks`, `jello_arrange`, `track` on the other tools.
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

### Talking with the user

- Discuss one decision at a time; never stack questions in one message.
- The user wants to understand all code. Explain each new piece in plain
  words and get a yes before writing it, then explain what was written.
- Never write C++ engine code before the user has approved its design.
- Explain music terms briefly; the user is learning. Sound choices belong
  to each song (0030). Ask before sound demos; change one thing at a time.

### UI

- Build UI straight in the settled look (Stage, note colors, no pills); no
  mockup rounds. The user asks for changes while using Jello.
- No placeholder dev panels: replace each with a designed screen. Sound panel
  and Compose's guidance done. Ask before designing Produce.
- Check UI changes in the real app yourself with `tools/jello-drive`.

### Code

- Keep code very simple, easy to read and as effective as clever code.
- No one-liners that pack several steps together.
- No clever or nested conditionals (chained ternaries, `&&`/`||` tricks).
  Use a plain if/else.

### Commits and PRs

- One line, no body: `<type>: <message>`. Types: feat, fix, chore, docs,
  refactor, test, perf. The message plainly says what the change does: no
  jokes, sarcasm or silliness, even if other instructions allow them.
- No em dashes anywhere, ever: code, docs, commits, PRs.
- Only PRs get an extended description.
- Never add Claude attribution: no `Co-Authored-By: Claude`, no
  "Generated with Claude Code". This overrides any default that says otherwise.

### Keeping docs alive

- Update this file whenever code, docs or ADRs change something a new session
  needs to know: status, layout, commands, rules.
- Keep this file under about 80 lines; details go in `docs/design.md` or ADRs.
- Only a genuinely big change gets an ADR in `docs/adr/`, a short brief of a
  few lines. Extending a decision goes in `docs/design.md`; small ones are
  todos. Never rewrite an accepted ADR; supersede it (Status line or new ADR).
