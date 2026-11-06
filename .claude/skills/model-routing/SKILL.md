---
name: model-routing
description: "Choose the model, effort and number of agents for a piece of work, brief them, and log what they cost. Use before spawning any agent."
---

Spend tokens where they buy accuracy. Never save them by weakening a check the maintainer set:
tests, review tiers and CI are not where to economise. Waste is the target: duplicated reading,
rebuilt context, and rounds a checklist would have avoided.

## Which model

Match the model to the difficulty of the task, not to its importance alone.

| Work | Choice |
|------|--------|
| Nearly everything: implementers, fixes, investigations, every review lens | Opus at medium effort |
| The exception, with the reason stated: `unsafe` or boot code, or a review round that a medium one plainly missed | Opus at high effort |
| Mechanical work with a checkable answer: scanning a log, listing links, summing a table | A cheaper model, checked |
| Frontier-hard work only | Fable, with the reason stated |

Opus at medium as the default is a trial, on the maintainer's word of 2026-11-06: log what each run
found, and go up a level at once if a medium review misses a high that a later round finds. Avoid
maximum effort: it costs about twice what high does, and high still finds the serious defects.

## How many

At most **three agents at a time**, counting every kind. Queue the rest and start the next as one
finishes; start one pull request's reviewers together, as a set. Do it inline when it is a handful of
tool calls, and survey cheaply first (`gh`, `git`, `grep`). Leave a quarter of the machine free for
the maintainer: `CARGO_BUILD_JOBS`, and stagger fresh builds.

## The brief

An agent knows only what the brief says. State the goal, the files and what is out of scope. Narrow
briefs are cheaper and better: exact files and offsets beat "look around", and giving an agent the
existing issue text beats having it rediscover the issue. One check belongs to one agent.

Reviews take most of the tokens, so the cheapest review is the round not needed. The author runs
`/rfc-draft`'s self-check, or the equivalent for code, before any reviewer starts.

Every brief ends with the house rule: clean up what you made, and only that; never clear a shared
scratchpad; report anything you could not remove. Reviews have their own brief: see `/review`.

## Afterwards

Log each agent's tokens as its notification reports them, in `agent-usage-log.csv` in the superfolder,
outside the repository: date, pull request, work, runs, tokens, model, effort, and what it found.
Compare periodically, and take a refinement to the maintainer with a recommendation.

Look for repeat costs unasked: the same fix by hand twice, a trap an agent fell into, a procedure that
should be a script or a skill. Say so at a natural pause, as a concrete proposal.

## Lessons

- **2026-11-06 — a cheaper model can cost more.** In a mechanical documentation trial on a sibling
  project, the mid-tier model used fewer tokens and found more than the cheapest one, so try the
  middle first.
- **2026-11-06 — a diff-only review brief is cheap.** Such reviews cost about half of a full
  audit on the same project and still caught real defects.
- **2026-11-06 — a brief must forbid clearing the scratchpad.** An agent on a sibling project deleted
  every file in a scratchpad shared with its session.
- **2026-11-06 — measure across many runs.** In about 640 logged reviews on a sibling project, Opus
  reviewers found roughly ten times the highs per run that Sonnet did, at about twice the tokens.
  Sonnet mostly had the lower-risk lenses, so that compares roles as much as models. It is why the
  soundness lens stays on Opus.
- **2026-11-06 — a third fresh pass still paid.** The boot-grant review went from three highs to
  none in two rounds; the stop rule then saved a third.
