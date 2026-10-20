# Jello

Jello is a desktop synthesizer and music studio, played with a computer or
MIDI keyboard. Later versions add vocals, a learning mode and AI music making.

## Stack

- **Webview UI** (TypeScript, Tauri 2): only draws and forwards laptop keys.
- **Rust core** (Tauri backend): commands, input, song files, scoring, file I/O.
- **C++ engine**: makes all sound. Sequencer, instruments, effects, mixer,
  audio out and offline render to WAV.
- Platforms: macOS (laptop) and Windows (desktop), both from day one.
- Open source under GPL-3.0, public on GitHub (anubhav-qt/jello). Only add
  dependencies with GPL-3.0-compatible licenses (see ADR 0007).
- Run: `nvm use`, `npm install`, `npm run tauri dev`. Tests: `cargo test` in
  `src-tauri/`. `tauri dev` does not watch `engine/`: restart it after C++ edits.

## Status

- Step 1 (skeleton) is done: test tone, WAV render, CI on macOS and Windows.
  Not yet heard on Windows.
- Now: step 2, built in [slices](docs/design.md#step-2-slices), all done: the
  laptop keys play on the Stage start page (ADR 0010), tuned by ear, in
  Svelte 5 (ADR 0011). Default layout: chords left, melody right (ADR 0012).
  Next: key mapping and the on-screen piano.

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
- The user is still learning music theory. Explain music terms briefly when
  they first come up.
- Sound choices are the user's. Ask before rendering any sound demo, change
  one thing at a time, and let the user pick by ear.

### UI

- The user is very particular about UI. UI is never a one-off task; expect
  many rounds before a design is accepted.
- Never build a UI design the user has not approved. Show mockups first,
  iterate one screen or component at a time, then build.
- Until a screen's design is approved, it is a plain "dev panel" placeholder,
  clearly labeled as such.

### Code

- Very simple and easy to read. The most beautiful code is the easiest to read
  and still does the job as well as clever code.
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
- Keep this file brief (under about 80 lines). Details go in `docs/design.md`
  or an ADR.
- Every significant decision gets an ADR in `docs/adr/`: a short brief of a
  few lines. Small decisions are just todos. Never rewrite an accepted ADR;
  write a new one that supersedes it.
