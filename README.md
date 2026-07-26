<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix kernel

The Setonix microkernel — hand-written in Rust, providing mechanism and never
policy.

```
  (\_/)
 ( •_•)
 / ></
```

## Design in one paragraph

Capabilities are the only authority: unforgeable, kernel-validated, explicitly
passed, and reducible in rights. IPC is the product — a register-based fast path
for small messages with a direct sender-to-receiver switch, and zero-copy page
transfer for large ones. Drivers live in userspace, so a crashing driver
restarts without taking the system down. The kernel is minimal by construction,
because everything above it is reached through broker-mediated schemes rather
than ambient authority.

The reasoning behind all of that, and the lineage of each borrowed idea, is in
the [constitution](https://github.com/setonix-os/docs/blob/main/CLAUDE.md).

## Status

Phase 1, first milestone: the kernel boots on QEMU aarch64 `virt` and greets you
on the PL011 UART. That is all it does — there is no scheduler, no IPC, no
capability table and no MMU yet. What it does prove is the chain everything else
rests on: link script, boot stub, stack, `.bss`, the hardware-abstraction
boundary, and the console.

x86_64 compiles and links against the same architecture-independent kernel, which
is what keeps the hardware-abstraction boundary honest, but it does not boot yet —
`q35` needs a UEFI stub first.

## Building

The canonical environment is the devcontainer — see [.devcontainer/](.devcontainer/).

```bash
cargo xtask build     --arch aarch64            # cross-compile
cargo xtask run-qemu  --arch aarch64            # boot, serial on this terminal
cargo xtask run-qemu  --arch aarch64 --debug    # halt at reset, gdb on :1234
cargo xtask boot-test --arch aarch64 --expect "Kaya!"
```

`boot-test` is what CI gates on. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
full development setup and [CLAUDE.md](CLAUDE.md) for the rules that apply to
kernel code.

## Licence

GPLv3 for original Setonix code — see [LICENCE](LICENCE). Vendored
MIT-licensed components retain their notices and are incorporated under GPLv3 as
the MIT licence permits.
