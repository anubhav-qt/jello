# 0046: The mix

- Status: Accepted
- Date: 2026-10-25

Each track has up to 8 effect slots, any effect in any order. A song has up
to 4 send buses, each with 8 slots of its own, and every track sends a share
of its sound to each. Everything meets in the master: 8 slots, then a
limiter that is always last and replaces the soft clip. Any effect that
reacts to loudness can listen to another track or bus (sidechain).

v1 builds a core set of effects first (EQ, compressor, gate, saturation,
chorus, delay, reverb, filter, volume shaper); the rest of the catalog comes
in batches. LFOs, connections and section automation move every effect
setting. The Mix panel opens in Compose like the Sound panel, and the agent
gets one `jello_mix` tool, with loudness and true peak in listening. Details
in [design.md](../design.md#mix).
