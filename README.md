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

Pre-code. The repository holds the pinned toolchain and development environment;
the first milestone is `Kaya!` and the resident critter on the QEMU aarch64
`virt` PL011 UART.

## Building

Requires the devcontainer — see [.devcontainer/](.devcontainer/) and
[CLAUDE.md](CLAUDE.md). Build and run instructions land with the first commit
that produces a bootable image.

## Licence

GPLv3 for original Setonix code — see [LICENSE](LICENSE). Vendored
MIT-licensed components retain their notices and are incorporated under GPLv3 as
the MIT licence permits.
