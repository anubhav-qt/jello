# 0007: License Jello under GPL-3.0

- Status: Accepted
- Date: 2026-10-18

## Context

Jello is open source and public on GitHub. Without a license nobody may
legally reuse the code. The main options were permissive (MIT, Apache-2.0),
middle ground (MPL-2.0) and copyleft (GPL-3.0).

## Decision

Jello is licensed under GPL-3.0-only (`LICENSE` at the repo root).

- Anyone may use, change and share Jello, but whatever they distribute that is
  built on it must also be GPL-3.0. Jello and its forks stay open.
- Much of the best open-source audio code (Surge XT, Vital, Dexed) is
  GPL-3.0, and a GPL project may learn from and reuse it.

## Consequences

- Every dependency must have a GPL-3.0-compatible license: MIT, BSD,
  Apache-2.0, MPL-2.0, LGPL, zlib or public domain. Proprietary code and
  GPL-2.0-only code cannot be included.
- Some companies avoid GPL code, so fewer of them will build on Jello.
- The copyright owner can still sell Jello or offer it under another license
  later. If outside contributors join, set up a contributor agreement first
  so that stays possible.
