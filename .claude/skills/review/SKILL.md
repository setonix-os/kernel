---
name: review
description: "Run independent reviews of a pull request: how many reviewers by risk, which lenses, what they are told, how findings are graded, and when to stop. Use before pushing any pull request."
---

Reviewers find; the author decides. Authority stays with the maintainer (constitution §5.3).

## How many

| Tier | Touches | Reviewers |
|------|---------|-----------|
| Critical | `unsafe`, boot, MMU, capabilities, security, CI publishing, an RFC heading for acceptance | Three, with a new round after substantial fixes |
| Ordinary | Everything else | Two, one round |

## Lenses

Each reviewer leads with a lens of its own, because reviewers alike mostly find the same things.
The lens comes from the tier, not from the change.

1. **Soundness:** try to falsify the change. Find the concrete case that breaks it.
2. **Rules and documents:** the constitution, `CLAUDE.md`, `STYLE.md` and the RFCs against the diff.
3. **Adversary (critical only):** the threat model's adversaries, against the obligations touched.

## What a reviewer is told

The branch, the diff by exact hashes copied from `git rev-parse`, and the lens. Never your
conclusions, your notes, or another reviewer's report: an agent handed a hypothesis tends to return
it confirmed. Every reviewer in a round reads the same commit.

Each brief also says:

- work read-only, in a separate worktree made with `git worktree add --detach`;
- never run `git checkout`, `stash` or `reset`, and leave nothing behind;
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

Check every finding against the tree yourself before acting on it. Apply what is real, on the
theme, nits included. A bug off the theme gets its own branch. Post one summary comment, with what
was not applied and why. Remove the worktree.

## When to stop

If a round's high findings do not fall, or a third round looks necessary, stop patching. Rethink the
design and take it to the maintainer, or build a model of it, or split it.

## Lessons

- **2026-11-06 — RFCs converge at the core, not at the seams.** If a third review round is needed,
  model it or split it rather than patch it.
- **2026-11-06 — keep reviewers out of the author's worktree.** A reviewer left one detached and a
  merge almost landed on no branch.
- **2026-11-06 — two lenses found what one author missed.** Both reviewers of the drift fix caught
  an overclaimed "nothing is lost" that a commit in the project's own log contradicted.
