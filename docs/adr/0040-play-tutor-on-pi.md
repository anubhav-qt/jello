# 0040: Play tutor on pi

- Status: Superseded by 0041
- Date: 2026-10-23

Bring an AI tutor into Play now. Its first harness uses pi 1.4.0, pinned as an
MIT npm dependency, with Jello-specific observation and practice tools. This
supersedes 0018's requirement to start with a fork/submodule; broader song
editing, rendering and listening remain future harness work.

A request snapshots the map, guidance and recent real performance events.
The agent can read that context and propose a validated exercise. Jello
highlights its steps and measures pitch matches; the agent supplies teaching
and feedback. No model is simulated, no audio-listening claim is made, and
no general shell or file tools are enabled. Audio remains responsive during
requests. Cancellation and a two-minute timeout stop the model process.

pi owns provider credentials and login. Development uses the local npm runtime;
packaged apps currently need Node 22.19+ and pi installed separately. A bundled
runtime and in-app provider login are follow-up distribution work. A model
login is required to test actual lessons. Lesson chat and tries are session
state; song files only persist the chosen progression and musical state.
