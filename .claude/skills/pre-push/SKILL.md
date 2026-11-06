---
name: pre-push
description: "The way from finished work to an open pull request: gates, signed commits, push, proof that the push landed, and this commit's CI watched to green. Use whenever work is ready to leave the machine."
---

The maintainer alone merges (constitution §5.1). This skill ends at an open pull request.

## Steps

1. Work on a `claude/…` branch from a fresh `main`. If `main` moved, merge it in; never rebase a
   branch that is pushed.
2. Run `bash .github/scripts/check-all.sh` inside the devcontainer. Read each gate's count, not only
   its exit status. If Docker is not available, run what the host can, and say in the pull request
   exactly what was not run.
3. Commit with the message in a file (`git commit -F`), signed, in Conventional Commits, ending with
   the attribution trailers the session was given. No model identifiers.
4. Have it reviewed (`/review`) before pushing, while a fix costs least.
5. Push without filtering the output, then confirm that `git rev-parse HEAD` equals
   `git ls-remote origin <branch>`.
6. Open the pull request with `gh pr create --body-file`, completing the template, and saying what
   was not run.
7. Watch this commit's run, by its hash: `gh run list --commit <sha>`, then
   `gh run watch <id> --exit-status`. `gh pr checks` can answer before the new checks register.
8. Post the review summary as one comment, and tell the maintainer what needs a ruling.

## Sequencing

Pull requests that touch the same files are sequenced, not parallel. Stack the later one and name
the order. After a merge, merge `main` into the next branch: the ruleset's up-to-date rule makes
every merge stale the others.

## Lessons

- **2026-11-06 — a gate that can pass on an empty input is not a gate.** The spelling gate checked no
  files in CI for months. Every gate refuses an empty input and reports its count; read the count.
- **2026-11-06 — verify that a flag really arrives.** `RUSTFLAGS` was set at workflow level for months
  and never reached cargo. Read the log line itself.
- **2026-11-06 — a green check is not proof.** A SHA-pinned action installed an unpinned tool at run
  time. Read an action's source and its real logs, not its README.
- **2026-11-06 — never filter push output.** Compare `HEAD` with `git ls-remote` instead.
- **2026-11-06 — `awk length` counts bytes.** Measure line lengths in characters, with Python.
- **2026-11-06 — never chain a commit past a linter.** A chained command committed and pushed a known
  failure. Make the push conditional on the gate's clean result, and lint on the host when Docker is off.
- **2026-11-06 — a key can be configured and absent.** A repository's `user.signingkey` named a key
  this machine lacked, while the global one worked. Read `git config --show-origin` before anything
  else, and never bypass signing to get past it.
