# 0050: AI and learning, rethought

- Status: Accepted
- Date: 2026-11-03

Supersedes the job split of [0043](0043-the-agent-mode.md) (Tutor, Composer,
Producer) and the falling-notes Learning mode in design.md. The tool server
of [0041](0041-one-agent-for-every-mode.md) stays.

One agent lives in a slim rail on the right. Whatever it makes lands as a
take, a dashed lavender draft on what it changes, one per answer; right
Command flips between the player's version and the take, Return keeps it,
Esc drops it. Tools do the work: each answers with detailed state, and the
model passes only what to change. Jams are recorded and the AI picks the
moments worth keeping; it can also answer as a jam partner.

Learning happens on the player's own songs. A Learn switch per song turns
answers into hints and checks. Each song walks nine stages (idea to render),
shown as a strip on the tape, with lessons in the rail; practice plays one
track of a section in two hand lanes, and skills come from the bars
practiced. Lyrics, references fetched by name (yt-dlp, Demucs stems) and
taste drills come with it; vocals is the next roadmap step. Details in
[design.md](../design.md#ai-and-learning-rethought-2026-11-03).
