# 0003: Every action is a named command

- Status: Accepted
- Date: 2026-10-18

## Context

The UI, an MCP server and an in-app AI harness must all be able to do
everything in Jello. Building three separate ways to do each thing would be
slow and they would drift apart.

## Decision

Every action ("add track", "set filter cutoff", "play note", ...) is a named
command, defined once in Rust. The UI, the MCP server and the in-app AI all
call the same commands.

## Consequences

- Every new feature shows up for the UI and the AI automatically.
- The AI never has to click around the UI.
- A little more structure up front: a feature is not done until it is a
  command.
