# 0038: Song files

- Status: Accepted
- Date: 2026-10-23

Every song is a text file in Music/Jello/Songs, like "Night drive.song",
holding everything: its key, its chords with how long each was held, its
tempo, snap, the loop and its sound. The text is TOML with words for every
choice: the key is "A minor", a chord "Am" (roots C3 to B3), a loop note
"E4". Loop notes are in beats from the loop's start, with the loop's bars;
chords keep their milliseconds, since Compose's chords are played freely.
The sound is a preset's text (ADR 0037) under `[sound]`. Reading forgives
like a preset's. There is no save button: the song open saves itself a
second after its last change, and as Jello quits. A new song is
"Untitled 1", or the next free number, and Jello opens the song open last
as it starts. The top bar's title is the song's name; clicking it opens a
Songs panel in the tape's place, from any tab, to open, make or rename
songs. Removing happens in the folder. Each loop layer gets a play and
pause button under its knob, and the lane a button for the whole loop:
an opened loop comes in paused, playing starts it from beat 1, and right
Shift while paused plays it. Which layers play is saved too. The tempo
stays locked while there is a loop, paused or not. Details are in
[design.md](../design.md#song-files).
