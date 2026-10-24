# Jello

Jello is a desktop synthesizer and music studio, played on computer keyboards
only, no MIDI, so step 3 is dropped (0027). Later: vocals, learning, AI music.

## Stack

- **Webview UI** (TypeScript, Tauri 2): only draws and forwards laptop keys.
- **Rust core** (Tauri backend): commands, input, song files, scoring, file I/O.
- **C++ engine**: all sound, sequencer, instruments, effects, mixer and WAV.
- Platforms: macOS (laptop) and Windows (desktop), both from day one.
- Open source under GPL-3.0, public on GitHub (anubhav-qt/jello). Only add
  dependencies with GPL-3.0-compatible licenses (see ADR 0007).
- Run: `nvm use`, `npm install`, `npm run tauri dev`. Tests: `npm run build`,
  `npm run test:tutor`, `cargo test` in
  `src-tauri/`. `tauri dev` does not watch `engine/`: restart it after C++ edits.

## Status

- Synth/looper/song slices through 0038 are done; [details](docs/design.md).
  Windows sound still needs listening. Produce is an undesigned dev panel.
- Fullscreen workspace (0039): plain page, no box, dots or beams around
  the keys; looper lane (keys' width) only once looping. Keep 24 chords left,
  24 melody notes right. Panels stay fixed; contents scroll without bars.
- Bare arrows shift octaves (up/down 1, right/left 2). Left Cmd+arrows edits
  sound values (left Alt Windows). Shift vibrato; Option glide (right Alt
  Windows); F12 steps the wave shown in Sound. Recordings keep original
  pitch after octave changes. Agent button: Tutor, Composer or Producer.
- One agent for every mode (0041), before the sampler. Built in Rust: MCP
  tool server (`tool_server.rs`, `tools.rs`): look, sound, play, looper,
  song, presets, listen (`listening.rs`, `measures.rs`); Undo (`undo.rs`).
  Agent mode (0043): live steps, cards, saved chats, memory, master-level
  Tutor, attachments, voice; build order in design.md. `npx pi`, `/login`.

- Pointer transport and independent layer lengths (0042): Record/Finish,
  exact tempo, timed takes, expression holds and cycle WAV export. Layer
  lengths live in song files; `layer_cycles.rs` expands playback on a common
  cycle without changing the audio thread. Limits: 64 bars per layer,
  4096 playback notes/common bars. Shortening preserves original notes.

## Repo layout

- `ui/`: webview UI (Svelte 5, TypeScript, Vite). Start page: `ui/Stage.svelte`.
- `src-tauri/`: Rust core (Tauri app, `tauri.conf.json`, capabilities).
- `engine/`: C++ engine. C API in `include/jello_engine.h`, built by
  `src-tauri/build.rs`. How it works: [docs/design.md](docs/design.md#engine).
- [docs/design.md](docs/design.md): the design, roadmap and open questions.
- [docs/adr/](docs/adr/README.md): one file per decision (ADRs).
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
