# 0043: The agent mode

- Status: Accepted
- Date: 2026-10-24

The agent of 0041 becomes a full mode. While it works, every step shows live
as a drawing: what it looked at, what it changed, what it played. Replies are
mostly Jello's own screens (practice lanes, sound changes, options to
audition, measures) with a line or two of text. Every chat is saved and
cached through pi. The agent keeps its own memory across chats, and Jello
measures the player's skills from practice. Images, PDFs and videos can be
attached; prompts can be dictated; ideas can be hummed in a modal with a
box to describe the vibe.

The Tutor aims at mastery, not only first lessons: technique, rhythm,
harmony, ear training, improvisation and long two-handed pieces, with exact
theory and exact scoring done by Jello, never guessed by the model. The
Composer aims at release-grade songs. That depends on the engine: tracks,
drums, effects, mixing and mastering (roadmap steps 4 to 6), each with its
own ADR and C++ design. The Composer gains a tool with each.
To keep the cache, all three jobs' instructions sit together at the start
of the prompt and a message switches between them, extending 0041.
Details and build order are in [design.md](../design.md#the-agent-mode).
