<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix kernel — repo guide

> **The project constitution lives in the sibling `docs` repo, at
> `../docs/CLAUDE.md`, and is binding.** It is the single canonical copy. Read
> it before working here. This file adds only what is specific to *this* repo:
> how to build it, how to run it, and the rules that apply to kernel code.

## What belongs here

Everything that boots. The kernel crate, the hardware-abstraction layer, the
per-architecture boot stubs, the pinned devcontainer, CI, and the build tooling.

## What does not belong here — yet

Drivers, filesystem, libc and the network stack are all *port* verdicts in the
constitution's Borrow Ledger, and they will eventually want their own repos.
They start life as members of this Cargo workspace instead, because the HAL and
IPC ABI are still moving: splitting them out now would turn every interface
change into a synchronised multi-repo dance for a single maintainer. Promote
them out once the ABI stops moving — Phase 2 or 3, not before.

Vendored MIT-licensed code (Redox and similar) goes under `vendor/`, one
subdirectory per upstream crate, each retaining its own `LICENSE` verbatim, with
a top-level `vendor/NOTICE.md` indexing provenance. A per-directory licence
boundary satisfies the obligation exactly as well as a repo split, without the
friction.

## Architectures

Tier 1, both first-class from day one: `aarch64-unknown-none` and
`x86_64-unknown-none`. The HAL boundary lives at `kernel/src/arch/mod.rs`;
nothing above it may name an architecture. Bring-up order is QEMU aarch64
`virt`, then QEMU x86_64 `q35`, then real hardware.

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
commit. `Cargo.lock` **is** committed — this is a binary, and the project's
reproducibility ethos applies to its own build before it applies to anyone
else's.

## Development environment

Canonical setup is the devcontainer in [.devcontainer/](.devcontainer/) —
"Reopen in Container" and the identical workshop exists for every contributor.
Clone into a container volume or the WSL-side filesystem, never a bind mount
from `/mnt/c`: the Windows-filesystem bridge is many times slower and is felt on
every rebuild.

## Licence

GPLv3 — see [LICENSE](LICENSE). New files carry an SPDX header.
