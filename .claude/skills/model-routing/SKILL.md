---
name: model-routing
description: "Choose the model, effort and number of agents for a piece of work, and brief them. Use before spawning any agent."
---

Tokens are the maintainer's money. Spend as few as the work allows, and survey cheaply in the main
session first (`gh`, `git`, `grep`).

## Which model

| Work | Choice |
|------|--------|
| The default | Opus at medium effort |
| Hard design, `unsafe` or boot code, adversarial review | Opus at high effort |
| Mechanical jobs with a known answer | A cheaper model |
| Frontier-hard work only | Fable, with the reason stated |

At most **three agents at a time**. Do it inline when it is a handful of tool calls.

## The brief

An agent knows only what the brief says. State the goal, the files, and what is out of scope. Every
brief ends with the house rule: clean up what you made, and only that; never clear a shared
scratchpad. Reviews have their own brief: see `/review`.

Narrow briefs are cheaper and better. Exact files and exact offsets beat "look around". Give an
agent the existing issue text and ask what is missing, rather than have it rediscover the issue.

## Lessons

- **2026-11-06 — a cheaper model can cost more.** In a mechanical documentation trial on a sibling
  project, the mid-tier model used fewer tokens and found more than the cheapest one, so try the
  middle first.
- **2026-11-06 — a diff-only review brief is cheap.** Such reviews cost about half of a full
  audit on the same project and still caught real defects.
- **2026-11-06 — a brief must forbid clearing the scratchpad.** An agent on a sibling project deleted
  every file in a scratchpad shared with its session.
