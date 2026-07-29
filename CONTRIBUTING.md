# Contributing to the Setonix documents

This repository is the project's paper trail. Design disagreements are settled here, where being
wrong is cheap (constitution §5.5).

## Read the Constitution First

[`CLAUDE.md`](CLAUDE.md) in this repository is the project constitution and is binding project law for
every contributor, human or AI. It is the single canonical copy — it deliberately exists nowhere else,
so that it can never diverge.

**It is revised only by the maintainer** (§4, §11.1). You are welcome and encouraged to argue that it
is wrong; you may not amend it in a pull request.

## Code of Conduct

This project adheres to the Contributor Covenant. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## What Belongs Here

| Document | Purpose |
|----------|--------|
| `CLAUDE.md` | The constitution: the primitive, the pillars, kernel doctrine, the Borrow Ledger |
| `threat-model.md` | Expansion of the constitution's seed threat model |
| `rfcs/` | One document per design decision, with rationale and rejected alternatives |

Code belongs in [`setonix-os/kernel`](https://github.com/setonix-os/kernel), not here.

## Writing an RFC

An RFC earns its place by making a decision cheaper to revisit later. State, in this order:

1. **The question.** One sentence. If you cannot compress it to one sentence, it is two RFCs.
2. **Which pillar it serves.** Constitution §1: an addition that is not a consequence of the primitive
   does not go in, however attractive. If the answer is "none", say so — that is a finding, not a
   failure.
3. **The options considered.** Including the one you rejected and why. An RFC listing only the chosen
   design is a decision without a record, which is the thing RFCs exist to prevent.
4. **The lineage.** Which prior system solved this, and what it got wrong. The constitution's ledger
   names lineage for every borrowed idea; keep the habit.
5. **The graves.** Whether the design walks into one of the failures §3 names — multi-copy IPC, policy
   in the kernel, baroque capability hierarchies, bolted-on multicore.
6. **What it costs.** What becomes harder, not just what becomes possible.

Number RFCs sequentially. Do not renumber or delete a rejected one: a rejected RFC is a permanent
record of a question already settled, and is often more useful than an accepted one.

## Amending the Constitution

Only the maintainer may. If an amendment lands, it must leave no stale clause behind — a constitution
with two clauses contradicting each other is worse than one that is merely wrong, because it makes
every future reader guess. Check cross-references before merging.

## Pull Requests

- [ ] British spelling (`bash .github/scripts/check-british-spelling.sh`)
- [ ] Markdown lint-clean (`markdownlint-cli2 "**/*.md"`)
- [ ] Cross-references resolve; no clause left contradicting another
- [ ] `CHANGELOG.md` updated under `[Unreleased]` for substantive changes
- [ ] Commit messages follow Conventional Commits
- [ ] Every commit is GPG-signed and shows **Verified** on GitHub

## Commit Messages

[Conventional Commits](https://www.conventionalcommits.org/). In this repository the useful types are
`docs`, `chore` and `ci`. Examples:

```text
docs(threat-model): add compromised-author adversary and its mitigations

docs(rfcs): add RFC-0001 on the hardware-abstraction boundary
```

Imperative mood, no capital after the colon, no full stop, subject under 72 characters.

Every commit is GPG-signed — the constitution's third pillar is author-signed artefacts, and the
project's own history holds itself to the same standard. Configure `git config commit.gpgsign true`
with your signing key, and add the public key to your GitHub account so commits show **Verified**.

## Style

See [STYLE.md](STYLE.md). Briefly: 4-space indentation, 170-column lines, ATX headings, dash lists,
fenced code blocks with a language, British spelling.
