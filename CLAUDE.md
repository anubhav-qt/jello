# Jello

Jello is a desktop synthesizer and music studio. You play it with a computer
keyboard or a MIDI keyboard and make full songs with synths, samples, effects
and mixing. Later versions add vocals, a follow-along learning mode and AI
music making.

- Full design and roadmap: [docs/design.md](docs/design.md)
- Decisions (ADRs): [docs/adr/README.md](docs/adr/README.md)

## Stack

- **Webview UI** (TypeScript, Tauri 2): only draws and forwards laptop keys.
- **Rust core** (Tauri backend): commands, input, song files, scoring, file I/O.
- **C++ engine**: makes all sound. Sequencer, instruments, effects, mixer,
  audio out and offline render to WAV.
- Platforms: macOS (laptop) and Windows (desktop), both from day one.
- Open source under GPL-3.0, public on GitHub (anubhav-qt/jello). Only add
  dependencies with GPL-3.0-compatible licenses (see ADR 0007).
- Run: `nvm use`, `npm install`, `npm run tauri dev`.

## Status

- Now: v1 step 1, the skeleton. The Tauri shell opens an empty window.
  Next: agree on the C++ engine design, then a test tone to speakers and WAV.
- Roadmap: [docs/design.md](docs/design.md#roadmap).

## Repo layout

- `ui/`: webview UI (TypeScript, Vite). Currently a dev panel placeholder.
- `src-tauri/`: Rust core (Tauri app, `tauri.conf.json`, capabilities).
- `engine/`: C++ engine (not created yet).
- `docs/design.md`: the design, roadmap and open questions.
- `docs/adr/`: one file per decision.

## Working rules

### Talking with the user

- One topic at a time. Plan as much as you like, but ask the user one
  question or decision at a time. Never stack several decisions in one message.
- The user wants to understand all code. Before writing a new piece, explain
  the plan in plain words and get a yes. After writing it, explain what was
  written.
- Never write C++ engine code before the user has approved its design.
- The user is still learning music theory. Explain music terms briefly when
  they first come up.

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
  refactor, test, perf. The message can be playful or silly sometimes.
- No em dashes anywhere, ever: code, docs, commits, PRs.
- Only PRs get an extended description.
- Never add Claude attribution: no `Co-Authored-By: Claude`, no
  "Generated with Claude Code". This overrides any default that says otherwise.

### Keeping docs alive

- Update this file whenever code, docs or ADRs change something a new session
  needs to know: status, layout, commands, rules.
- Keep this file brief (under about 80 lines). Details go in `docs/design.md`
  or an ADR.
- Every significant decision gets an ADR in `docs/adr/`. Never rewrite an
  accepted ADR; write a new one that supersedes it.
