<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix kernel

The Setonix microkernel — hand-written in Rust, providing mechanism and never
policy.

```
  (\_/)
 ( •_•)
 / ></
```

**Setonix** — /ˈsɛtənɪks/, *SET-oh-niks* — is the genus of the quokka,
*Setonix brachyurus*: Latin *seta* "bristle" and Greek *ónyx* "claw", for the
bristles around the claws of its hind feet.

**Kaya** — /ˈkaja/, *KAH-ya*, stress on the first syllable, never *ka-YA* — is
Noongar for both "hello" and "yes", and is answered with *Kaya* in return. It is
the first word this kernel says.

Both notes are summarised from [the constitution §8](CONSTITUTION.md), which
holds the full derivation and records that the pronunciation is offered in good
faith pending confirmation with Noongar language custodians.

## Design in one paragraph

Capabilities are the only authority: unforgeable, kernel-validated, explicitly
passed, and reducible in rights. IPC is the product — a register-based fast path
for small messages with a direct sender-to-receiver switch, and bulk data moving
through shared-memory regions established out of band, never through the message
itself (RFC-0004). Drivers live in userspace, so a crashing driver
restarts without taking the system down. The kernel is minimal by construction,
because everything above it is reached through broker-mediated schemes rather
than ambient authority.

The reasoning behind all of that, and the lineage of each borrowed idea, is in
the [constitution](CONSTITUTION.md).

## Documents

The paper trail lives in this repository, beside the code it governs:

- [CONSTITUTION.md](CONSTITUTION.md) — the founding document. Binding on every
  contributor, human or AI. Start here.
- [docs/rfcs/](docs/rfcs/) — one document per design decision, with its
  rationale and the alternatives rejected.
- [docs/CHANGELOG.md](docs/CHANGELOG.md) — the amendment log for the
  constitution and the design documents.
- [docs/threat-model.md](docs/threat-model.md) — what Setonix defends and the
  numbered obligations its design must discharge; expands the constitution's §9
  seed.

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

## Acknowledgement

Setonix borrows words from the Noongar language with acknowledgement, and
commissions rather than imitates visual language. We acknowledge the Noongar
people as the traditional custodians of the country whose language and seasons
give this project its names, and pay our respects to their elders past and
present.

## Licence

GPLv3 for original Setonix code — see [LICENCE](LICENCE). Vendored
MIT-licensed components retain their notices and are incorporated under GPLv3 as
the MIT licence permits.
