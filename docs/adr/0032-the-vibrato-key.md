# 0032: The vibrato key

- Status: Accepted; Option/Alt as a shortcut key superseded by 0036
- Date: 2026-10-22

Left Shift becomes the vibrato key: held, it brings in whatever the song's
routing list connects to it (0031), vibrato in a new song. It rises and
falls over times the song sets. So F#m moves one key down, to left Control,
and Control stops being a shortcut key in Jello: it no longer blocks keys or
lets go of every note. Cmd and Option/Alt still do. This supersedes left
Shift's F#m in ADR 0020 and Ctrl's part of the shortcut rule in ADR 0009.
Each player has its own key: live play, each loop layer and the song. The
looper records the key's presses with a layer's notes and plays them back.
The song's key stays up until tracks and automation (step 5), when the AI
harness can write it too. Windows uses Ctrl+S and Ctrl+Z for saving and
undo, and Ctrl+Space or Ctrl+Shift may switch the keyboard language, so both
machines need checking. The whole key map, octave shift on a held Option or
Cmd included, will be rethought later. Details are in
[design.md](../design.md#computer-keyboard).
