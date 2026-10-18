# Jello

A desktop synthesizer and music studio. Play it with a computer keyboard or a
MIDI keyboard and make full songs with synths, samples, effects and mixing.
Later versions add vocals, a follow-along learning mode and AI music making.

Early days: the app shell runs, sound comes next. See the
[design and roadmap](docs/design.md).

## Stack

- **UI:** TypeScript in a Tauri 2 webview (`ui/`)
- **Core:** Rust (`src-tauri/`)
- **Engine:** C++ (`engine/`, coming soon)

Runs on macOS and Windows.

## Run it

You need [Rust](https://rustup.rs), Node (version in `.nvmrc`) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
nvm use
npm install
npm run tauri dev
```

## Docs

- [Design and roadmap](docs/design.md)
- [Architecture decisions](docs/adr/README.md)
