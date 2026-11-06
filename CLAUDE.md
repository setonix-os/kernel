<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix kernel — repo guide

> **The project constitution is [CONSTITUTION.md](CONSTITUTION.md), at the root
> of this repository, and is binding.** It is the single canonical copy. Read it
> before working here. This file adds only what is operational: how to build,
> how to run, and the rules that apply to kernel code. Design records live in
> [docs/rfcs/](docs/rfcs/); amendments to the documents are logged in
> [docs/CHANGELOG.md](docs/CHANGELOG.md).

## What belongs here

Everything that boots. The kernel crate, the hardware-abstraction layer, the
per-architecture boot stubs, the pinned devcontainer, CI, and the build tooling.

## What does not belong here — yet

Drivers, filesystem, libc and the network stack are all *port* verdicts in the
constitution's Borrow Ledger, and they will eventually want their own repos.
They start life as members of this Cargo workspace instead, because the HAL and
IPC ABI are still moving: splitting them out now would turn every interface
change into a synchronised multi-repo dance for a deliberately small team. Promote
them out once the ABI stops moving — Phase 2 or 3, not before.

Vendored MIT-licensed code (Redox and similar) goes under `vendor/`, one
subdirectory per upstream crate, each retaining its own licence file **under the
upstream's own filename and spelling** — usually `LICENSE` — with a top-level
`vendor/NOTICE.md` indexing provenance. Our British `LICENCE` convention applies
to our files, never to a quoted one. A per-directory licence
boundary satisfies the obligation exactly as well as a repo split, without the
friction.

## Layout

```text
kernel/          the kernel crate (no_std, no_main)
  src/main.rs    architecture-independent entry; contains no unsafe, ever
  src/arch/      the HAL boundary — the only tree that knows the architecture
  link/          per-architecture linker scripts
capability/      the capability table (RFC-0003): pure no_std logic, host-tested
xtask/           build, run and boot-test automation (host binary, no deps)
```

Architecture-independent kernel logic that benefits from host unit tests lives in
its own `no_std` workspace crate (like `capability/`), because the kernel crate
cannot run a test harness on a bare-metal target. Such a crate is `#![no_std]` in
the kernel build and `std` under `cargo test`; it builds for both Tier-1 targets
in CI to prove it stays `no_std`.

## Architectures

Tier 1, both first-class from day one: `aarch64-unknown-none-softfloat` and
`x86_64-unknown-none`. The HAL boundary lives at `kernel/src/arch/mod.rs`;
nothing above it may name an architecture. Bring-up order is QEMU aarch64
`virt`, then QEMU x86_64 `q35`, then real hardware.

**Both targets are soft-float**, and the AArch64 one needs its `-softfloat`
variant named explicitly. The plain `aarch64-unknown-none` permits NEON, LLVM
emits FP/SIMD for ordinary data movement in debug builds, and `CPACR_EL1.FPEN`
is 0 at reset — so the first such instruction trapped to a vector table that
did not yet exist, and the kernel died before its first character reached the
UART — the fault the exception reporter now names outright.
Independently of that, a microkernel must not touch registers it would have to
save and restore on every context switch: FP/SIMD belongs to userspace, enabled
per process and saved lazily, once there is a userspace.

aarch64 boots. x86_64 compiles and links but does not boot. QEMU's x86 `-kernel`
loads a 64-bit ELF only if it carries a Xen PVH note
(`XEN_ELFNOTE_PHYS32_ENTRY`), which this image does not; whether x86_64 enters
through PVH, a UEFI stub or both is a decision for its boot RFC. Its entry point
nevertheless calls straight through into the kernel proper, deliberately — an
entry that merely halted would leave everything above `arch` as dead code on
that target, and the second Tier-1 build would then prove nothing.

## Building

```bash
cargo xtask build     --arch aarch64 [--release]
cargo xtask run-qemu  --arch aarch64 [--debug]        # --debug: gdb stub on :1234
cargo xtask boot-test --arch aarch64 --expect "Kaya!"
```

Before pushing, run everything CI's checks job runs with the one command in
[CONTRIBUTING.md § Before pushing](CONTRIBUTING.md#before-pushing).

Always name the package when invoking cargo directly. The workspace's crates
have incompatible targets — the kernel only cross-compiles, `xtask` only builds
for the host, and `capability` builds for both — so an unscoped `--target` will
try to build the host tool for bare metal:

```bash
cargo clippy --package setonix-kernel --target aarch64-unknown-none-softfloat -- -D warnings
cargo test   --package xtask
```

## `unsafe` policy

The constitution restricts `unsafe` to explicitly designated modules. In this
repo those are, and are only:

- `kernel/src/arch/**` — MMIO, system registers, page tables, context switching.
- `kernel/src/mm/**` — physical frame and page-table manipulation.

Every `unsafe` block carries a `// SAFETY:` comment stating the invariant that
makes it sound and who upholds it. Every new `unsafe` block is listed in the
session summary. `unsafe` outside those trees is a review failure, not a
judgement call.

## Toolchain

Pinned in [rust-toolchain.toml](rust-toolchain.toml); the devcontainer's
`RUST_VERSION` build-arg mirrors it. Bump both together in one deliberate
commit, then run `.github/scripts/check-toolchain-pin.sh` — which also fails if
any workflow starts installing a toolchain of its own, since CI runs the
devcontainer image and must have no second source of truth.

Workflows and composite actions use only actions owned by GitHub (`actions/`,
`github/`) or by `setonix-os/`, pinned as
[STYLE.md § Action Pinning](STYLE.md#action-pinning) requires. The repository's
Actions settings allow no other owner, and the same script fails any `uses:` that
breaks either rule. Anything else a job needs is a `run:` step: the devcontainer,
for one, is built and entered by `.github/scripts/devcontainer.sh`.

`Cargo.lock` **is** committed — this is a binary, and the project's
reproducibility ethos applies to its own build before it applies to anyone
else's.

The same reasoning governs [.devcontainer/Dockerfile](.devcontainer/Dockerfile):
Rust, Node and QEMU come from their authors at pinned versions, verified by hash
or signature, rather than from apt. Debian 13 ships QEMU 10.0.11 against
upstream's 11.0.3 — a major version of the emulator this kernel is tested on,
withheld by a packaging decision. Distribution packages remain only for tools
that publish no upstream binary, and each is labelled in the Dockerfile as a
gatekeeper not yet removed. Adding a tool means adding it there, with a pinned
version and a verified download.

## Claude Code skills

Project skills live in `.claude/skills/<name>/SKILL.md` and are invoked as `/<name>`. A skill
records how this project does a recurring job, and it ends with a dated **Lessons** list. When
work goes wrong, or better than expected, the lesson goes into the skill it concerns in the same
pull request as the fix, with the date and the cause, so the next session starts from it.

Where the work stands lives in [TODO.md](TODO.md) § Resume here, which `/handoff` overwrites.
Start a session with `git fetch origin`, then `git show origin/main:TODO.md`: the working tree may be
behind it.

## Never do these

Those that cite a source restate it, so that a session meets the rule where it reads its
instructions. The others are owned here.

- Never push to `main`, merge a pull request, or approve one: the maintainer alone merges
  (constitution §5.1).
- Never amend `CONSTITUTION.md` or the Borrow Ledger of your own accord: raise the question, and draft
  only on the maintainer's instruction (`CONTRIBUTING.md` § Amending the constitution).
- Never edit `CODE_OF_CONDUCT.md` or `LICENCE`, and never vendored text (constitution §11.5).
- Never change GitHub settings, rulesets or package visibility: advise, and the maintainer applies.
- Never bypass signing or hooks, and never loosen a check to pass it: change the content, or propose
  a check's own recorded exception to the maintainer.
- Never add a workflow-installed toolchain (constitution §7).
- Never litter. Everyone, every session and every agent, removes what they made once done: temporary
  files, worktrees, throwaway branches, task-only containers and processes left running. Throwaway
  build output goes in the session scratchpad or the system temporary folder, never beside the
  project. Clean up only what you made: uncommitted work, stashes and unpushed commits are not litter.

## Development environment

Canonical setup is the devcontainer in [.devcontainer/](.devcontainer/) —
"Reopen in Container" and the identical workshop exists for every contributor.
Clone into a container volume or the WSL-side filesystem, never a bind mount
from `/mnt/c`: the Windows-filesystem bridge is many times slower and is felt on
every rebuild.

## Licence

GPLv3 — see [LICENCE](LICENCE). New files carry an SPDX header.

The design documents cite Linux, the seL4 kernel and QEMU down to the file. Read and
cite them freely; copy from them never. Linux and seL4's kernel code are
GPL-2.0-only, which cannot be combined with this GPLv3 tree; QEMU is GPL version 2
as a whole and licensed file by file, so even its GPL-2.0-or-later parts would need
a per-file provenance check that `vendor/`, which takes MIT code, does not provide
for. An increment re-derives the mechanism from the architecture manuals and the
cited design, in its own words and code.
