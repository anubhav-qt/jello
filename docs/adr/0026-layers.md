# 0026: Layers

- Status: Accepted
- Date: 2026-10-21

Once a loop plays, right Shift records a layer over it, round and round
until the next press, with the click on. Notes played in one time around
play from the next. A loop holds at most 8 layers, and a layer with nothing
played is dropped; a message in the top bar says so each time. Esc removes
one layer, not the whole loop: with two layers or more it waits half a
second, and a number from 1 to 8 picks the layer that goes; otherwise, or
on a second Esc, the newest goes. Removing the only layer clears the loop
as before. Each layer has a volume from 0 to 100, turned by a ring knob of
9 dots on the left of its box: drag up or scroll to turn it up, double
click for full. Notes dim with their layer's volume, and live notes always
play as loud as played. Supersedes ADR 0021's Esc and decides ADR 0025's
past eight layers. Details are in [design.md](../design.md#looper).
