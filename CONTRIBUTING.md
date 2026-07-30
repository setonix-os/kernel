# Contributing to the Setonix kernel

Thank you for your interest in Setonix. Please read this before opening a pull request.

## Table of Contents

- [Read the Constitution First](#read-the-constitution-first)
- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Coding Standards](#coding-standards)
- [Commit Messages](#commit-messages)
- [Testing](#testing)
- [Documentation](#documentation)

---

## Read the Constitution First

Setonix is governed by a written constitution: [CONSTITUTION.md](CONSTITUTION.md), at the root of this
repository. It is binding on every contributor, human or AI, and it is short.

Two of its rules shape every contribution here:

- **Coherence over accumulation** (§1, §5.4). Every addition must justify itself as a consequence of
  the project's primitive. A good feature that does not follow from it is still rejected. This is not
  gatekeeping for its own sake — it is the only reason the system will still be explainable in five
  years.
- **Nothing merges un-understood** (§5.2, §11.4). If the maintainer cannot explain your change, it
  does not land, however correct it is. Ship the explanation with the code.

If you believe the constitution is wrong about something, that is a legitimate position — argue it on
paper, in an issue or an RFC under [docs/rfcs/](docs/rfcs/), where being wrong is cheap. Do not route
around it in a pull request. The constitution itself is revised only by the maintainer.

---

## Code of Conduct

This project adheres to the Contributor Covenant. By participating you are expected to uphold it.
See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

---

## How to Contribute

### Reporting Bugs

Use the bug report template. For a kernel, two details matter more than anything else and are the
ones most often missing: **the full serial console output from reset onwards**, and **whether it
reproduces on the other Tier-1 architecture**. A fault on exactly one architecture usually means the
hardware-abstraction boundary has leaked, which is a different bug from the one you are reporting.

### Suggesting Features

Before opening a feature request, check it against the primitive. A proposal that names which pillar
it follows from will be taken seriously; one that does not will mostly generate a conversation about
whether it belongs at all.

### Pull Requests

1. Open an issue first for anything beyond a fix or a typo
2. Check the **Borrow Ledger** (constitution §4) for the subsystem you are touching. If its verdict
   is "write ourselves", a port will be rejected; if "port code", hand-written work needs a reason. If
   the verdict is unclear, **ask** — §11.2 is explicit that contributors do not decide this
3. Branch from `main`
4. Fill in the PR template completely, including the `unsafe` register — leave it saying "None"
   rather than deleting it, because its emptiness is the useful signal

#### PR Requirements

- [ ] Builds for both Tier-1 targets (`aarch64-unknown-none-softfloat`, `x86_64-unknown-none`)
- [ ] Clippy passes (`cargo clippy --all-targets --all-features -- -D warnings`)
- [ ] Formatted (`cargo fmt --all --check`)
- [ ] Toolchain pins agree (`bash .github/scripts/check-toolchain-pin.sh`)
- [ ] British spelling (`bash .github/scripts/check-british-spelling.sh`)
- [ ] Markdown lint-clean (`markdownlint-cli2 "**/*.md"`)
- [ ] Boots in QEMU where applicable
- [ ] Every new `unsafe` block is inside a designated module, carries a `// SAFETY:` comment, and is
      listed in the PR
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Commit messages follow [Conventional Commits](#commit-messages)
- [ ] Every commit is GPG-signed and shows **Verified** on GitHub (see [Signed Commits](#signed-commits))

### Writing an RFC

Design decisions are settled on paper under [docs/rfcs/](docs/rfcs/), where being wrong is cheap
(constitution §5.5). An RFC earns its place by making a decision cheaper to revisit later. State, in
this order:

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
record of a question already settled, and is often more useful than an accepted one. Accepted RFCs are
never silently edited — changes go in an **Amendments** section, dated, with the reasoning.

### Amending the constitution

Only the maintainer may amend `CONSTITUTION.md`. When an amendment lands, it must leave no stale clause
behind: a constitution with two clauses contradicting each other is worse than one that is merely
wrong, because it makes every future reader guess which to obey. Check cross-references before merging —
`/check-coherence` automates exactly this sweep — and never renumber a section that other documents
cite, since the citations break silently.

---

## Development Setup

### Canonical: the devcontainer

The pinned toolchain is the whole point — see the constitution §7. Open the repository in VS Code and
choose **Reopen in Container**. Everything is pinned in [.devcontainer/](.devcontainer/) and CI uses
the same versions.

**Clone into a container volume or the WSL-side filesystem, never a bind mount from `/mnt/c`.** The
Windows-filesystem bridge is many times slower and you will feel it on every rebuild.

On Windows, Docker Desktop must use the **WSL 2** backend. WSL 1 cannot run this.

### Prerequisites if you insist on a host toolchain

CI runs the devcontainer image, so this is the unsupported path — you are reproducing by hand what the
container gives you, and any difference is yours to debug.

- Rust, pinned in `rust-toolchain.toml` and installed automatically by rustup on first `cargo` command
- `qemu-system-aarch64` and `qemu-system-x86_64` at the version the Dockerfile pins. **Your
  distribution's QEMU is probably older** — Debian 13 ships 10.0.11 against upstream's 11.0.3 — so a
  bug you see and CI does not may simply be a packager's freeze rather than a fault in the kernel
- `gdb-multiarch`, OVMF and AAVMF UEFI firmware, `mtools`, `dosfstools`
- Node and `markdownlint-cli2`, at the versions the Dockerfile pins

The Dockerfile is the specification; read it rather than guessing, since it names an exact version and
a verified download for each of these.

### Commands

```bash
cargo xtask build     --arch aarch64 [--release]
cargo xtask run-qemu  --arch aarch64            # boot, serial on this terminal
cargo xtask run-qemu  --arch aarch64 --debug    # halt at reset, gdb stub on :1234
cargo xtask boot-test --arch aarch64 --expect "Kaya!"
cargo fmt --all
```

When invoking cargo directly, **always name the package**. The workspace holds two
crates with incompatible targets — `setonix-kernel` only ever cross-compiles,
`xtask` only ever builds for the host — so an unscoped `--target` tries to build
the host tool for bare metal and fails for a reason unrelated to your change:

```bash
cargo clippy --package setonix-kernel --target aarch64-unknown-none-softfloat -- -D warnings
cargo clippy --package setonix-kernel --target x86_64-unknown-none  -- -D warnings
cargo clippy --package xtask --all-targets -- -D warnings
cargo test   --package xtask
```

---

## Coding Standards

See [STYLE.md](STYLE.md) for the full conventions. The rules that get changes rejected:

### `unsafe`

Permitted only in the modules `CLAUDE.md` designates, and enforced by `unsafe_code = "deny"` at the
workspace level — a designated module opts in visibly with `#![allow(unsafe_code)]`. Every block
carries a `// SAFETY:` comment naming the invariant, not restating the code.

The full set of designated modules is greppable, deliberately:

```bash
grep -rn "allow(unsafe_code)" kernel/src/
```

If that list grows, a reviewer will ask why.

### The architecture boundary

Nothing above `kernel/src/arch/mod.rs` may name an architecture. No `#[cfg(target_arch)]` above the
HAL. Both Tier-1 architectures are first-class; neither is the one that only builds "usually".

### Mechanism, not policy

The kernel provides mechanism. Defaults, heuristics and permission rules belong in userspace. Policy
in the kernel is one of the graves the constitution names explicitly.

### British Spelling 🇬🇧

British spelling in documentation, comments and console output. The enforced word list is in
`.github/scripts/check-british-spelling.sh`.

Code identifiers may use American spelling where it matches a Rust or hardware convention — a
register field named in a datasheet is quoted, not corrected. Vendored MIT code is never reworded:
its provenance is a licence obligation.

---

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no code change |
| `refactor` | Neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding or updating tests |
| `chore` | Maintenance tasks |
| `ci` | CI/CD changes |
| `security` | Security improvements |

### Examples

```text
feat(ipc): add register-based fast path for messages under 32 bytes

fix(mmu): invalidate TLB after unmapping a range

chore(toolchain): bump Rust to 1.95.0 in all three pinned locations
```

### Rules

- Imperative mood ("add feature", not "added feature")
- Do not capitalise the first letter of the description
- No full stop at the end of the subject line
- Subject line under 72 characters
- Reference issues in the footer: `Fixes #123`

### Signed Commits

Every commit is GPG-signed. The project's third pillar is author-signed artefacts, and its own history
holds itself to the standard it asks of app authors: all of `main` shows **Verified** on GitHub, from
the root commit onwards. Set it up once —

```bash
git config user.signingkey <your-key-id>
git config commit.gpgsign true
```

— and add the public key to your GitHub account so the badge reads Verified. To re-sign an existing
branch: `git rebase --exec "git commit --amend --no-edit -S" main`.

---

## Testing

A kernel cannot run a normal test harness on its target, so tests are split by what they can reach:

| Kind | Where | Runs on |
|------|-------|---------|
| Unit tests of architecture-independent logic | `#[cfg(test)]` beside the code | Host |
| Integration tests | `tests/` | Host |
| Boot and hardware behaviour | `cargo xtask boot-test` | QEMU, both architectures |

The boot smoke test is the one that matters most and the cheapest to keep honest: the kernel must
reach its console and print its greeting. If that breaks, nothing above it can be trusted.

Write tests that would fail for the right reason. A test that passes because a stub returns `Ok(())`
is worse than no test.

---

## Documentation

| Location | Purpose |
|----------|--------|
| `CONSTITUTION.md` | The constitution — binding project law |
| `docs/rfcs/` | One document per design decision, with rationale and rejected alternatives |
| `docs/CHANGELOG.md` | Amendment log for the constitution and the design documents |
| `CLAUDE.md` | Repo-local rules: build, `unsafe` policy, HAL boundary |
| `README.md` | User-facing overview |
| `CONTRIBUTING.md` | This file |
| `STYLE.md` | Style conventions |
| `SECURITY.md` | Security policy and vulnerability reporting |
| `CHANGELOG.md` | User-facing change history |
| Rustdoc comments | API documentation (`cargo doc --open`) |

Update `CHANGELOG.md`'s `[Unreleased]` section for all notable changes, and bump `STYLE.md`'s
*Last updated* line when you change a convention.

---

## Questions?

- Design questions belong in [discussions](https://github.com/setonix-os/kernel/discussions) or an RFC
- Check existing issues first
- Be patient — Setonix is a deliberately small team, and every change crosses the maintainer's desk
