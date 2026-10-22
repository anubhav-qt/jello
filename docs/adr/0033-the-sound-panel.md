# 0033: The sound panel

- Status: Accepted
- Date: 2026-10-22

The sound's dev panel is replaced by a designed sound panel, approved through
mockups one page at a time. A Sound button at the right end of the tabs, in
Compose, opens it in the tape's place, as wide as the keyboard, which stays
in view and keeps playing. It has four pages: Notes (wave, loudness
envelope, unison, tuning, the vibrato key's times), Filter (cutoff and
resonance, its envelope, key tracking), LFOs (all 8) and Connections (the
routing list). Every value is the song's (0030) and is a name, a white
number and a row of dots lit up to it: drag along the dots or click one.
Drawings show the real values, and their squares drag them too. Clicking a
value picks it, framed in lavender, and the arrow keys step it, one step
sideways and ten up or down, while every other key keeps playing. Values,
drawings and readouts are white; lavender marks only what is picked or open.
Settings that would do nothing are dimmed. Details are in
[design.md](../design.md#sound-panel).
