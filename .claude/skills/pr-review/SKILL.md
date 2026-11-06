---
# SPDX-License-Identifier: GPL-3.0-or-later
name: pr-review
description: "Run independent reviews of a change before it is pushed: how many reviewers by risk, which lenses, what they are told, how findings are graded, and when to stop."
---

Reviewers find; the author decides what to apply; the maintainer decides what lands
(constitution §5.1).

## How many

| Tier | Touches | Reviewers |
|------|---------|-----------|
| Critical | `unsafe`, boot, MMU, capabilities, security, CI publishing, an RFC heading for acceptance | Three, with a narrow re-check after a substantial fix |
| Ordinary | Everything else | Two, one round |

## Lenses

Each reviewer leads with a lens of its own, because reviewers alike mostly find the same things.
The lens comes from the tier, not from the change.

1. **Soundness:** try to falsify the change. Find the concrete case that breaks it.
2. **Rules and documents:** the constitution, `CLAUDE.md`, `STYLE.md` and the RFCs against the diff.
3. **Adversary (critical only):** the threat model's adversaries, against the obligations touched.

## The worktree

The author makes one with `git worktree add --detach <path> <sha>`, gives reviewers the path, and
removes it after the last report. Reviewers never change it.

## What a reviewer is told

The worktree, the diff by exact hashes copied from `git rev-parse`, and the lens. Never your
conclusions, your notes, or another reviewer's report: an agent handed a hypothesis tends to return
it confirmed. Every reviewer in a round reads the same commit.

Each brief also says:

- read only: no edits, and never `git checkout`, `stash`, `reset` or `fetch`;
- leave nothing behind, never clear a shared scratchpad, and report anything you could not remove;
- report in plain text: findings by severity, each with file and line and a concrete case; then what
  was checked and found sound; then what was not reached.

## Grading

| Severity | Meaning |
|----------|---------|
| High | A security obligation broken, `main` broken, or a binding rule broken |
| Medium | A real bug, or code and documents that disagree |
| Low | A minor bug or gap |
| Nit | Cosmetic |

## When they report

Check every finding against the tree yourself before acting on it. Apply what is real, on the theme,
nits included. A bug off the theme gets its own branch. Keep a summary, with what was not applied and
why, for the one comment `/pre-push` posts.

## When to stop

If a round's high findings do not fall, stop patching. Rethink the design and take it to the
maintainer, or build a model of it, or split it.

## Lessons

- **2026-11-06 — RFCs converge at the core, not at the seams.** RFC-0003a took repeated rounds at its
  seams with RFC-0005 and RFC-0006. Model or split, rather than patch.
- **2026-11-06 — keep reviewers out of the author's worktree.** A reviewer left one detached and a
  merge almost landed on no branch.
- **2026-11-06 — stop means stop.** Two rounds in a row broke a proposed O-22 text, each listing what
  the broker holds. Dropping the list solved what a third patch would not have.
- **2026-11-06 — two lenses found what one author missed.** Both reviewers of a drift fix caught an
  overclaimed "nothing is lost", which a changelog entry contradicted (`/rfc-draft`'s lesson).
