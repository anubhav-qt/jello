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
- Run: `nvm use`, `npm install`, `npm run tauri dev`. Tests: `cargo test` in
  `src-tauri/`. `tauri dev` does not watch `engine/`: restart it after C++ edits.

## Status

- Step 1 is done: test tone, WAV and CI on both OSes. Windows sound unheard.
- Step 2 [slices](docs/design.md#step-2-slices) are done and tuned by ear:
  Stage (0010), Svelte 5 (0011), chords left and melody right (0012), note
  colors (0014). Keep this layout. Every chord has its own key (0020).
- Tabs (0016) Play, Compose, Produce (dev panel). Compose: chord and melody
  keys guided (0017, 0023), rules in `src-tauri/src/harmony.rs`, tape (0022).
- Looper (0021): all 4 slices done, snap and layers (0026) last. Look
  (0024) built, lane as wide as the keyboard with the snap switch (0025).
  Later: AI harness on a pi fork (0018); own instruments, no guitar (0019).

## Repo layout

- `ui/`: webview UI (Svelte 5, TypeScript, Vite). Start page: `ui/Stage.svelte`.
- `src-tauri/`: Rust core (Tauri app, `tauri.conf.json`, capabilities).
- `engine/`: C++ engine. C API in `include/jello_engine.h`, built by
  `src-tauri/build.rs`. How it works: [docs/design.md](docs/design.md#engine).
- [docs/design.md](docs/design.md): the design, roadmap and open questions.
- [docs/adr/](docs/adr/README.md): one file per decision (ADRs).
- `.github/workflows/ci.yml`: CI, builds and tests on macOS and Windows.

## Working rules

### Talking with the user

- One topic at a time. Plan as much as you like, but ask the user one
  question or decision at a time. Never stack several decisions in one message.
- The user wants to understand all code. Explain each new piece in plain
  words and get a yes before writing it, then explain what was written.
- Never write C++ engine code before the user has approved its design.
- Explain new music terms briefly; the user is learning music theory.
- Sound choices are the user's. Ask before rendering any sound demo, change
  one thing at a time, and let the user pick by ear.

### UI

- The user is particular about UI. Expect many rounds before acceptance.
- Never build a UI design the user has not approved. Show mockups first,
  iterate one screen or component at a time, then build.
- Until a screen's design is approved, it is a plain "dev panel" placeholder,
  clearly labeled as such.

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
- Every significant decision gets an ADR in `docs/adr/`: a short brief of a
  few lines. Small decisions are just todos. Never rewrite an accepted ADR;
  write a new one that supersedes it.
