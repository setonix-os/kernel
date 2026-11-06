---
# SPDX-License-Identifier: GPL-3.0-or-later
name: handoff
description: "Record where the work stands so that a cold session can resume with nothing lost. Run before opening the pull request that merges next, whenever the maintainer says stop or pause, and at the end of every session."
---

Write the handoff. Each kind of record has one home, so that nothing is said twice and goes stale.

| Record | Home |
|--------|------|
| Where the work stands, and what is next | `TODO.md` § Resume here, overwritten and never stacked |
| Work with no other home | `TODO.md`; a finished item is deleted, not struck through |
| What changed | `CHANGELOG.md`, or `docs/CHANGELOG.md` for documents |
| Rulings and designs | `docs/rfcs/`, or a dated amendment |
| Lessons | The Lessons list of the skill they concern |
| Agents' token use | The maintainer's local log (`/model-routing`) |
| Facts the repository cannot hold | Session memory; never progress |

## Steps

1. Read the state rather than remembering it: `git fetch --prune origin`, `git status -sb`,
   `git log --oneline -5 origin/main`, `git branch -a`, `gh pr list`, `git stash list`.
2. Overwrite `TODO.md` § Resume here, dated: what stands, what is half-done and exactly where, what
   is next, what waits on the maintainer. Name files, branches and pull requests, and say which do
   not exist yet. Copy every commit hash from step 1's output, never from memory. With two or more
   pull requests open, give their merge order.
3. Delete each `TODO.md` item whose work is done, once its outcome is in a changelog.
4. Run `/steward` for anything this session taught.
5. Commit it. Only the branch that merges next carries the block, so that parallel pull requests do
   not fight over it; it goes in that branch's last commit before review. With no such branch, open
   a small pull request for it. A record left uncommitted reaches nobody.
6. At the end of a session, not at a pause, clean up as below.
7. Report in three lines: state, next, and what waits on the maintainer, plus every new `unsafe` block
   (constitution §11.3).

## Cleaning up

Remove what this session made, and only that (`CLAUDE.md` § Never do these holds the rule):

- worktrees, after `git log --not --remotes` shows they hold no unpushed commit, with
  `git worktree remove`, then `git worktree prune`;
- branches this session made, once merged: `git branch --merged origin/main` and the `-r` form,
  excluding `main` and `HEAD`, and never a branch that is an open pull request's base;
- files this session made inside the scratchpad, each by name; never the scratchpad itself.

Uncommitted work, stashes and unpushed commits are never litter.

## Lessons

- **2026-11-06 — a handoff that is a separate file rots.** The local clone stood three months behind
  `main`, and session memory still named a next step long since done. A record overwritten in the
  repository, from commands rather than recollection, cannot drift that far.
- **2026-11-06 — a skill's name can be taken.** `/checkpoint` and `/review`, this skill's and
  `/pr-review`'s first names, are Claude Code aliases, so typing one would not have run the skill.
  Check a new name against the built-in commands first.
