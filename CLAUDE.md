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
xtask/           build, run and boot-test automation (host binary, no deps)
```

## Architectures

Tier 1, both first-class from day one: `aarch64-unknown-none-softfloat` and
`x86_64-unknown-none`. The HAL boundary lives at `kernel/src/arch/mod.rs`;
nothing above it may name an architecture. Bring-up order is QEMU aarch64
`virt`, then QEMU x86_64 `q35`, then real hardware.

**Both targets are soft-float**, and the AArch64 one needs its `-softfloat`
variant named explicitly. The plain `aarch64-unknown-none` permits NEON, LLVM
emits FP/SIMD for ordinary data movement in debug builds, and `CPACR_EL1.FPEN`
is 0 at reset — so the first such instruction traps to a vector table that does
not exist yet and the kernel dies before its first character reaches the UART.
Independently of that, a microkernel must not touch registers it would have to
save and restore on every context switch: FP/SIMD belongs to userspace, enabled
per process and saved lazily, once there is a userspace.

aarch64 boots. x86_64 compiles and links but does not boot: `q35` has no
bare-ELF equivalent of `-kernel`, so it needs a UEFI stub first. Its entry point
nevertheless calls straight through into the kernel proper, deliberately — an
entry that merely halted would leave everything above `arch` as dead code on that
target, and the second Tier-1 build would then prove nothing.

## Building

```bash
cargo xtask build     --arch aarch64 [--release]
cargo xtask run-qemu  --arch aarch64 [--debug]        # --debug: gdb stub on :1234
cargo xtask boot-test --arch aarch64 --expect "Kaya!"
```

Always name the package when invoking cargo directly. The workspace holds two
crates with incompatible targets — the kernel only cross-compiles, `xtask` only
builds for the host — so an unscoped `--target` will try to build the host tool
for bare metal:

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

## Development environment

Canonical setup is the devcontainer in [.devcontainer/](.devcontainer/) —
"Reopen in Container" and the identical workshop exists for every contributor.
Clone into a container volume or the WSL-side filesystem, never a bind mount
from `/mnt/c`: the Windows-filesystem bridge is many times slower and is felt on
every rebuild.

## Licence

GPLv3 — see [LICENCE](LICENCE). New files carry an SPDX header.
