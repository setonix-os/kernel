<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Hand-off: 2026-11-06 cloud session to local Claude Code

This file records where things stand at the end of the cloud session, so that the next session can
start from it. It is not project documentation: read it, act on it, then delete it. Before doing any
of that, read `CONSTITUTION.md` and `CLAUDE.md`, which are binding.

This branch also carries `handoff/process-gap.txt`, the full gap analysis against
`embedded-society/altium-designer-mcp`. The scratchpad copy would otherwise be lost with the
container.

## 1. State of the repository

- `main` is at `221e2fe`, which is PR #30. CI is green, and the first `main` run with the new
  pipeline published `ghcr.io/setonix-os/kernel-devcontainer:latest` and `:221e2fe…` with the same
  digest.
- **No PRs are open.**
- **Remote branches:**
    - `claude/rfc-0003b-reclaim` is RFC-0003b, a **Draft**, deliberately without a PR. It is held for
    validation against the executable model; see §4.
    - `claude/test-actions-policy` is a throwaway from the policy test (PR #31, closed). **The
    maintainer deletes it**, because the cloud proxy refuses branch deletion.
- **Merged today:**

  | PR | What |
  |---|---|
  | #27 | The five Claude commands became skills in `.claude/skills/<name>/SKILL.md`, each with dated **Lessons**; the lessons rule is in `CLAUDE.md` § Claude Code skills |
  | #28 | `.github/scripts/check-all.sh` is the single list of CI's checks, and CI calls it; `check-code-spans.sh` gates split code spans and typographic quotes; the spelling gate, which had checked 0 files in CI ("dubious ownership"), now really runs |
  | #29 | `RUSTFLAGS=-Dwarnings` really reaches cargo in the container; `--locked` everywhere; least-privilege permissions; PRs boot release too; PR CI runs for every base branch |
  | #26 | RFC-0003a core, **Proposed**: forwarders, the no-unwrap rule, badges (x9/r14), ordered rendezvous tables, and payer destroy of forwarders |
  | #30 | No third-party actions; `.github/scripts/devcontainer.sh` (`build`, `build --publish`, `run --`); main publishes from `--no-cache`; the pin check enforces owner and SHA pins |

## 2. Settings the maintainer applied, and their verified state

- **Merge methods:** merge commits only; auto-merge on; branches deleted after merge; wiki off.
- **`main` ruleset:**
    - signed commits;
    - a PR with 1 approval, code-owner review, last-push approval, stale reviews dismissed and threads
    resolved;
    - the strict required check "CI Success";
    - CodeQL and code-quality gates;
    - Copilot review on drafts and on push;
    - the org-admin bypass, in **pull-request mode only**.
- **`tags` ruleset:** signed; admin-only.
- **Actions policy:** only GitHub-created and `setonix-os` actions.
    - **Verified live** on PR #31: a third-party action gives `startup_failure`; the same workflow
    without it succeeds.
    - The in-repo pin check also rejects it.
- **Consequence:** Claude's PRs are opened under the maintainer's own account, so he cannot approve
  them, and every merge goes through his PR-only bypass. A bot identity for Claude would make his
  approval count; this is an open follow-up.

## 3. How we work (agreed with the maintainer)

- **Authority.** The maintainer alone merges. Never merge, approve, or amend the constitution. PRs
  only on `claude/…` branches.
- **Models.** Opus at medium effort is the default; Opus at high effort for hard design, `unsafe` or
  boot code, and adversarial review; cheaper models for mechanical jobs; Fable only for
  frontier-hard work, with the reason stated.
- **Concurrency.** At most **3 agents at a time**.
- **Reviews.** Every PR gets **two independent agent reviewers on different lenses**. Critical PRs
  get three: `unsafe`, boot, MMU, capability, security, CI publishing, or an RFC heading for
  acceptance. Reviewers return findings only, and the lead applies them; send a fix back to the agent
  that has the context. Post **one summary comment** per PR, with the footer. A narrow re-verification
  follows a substantial fix.
- **Before every push:**
    - run `bash .github/scripts/check-all.sh`;
    - never pipe push output;
    - confirm that `git rev-parse HEAD` equals `git ls-remote`.
- **Commits.** Conventional Commits, SSH-signed, ending with the two trailers the session was given.
  No model IDs in commits or PRs.
- **Language.** British English everywhere.
- **Lessons.** A lesson lands in its skill **in the same PR** as the fix that taught it, dated, with
  its cause.
- **Reviewer worktrees.** A reviewer never works in the author's worktree; it uses its own, checked
  out with `--detach`. Today a reviewer left a worktree detached, and a merge almost landed on no
  branch.

## 4. Next steps, in the maintainer's order

1. **The skills and process PR.** Add the planned skills `pre-push`, `model-routing`, `steward`,
   `rfc-draft` and `fact-check`, and record today's lessons (§6). Possibly also a short process
   document recording §3.
2. **An executable model of RFC-0003a and RFC-0003b.** The maintainer asked for this *after* the
   stable base, and **it starts with a short plan for his approval**. It goes in the host-tested
   `capability` crate, or in a sibling `no_std` model crate, and covers:
   - forwarders, badges and the rendezvous tables;
   - Pools, `destroy` and `pool_reclaim`.

   Use property-based tests of the RFCs' claims. RFC-0003b §12 lists what the model must show:
   conservation, restart and termination of the walk, no escape of balance, how a lent context comes
   to rest, and recovery of the balance. RFC-0003b is reviewed again only after the model exists.
3. **Accept RFC-0005, 0006 and 0007 in tranches.** This is the maintainer's call. After that comes the
   EL0 "Kaya!" IPC demo (RFC-0007 §18), implemented against the accepted subset only.
4. **The remaining gap-analysis PRs.** Items 1–5 and parts of 6 and 7 landed in #28–#30. Still to do:
   - 6: one PR checklist, now mostly done through `check-all.sh`; verify it;
   - 7: the single-source sweep (`SECURITY.md`, README Status, CONTRIBUTING's `tests/` line);
   - 8: a never-do section in `CLAUDE.md`;
   - 9: document the rulesets in CONTRIBUTING;
   - 10: `cargo doc` with `-D warnings`;
   - 11 and 12: lychee internal links, then a weekly external-link check;
   - 13: a Dependabot docker ecosystem;
   - 14: in-image coverage;
   - 15: `TODO.md`;
   - 16: the extension lists.

   See `handoff/process-gap.txt` §(a).

## 5. Open decisions (the maintainer's)

- **RFC-0003a §14:**
    - the forwarder depth limit of 4;
    - eviction notice: a broker protocol or a kernel notification;
    - clearing dead waits eagerly or lazily;
    - Pool `REVOKE` as a right: is it the right shape to accept first, and does it stand if RFC-0003b
    is rejected?
- **RFC-0003b §13:**
    - sovereignty against the funder chain;
    - what a server may depend on;
    - a Pool depth bound of 8;
    - who holds `SchedControl`/`IrqControl`;
    - two new Pool-row rights: `WRITE` on `into`, and `pool_split`'s returned rights.
- **Carried over:**
    - C1, priority inheritance;
    - the rule on MCP for rightless capabilities;
    - accepting verdicts in tranches.
- **From the gap report §(b):**
    - Dependabot cadence;
    - Codecov;
    - a merge queue;
    - Copilot's standing;
    - the wording of the never-do rules;
    - security test obligations;
    - `TODO.md` versus issues;
    - when versioning starts;
    - pruning GHCR;
    - the single CI-checks script, now done.

## 6. Lessons to record in skills (next PR)

1. **A gate that can pass on an empty input is not a gate.** This is already in `check-spelling`.
   Generalise it: every gate refuses an empty input and reports its count; read the count.
2. **Verify that a flag or variable really arrives.** Read the log line itself. `RUSTFLAGS` was set
   at workflow level for months and never reached cargo.
3. **A green check is not proof.** The old `devcontainers/ci` installed an unpinned CLI at runtime,
   whatever its SHA pin suggested. Read an action's source and its real logs, not its README.
4. **RFCs converge at the core, not at the seams.** If a third review round is needed, model it or
   split it rather than patch it.
5. **Keep reviewers out of the author's worktree.** A reviewer uses its own worktree, checked out
   with `--detach`.
6. **Sequence PRs that touch the same files** (#28 and #29 needed manual reconciliation).
7. **Never filter push output.** Always compare HEAD with `ls-remote`.
8. **`awk length` counts bytes.** Measure line lengths in characters with Python.
9. **When merging a stacked PR, merge `main` into the next one.** The ruleset's strict up-to-date
   rule makes every merge stale the others.

## 7. Follow-ups noticed but not yet done

- **A GHCR retention policy.** Each merge keeps an image of about 800 MB alive under `:sha`.
- **`.devcontainer/Dockerfile` installs Claude Code unpinned.** That conflicts with CLAUDE.md's rule
  that tools are pinned and verified.
- **A bot identity for Claude's PRs** (§2).
- **An RFC-0007 inconsistency.** §18's increment 9b grants a `WRITE`-only Console through `GRANT`,
  which needs `TRANSFER`.
- **The `.github/CODEOWNERS` contact address** is `matejg03@gmail.com`, while the maintainer's
  account uses `matejg04`. Ask before changing it.
- **The `claude/rfc-0003b-reclaim` branch** carries RFC-0003a's CHANGELOG entry from before the
  final edits, which says 412 lines. Refresh it from `main` when RFC-0003b moves.
- **One `main` push costs about 6 minutes** for the no-cache build, well within the 30-minute
  timeout.
