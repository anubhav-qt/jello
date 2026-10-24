# 0042: Independent layer lengths and pointer recording

- Status: Accepted
- Date: 2026-10-23

Extend the looper with pointer controls for recording, overdubbing, exact
tempo, timed capture, vibrato, glide and exporting one cycle as WAV. A layer
can repeat every 1 to 64 bars at the shared tempo. Its length is saved with
its original notes; older files inherit the common loop length.

Rust expands the patterns onto their least common multiple before sending
them to the existing audio scheduler. This keeps playback on exact samples
without new allocation on the audio thread. Reject arrangements exceeding
4096 playback notes or 4096 common bars before replacing the current loop.
Changing a layer length restarts playback. Shortening clips playback while
keeping the original notes so lengthening can restore them.
