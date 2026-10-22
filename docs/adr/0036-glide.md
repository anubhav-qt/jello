# 0036: Glide

- Status: Accepted
- Date: 2026-10-23

Left Option (left Alt on Windows) becomes the glide key: held, each melody
note slides to its pitch from the last one instead of jumping. A melody
note is one started on its own; notes that start together are a chord, and
a chord never glides. If the last melody key is still held, its voice bends
to the new note with no new attack, and letting go of the new key bends it
back to the one still held. If it was let go, the new note gets its own
attack and slides in. Each glide moves in a straight line through the cents
and takes the song's glide time, 0 to 10,000 ms (100 ms in a new song),
whatever the distance; the routing list can move it. Each player glides on
its own: live play, the song and each loop layer, and the looper records the
key like the vibrato key (0032). So Option/Alt stops being a shortcut key
in Jello: it no longer blocks keys or lets go of every note. Cmd still
does. This supersedes Option/Alt's part of the shortcut rule in ADR 0009
and ADR 0032. On Windows, Alt+F4 and Alt+Space are the system's, and
pressing Alt alone may move the focus to a menu, so Windows needs checking.
Details are in [design.md](../design.md#glide).
