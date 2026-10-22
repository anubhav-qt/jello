# 0037: Presets

- Status: Accepted
- Date: 2026-10-23

A preset is a song's whole sound profile (ADR 0030), tuning included,
saved by name as a text file in Music/Jello/Sounds, like
"Glass lead.sound". The text is TOML with words for every choice ("saw",
"low-pass", "lfo 1") and only the connections in use. Reading forgives:
a value left out, of the wrong sort or naming an unknown word takes a new
song's; unknown names are skipped; the engine brings values within range.
Connections are only those listed. A file that is not TOML can't be read
and never half-loads. A sixth sound panel page, Presets, lists them as
tiles: clicking one copies it into the song, and the song then changes its
own copy. A tile is lit while the song sounds exactly like it. The page
saves the song's sound by name, replacing a preset of the same name in any
case. Names are letters, numbers, spaces, "-" and "_", up to 40, never a
name Windows keeps. One preset can be marked as the starting sound, kept in
`settings.toml` in Jello's settings folder: every new song starts with it,
and with none, or one gone or unreadable, a new song's triangle. Renaming
and removing happen in the folder for now. Song files will write sound the
same way. Details are in [design.md](../design.md#presets).
