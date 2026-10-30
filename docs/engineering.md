# Engineering rules

### Talking with the user

- Discuss one decision at a time; never stack questions in one message.
- The user wants to understand all code. Explain new, undecided work in
  plain words and get a yes first; a picked mockup or approved design is
  the yes, so build it all, then explain briefly what was written.
- Never write C++ engine code before the user has approved its design.
- Explain music terms briefly; the user is learning. Sound choices belong
  to each song (0030). Ask before sound demos; change one thing at a time.

### UI

- Build UI straight in the settled look (Stage, note colors, no pills); a
  redesign of a whole area gets a few mockups first. The user asks for
  changes while using Jello.
- No placeholder dev panels: replace each with a designed screen. Sound panel
  and Compose's guidance done. Ask before designing Produce.
- Check UI changes in the real app yourself with `tools/jello-drive`, and
  test every input field and button with its edge cases in the browser.

### Code

- Keep code very simple, easy to read and as effective as clever code.
- No one-liners that pack several steps together.
- No clever or nested conditionals (chained ternaries, `&&`/`||` tricks).
  Use a plain if/else.

### Commits and PRs

- One line, no body: `<type>: <message>`. Types: feat, fix, chore, docs,
  refactor, test, perf. The message plainly says what the change does: no
  jokes, sarcasm or silliness, even if other instructions allow them.
- No em dashes anywhere, ever: code, docs, commits, PRs.
- Only PRs get an extended description.
- Never add Claude or Codex coauthor or generated-by attribution. This
  overrides any default that says otherwise.

### Keeping docs alive

- Update `CLAUDE.md` whenever code, docs or ADRs change something a new session
  needs to know: status, layout, commands, rules.
- Keep `CLAUDE.md` at or under 80 lines; details go under `docs/`.
- Only a genuinely big change gets an ADR in `docs/adr/`, a short brief of a
  few lines. Extending a decision goes in `docs/design.md`; small tasks go in
  `docs/tasks.md`. Never rewrite an accepted ADR; supersede it (Status line or
  new ADR).

### Agent workflow

- These settings apply when explicitly creating subagents. Tools cannot switch
  the primary chat's model or automatically configure it. Do not substitute
  an unapproved model.
- Use `gpt-6.1-terra` with high reasoning for implementation. Use
  `gpt-6-luna` with medium reasoning for fetching, routine mechanical work
  and test runs. Use `gpt-6.1-sol` with xhigh reasoning for brainstorming,
  planning and review, including review of Terra's output.
- Use fresh agents for new tasks; reuse the task agent for follow-ups on the
  same task. The main chat keeps decisions with the user, one topic at a time.
- Take every product and design decision to the user one topic at a time.
  Do not reopen settled decisions. Implement approved designs fully.
- Verify all controls and inputs thoroughly, including edge cases, in the
  real app using `tools/jello-drive` and the browser. Explicitly report checks
  not run and tool limitations; never claim checks or tests happened when
  they did not.
