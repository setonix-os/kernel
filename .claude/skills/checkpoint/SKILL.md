---
name: checkpoint
description: "Record where the work stands so that a cold session can resume with nothing lost. Run before opening a pull request, whenever the maintainer says stop or pause, and at the end of every session."
---

Write a checkpoint. Each kind of record has one home, so that nothing is said twice and goes stale.

| Record | Home |
|--------|------|
| Where the work stands, and what is next | `TODO.md` § Resume here, overwritten and never stacked |
| Work still to do | `TODO.md`; a finished item is deleted, not struck through |
| What changed | `CHANGELOG.md`, or `docs/CHANGELOG.md` for documents |
| Rulings and designs | `docs/rfcs/`, or a dated amendment |
| Lessons | The Lessons list of the skill they concern |
| Facts the repository cannot hold | Session memory; never progress |

## Steps

1. Run `git fetch origin`, then read the state rather than remembering it: `git status -sb`,
   `git log --oneline -5 origin/main`, `git branch -a`, `gh pr list`, `git stash list`.
2. Overwrite `TODO.md` § Resume here, dated: what stands, what is half-done and exactly where, what
   is next, what waits on the maintainer. Name files, branches and pull requests. Copy every commit
   hash from the output of step 1, never from memory. With two or more pull requests open, give
   their merge order.
3. Delete each `TODO.md` item whose work is done, once its outcome is in a changelog.
4. Run `/steward` for anything this session taught.
5. Clean up: see `CLAUDE.md` § Never do these, and the list below.
6. Report in three lines: state, next, and what waits on the maintainer.

## Cleaning up

Remove what this session made, and only that:

- worktrees (`git worktree remove`, then `git worktree prune`);
- branches merged into `main`, local and remote, after confirming with
  `git branch -r --merged origin/main`;
- the session's scratchpad, with each path named rather than a wildcard;
- containers and images built only for this task, and processes left running.

Uncommitted work and stashes are never litter. Ask before removing anything you did not make.

## Lessons

- **2026-11-06 — a handoff that is a separate file rots.** The local clone stood three months behind
  `main`, and session memory still named a next step long since done. A record overwritten in the
  repository, from commands rather than recollection, cannot drift that far.
