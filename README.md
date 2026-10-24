# Jello

A desktop synthesizer and music studio for computer keyboards on macOS and
Windows. Play 24 chords and 24 melody notes, layer loops, explore guided
progressions, shape a three-oscillator sound, and save songs automatically.

Compose follows the chords and notes you play, keeps a history of your tries,
and lets you select and reorder the final progression. Sound and Tutor use
the workspace above a keyboard and looper that stay visible in fullscreen.

- Arrows shift the whole keyboard: up/down one octave, right/left two.
- Pick a sound value, then left Command + arrows edits it (left Alt on Windows).
- Left Shift holds vibrato; left Option holds glide (right Alt on Windows).
- Right Shift records loops. Escape closes an open Sound/Songs panel first,
  otherwise it removes a loop layer. In Compose, Return adds the last chord.

See the [design and roadmap](docs/design.md) for what is done and what is next.

## Stack

- **UI:** TypeScript in a Tauri 2 webview (`ui/`)
- **Core:** Rust (`src-tauri/`)
- **Engine:** C++ (`engine/`)

Runs on macOS and Windows.

## Run it

You need [Rust](https://rustup.rs), Node (version in `.nvmrc`) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
nvm use
npm install
npm run tauri dev
```

## Tutor

Play's Tutor uses the pinned pi 1.4.0 agent with Jello observation and exercise
tools. Connect a model first: run `npx pi` in the project and use `/login`.
Tutor can then suggest practice keys and give feedback on measured pitches
and timing. It does not listen to audio. Packaged apps currently need Node
22.19+ and a separate pi installation; see [setup details](docs/design.md#current-workspace-and-tutor-2026-10-23).

Checks: `npm run build`, `npm run test:tutor`, and `cargo test` in `src-tauri`.

## Docs

- [Design and roadmap](docs/design.md)
- [Architecture decisions](docs/adr/README.md)

## License

Jello is free software under the [GNU General Public License v3.0](LICENSE).
You can use, change and share it, and anything you distribute that is built
on Jello must also be released under the GPL-3.0.
