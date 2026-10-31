# 0049: The keyboard guitar

- Status: Accepted
- Date: 2026-10-31

Jello plays guitars on laptop keys with no recordings. A seventh oscillator
kind, string, makes a plucked string as it plays, a delay line that loses a
little each trip, so vibrato, glide, filter, unison and the routing list all
work on it. A guitar amp at the end of a track, off by default, makes an
electric its own instrument: acoustic and electric differ by pickup and amp,
not by added effects. All are saved as presets that are only places to start.

A track whose sound plays a string turns the keys into a guitar. The right
hand's U I O P [ ] are the six strings, ] the thinnest. The left hand's 20 keys
from 1QAZ to 5TGB each hold a shape, which says the note every string plays,
and with none held the strings play open. Bare arrows shift octaves as on any
other sound.

A plucked string rings after its key comes up, until it is plucked on another
note or rings away. A left-hand key moves a ringing string to its new note with
nothing plucked, carrying on from what is left of it, and leaves a string on the
same note alone. Glide stays on one string. Space, the pedal elsewhere, is a
palm: it stops every string, and plucks are softer while it is held.

Shapes follow [ADR 0047](0047-song-key-mappings.md): keys play nothing until a
scale is picked, each of the 11 scales has a default, and a song or a section
can change any shape. Details are in [design.md](../design.md#guitar).
