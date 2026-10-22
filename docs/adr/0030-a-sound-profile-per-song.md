# 0030: A sound profile per song

- Status: Accepted
- Date: 2026-10-22

Sound belongs to the song, not the app. Every song has its own sound
profile: its wave, its low-pass filter and filter envelope, and its unison.
Two songs can sound different, or the same if the player makes them so; no
setting is tuned to one value for every song, so this replaces picking the
filter's and unison's values by ear (0028, 0029). The song in Compose holds
its profile. Every change to the sound, from the sound's dev panel or the
wave key, goes to the song first and from it to the engine, so the engine
plays only what the song says and keeps no sound of its own. As the app
starts, the song sends its whole profile to the engine, before the first
note. A new song starts with a triangle wave, the filter open, no filter
envelope and 1 copy: only a starting point. Play plays the open song's
profile too. Saving the profile in song files waits for step 5, and so do
copying a profile from another song and a profile per track. Details are in
[design.md](../design.md#engine).
