---
# SPDX-License-Identifier: GPL-3.0-or-later
name: pre-push
description: "The way from finished work to an open pull request: gates, signed commits, review, push, proof that the push landed, and this commit's CI watched to green. Use whenever work is ready to leave the machine."
---

The maintainer alone merges (constitution §5.1). This skill ends at an open pull request.

## Steps

1. Work on a `claude/…` branch from a fresh `main`, or from its parent if stacked. If the base moved,
   merge it in. Never rebase a pushed branch, `CONTRIBUTING.md`'s re-signing recipe apart.
2. Run `bash .github/scripts/check-all.sh` inside the devcontainer, and read its final count and each
   count a gate prints. If Docker is not available, run what the host can, and say exactly what was
   not run. Then, by what changed: `cargo xtask boot-test --arch aarch64 --expect "Kaya!"` for the
   kernel, `/check-style` for any pull request, `/check-primitive` for an addition, `/audit-kernel`
   for kernel code.
3. Commit with the message in a file (`git commit -F`), signed, in Conventional Commits, ending with
   the attribution trailers the session was given. No model identifiers beyond those trailers.
4. If this branch merges next, update `TODO.md` (`/handoff`).
5. Have it reviewed (`/pr-review`) while a fix costs least. After any change, go back to step 2: push
   only a commit that passed the gates and, for a critical change, the final round.
6. Push without filtering the output, then confirm that `git rev-parse HEAD` equals
   `git ls-remote origin <branch>`.
7. Open the pull request with `gh pr create --body-file`, adding `--base <parent>` if stacked,
   completing the template, and saying what was not run.
8. Watch this commit's pull-request run: repeat
   `gh run list --commit <full sha> --workflow "CI - Pull Request" --json databaseId,status` until it
   is not empty, then run `gh run watch <id> --exit-status` in the background. A bare list also
   shows CodeQL, and `gh pr checks` can answer before the new checks register.
9. Post the review summary as one comment, and tell the maintainer what needs a ruling.

## Sequencing

Pull requests that touch the same files are sequenced, not parallel, `TODO.md` included: see
`/handoff` step 5. Stack the later one and name the order. After a merge, merge `main` into the next
branch: the ruleset's up-to-date rule makes every merge stale the others.

## Lessons

- **2026-11-06 — a gate that can pass on an empty input is not a gate.** See `/check-spelling`'s
  lesson of that date. The gates that list files refuse an empty list and report a count.
- **2026-11-06 — verify that a flag really arrives.** `RUSTFLAGS` was set at workflow level for months
  and never reached cargo. Read the log line itself.
- **2026-11-06 — a green check is not proof.** A SHA-pinned action installed an unpinned tool at run
  time. Read an action's source and its real logs, not its README.
- **2026-11-06 — never filter push output.** A filtered push hides a rejection. Compare `HEAD` with
  `git ls-remote` instead.
- **2026-11-06 — `awk length` counts bytes.** Line lengths measured that way misled a width check.
  Measure in characters, with Python.
- **2026-11-06 — never chain a commit past a linter.** A chained command committed and pushed a known
  failure. Make the push conditional on the gate's clean result, and lint on the host when Docker is off.
- **2026-11-06 — a key can be configured and absent.** A repository's `user.signingkey` named a key
  this machine lacked, while the global one worked. Read
  `git config --show-origin --get user.signingkey` first, and never bypass signing to get past it.
