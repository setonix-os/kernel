# Setonix — Founding Document

> *Setonix does to operating systems what Rust did to systems languages:*
> *keep the proven, prune the legacy.*
>
> This file is the project constitution. It doubles as the repo-root `CLAUDE.md`:
> every Claude Code session reads it at start-up and is bound by it, exactly as
> human contributors are.

**What:** An original operating system, written from scratch in Rust, freely
adopting the best proven ideas — and, where licensing permits, code — from four
decades of OS research and practice.

**Name & mascot:** *Setonix*, after *Setonix brachyurus* — the quokka. The
mascot is **Kaya** (Noongar: "hello"/"yes") — the world's happiest animal,
greeting you by name. (Founded as BlueOS; renamed July 2026 to escape a
crowded namespace.)

**Licence:** GPLv3 for all original Setonix code. Vendored MIT-licensed
components (e.g. from Redox OS) retain their notices and are incorporated under
GPLv3 as permitted by the MIT licence.

**Language:** Rust. No exceptions beyond the minimal assembly required for boot
and context switching.

**Status:** Pre-code. Design phase. This document precedes the first commit.

---

## 1. The Primitive

Everything in Setonix is a consequence of one primitive:

> **An application is a self-contained, signed, identity-bearing, immutable,
> content-addressed blob — and every resource it touches is a scheme mediated
> by a broker.**

Any proposed feature, subsystem, or borrowed idea must justify itself as a
consequence of this primitive. If it cannot, it does not go in — however
attractive it is. Coherence beats accumulation; darlings get killed.

## 2. The Pillars

Each pillar is the primitive expressed in one domain, with its lineage noted:

1. **Immutable binaries, mutable data** — an app's executable tree is
   read-only and separated from its config, cache, and state. *(Haiku)*
2. **Explicit permission brokering** — apps declare needed capabilities up
   front; a broker grants, denies, and mediates at runtime. No ambient
   authority anywhere. *(Android's model; Plan 9 per-process namespaces as the
   implementation substrate)*
3. **Author-signed, self-distributing updates** — developers ship directly to
   users, signed with their own identity. No distro gatekeepers.
   *(Firefox-on-Windows model, not the APT-curator model)*
4. **Content-addressed, strictly versioned dependencies** — every dependency
   closure is exact and immutable. Dependency hell is structurally impossible.
   *(Nix)*
5. **Everything is a scheme** — files, devices, networks, and libraries are
   reached through a unified URL-style namespace (`file://`, `device://`,
   `net://`, `lib://`), which is also where the broker interposes.
   *(Redox / Plan 9)*

## 3. Kernel Doctrine

The kernel is hand-written and minimal. It provides mechanism, never policy.

- **Microkernel** with userspace drivers; a crashing driver restarts without
  taking the system down. *(Redox, QNX)*
- **Capabilities are the only authority.** Unforgeable, kernel-validated,
  explicitly passed, and reducible in rights. Rust's ownership and move
  semantics model capability transfer at compile time. *(seL4; Fuchsia's
  downgradeable handle rights)*
- **IPC is the product.** Register-based fast path for small messages, direct
  sender→receiver switch, zero-copy page transfer for large ones. *(L4)*
- **Everything is a message**, with priority inheritance to prevent priority
  inversion. *(QNX)*
- **Typed, bidirectional channels** as the userspace-facing IPC abstraction.
  *(Fuchsia/Zircon)*
- **Known graves to avoid:** multi-copy IPC (Mach), policy in the kernel
  (early microkernels), baroque capability hierarchies (KeyKOS), bolted-on
  multicore support.

## 4. The Borrow Ledger

Initial verdicts — revised only by the maintainer. **Author** records who
produces the code; the maintainer reviews and must understand everything
regardless. Where a row cites an RFC, that document holds the reasoning and the
alternatives rejected; the row is the verdict, not the argument.

| Subsystem | Verdict | Source / lineage | Author |
|---|---|---|---|
| Microkernel core (scheduler, IPC, capabilities, MMU) | Write ourselves | Earlier C++17 blueprint, re-expressed in Rust | Human-first, AI as sparring partner |
| Boot path (asm stub, early init) | Write ourselves | Redox aarch64 port as reference | Human-first |
| Hardware drivers | Port code | Redox driver corpus (MIT) | AI-first, human-reviewed |
| Filesystem | Port code | RedoxFS (MIT). Serves `file://` for mutable data, and backs the store's substrate. Revisit only if verification at rest or measurement demands it — not on a schedule. *(RFC-0001)* | AI-first, human-reviewed |
| libc / runtime | Port code | relibc pieces (MIT), trimmed | AI-first, human-reviewed |
| Network stack | Port code | Redox (MIT) | AI-first, human-reviewed |
| Scheme registry & namespace | Write ourselves | Redox schemes + Plan 9 namespaces, design only | Human-first |
| Permission broker | Write ourselves | Android model, design only | Human-first |
| App format, signing, content-addressed store | Write ourselves | Nix + Haiku, design only. This row owns the store's **semantics and interface** — which is what the pillars rest on — and not its on-disk substrate. *(RFC-0001)* | Human-first |
| App manager & updater | Write ourselves | Original project spec | Mixed |
| Build system, CI, tests, tooling | Write ourselves | — | AI-first, human-reviewed |

## 5. Working Principles

1. **The human holds the steering wheel.** AI builds; the maintainer directs,
   oversees, and decides. Control and understanding are non-negotiable.
2. **Nothing merges un-understood.** If the maintainer cannot explain a change,
   it does not land. AI compresses drudgery, never understanding.
3. **Hand-write the learning core.** Kernel hot paths are written by the
   maintainer; toil (ports, boilerplate, tooling, tests, docs) is delegated
   with mandatory review.
4. **Coherence over accumulation.** Every addition must serve the primitive.
5. **Documents before code.** Design disagreements are settled on paper, where
   being wrong is cheap.

## 6. Scope and Non-Goals

**Target:** security-critical server workloads first (web servers and similar).
This deliberately sidesteps the two tensions that kill clean-design OSes:
broad POSIX ambient-authority compatibility, and the microkernel performance
tax in desktop workloads.

**Tier-1 architectures:** x86_64 and AArch64 — together covering essentially
every consumer PC of the last decade plus the modern ARM server fleet. Both
are first-class from day one, forcing a clean hardware-abstraction boundary.
RISC-V is a possible future Tier-2; ARM 32-bit is legacy and stays pruned.

**Bring-up order:** QEMU aarch64 `virt` → QEMU x86_64 `q35` → real x86_64
laptop (UEFI + ACPI + AHCI/NVMe) → Raspberry Pi. Usefulness on a given
machine is gated by drivers and firmware, not the ISA — the Borrow Ledger's
Redox ports carry that promise.

**Explicitly out of scope:** Apple Silicon Macs (non-standard boot chain — a
multi-year project of its own), Windows-on-ARM firmware quirks beyond generic
UEFI, GPU acceleration (UEFI GOP framebuffer only for now).

**Non-goals (for now):** desktop polish, bug-for-bug POSIX compatibility,
replacing Linux, supporting every board on earth.

## 7. Development Environment

- **Canonical setup: a VS Code devcontainer.** `.devcontainer/` (Dockerfile +
  devcontainer.json) lives at the root of the `kernel` repository — beside the
  code it serves, not beside this document — and pins the entire toolchain:
  the project's reproducibility ethos applied to its own dev environment. Any
  contributor, human or AI, gets the identical environment via "Reopen in
  Container", locally or in GitHub Codespaces; **CI runs that same image**, built
  from that same Dockerfile, so there is exactly one environment and no
  "works on my machine" gap to argue about. It follows that the image must be able
  to run every check a contributor is asked to run — compiler, emulator, linters
  and all. The Rust version is named in two places, `rust-toolchain.toml` and the
  Dockerfile's `ARG RUST_VERSION`; `.github/scripts/check-toolchain-pin.sh` fails
  the build if they disagree, and also fails if CI ever reintroduces a toolchain of
  its own, since that would quietly recreate the second source of truth this
  arrangement exists to remove.
- **The workshop practises the pillars on itself.** Tools are fetched from their
  authors at pinned versions and verified by hash or signature — not taken from
  the host distribution. This is not purism: Debian 13 ships QEMU 10.0.11 while
  upstream is at 11.0.3, so building our environment from distribution packages
  would mean testing the kernel on an emulator a packager froze, which is
  pillar 3's gatekeeper problem turning up inside our own build. Where a tool
  publishes no upstream binary at all — GDB, the UEFI firmware images — the
  distribution's build is used and labelled in the Dockerfile as a gatekeeper not
  yet removed.
- **Host stack:** Windows → VS Code → Docker Desktop on the **WSL 2** backend
  (never WSL 1 — a real Linux kernel is required). No native Linux install is
  needed for the QEMU phases.
- **Repository location:** clone into a container volume (or the WSL-side
  filesystem), never bind-mount from `/mnt/c` — the Windows-filesystem bridge
  is many times slower and is felt on every kernel rebuild.
- **Toolchain (pinned in the Dockerfile):** rustup with `aarch64-unknown-none`
  and `x86_64-unknown-none` targets; `qemu-system-aarch64` /
  `qemu-system-x86_64`; `gdb-multiarch` against QEMU's gdb stub over TCP;
  OVMF/AAVMF UEFI firmware; `mtools` for building boot images without root.
- **Emulation:** plain TCG is adequate for a microkernel and needs no
  container privileges; the aarch64 guest is software-emulated on an x86 host
  regardless. KVM via Windows 11 nested virtualisation is an optional
  speed-up for the x86_64 guest only. Serial console by default; QEMU's VNC
  server when a display is wanted.
- **Claude Code:** installed inside the devcontainer, following the official
  reference configuration; the container's isolation and firewall make
  well-guarded unattended sessions possible.
- **Real-hardware days:** write boot USB sticks and Pi SD cards from the
  Windows side; `usbipd-win` for USB passthrough when needed.

## 8. Identity & Artwork

- **The name:** *Setonix* is the genus of the quokka, *Setonix brachyurus* —
  the only species in it. The accepted derivation is Latin *seta*, "bristle",
  with Greek *ónyx*, "claw": the bristles around the claws of its hind feet.
  *brachyurus* is Greek *brachys* + *oura*, "short-tailed". So the full name
  reads roughly **short-tailed bristle-claw**.

  Pronounced **SET-oh-niks** — /ˈsɛtənɪks/, stress on the first syllable.

  The quokka lives only in the south-west corner of Western Australia —
  Wadjemup (Rottnest Island), Bald Island, and pockets of mainland forest.
  That is Noongar country, which is why the project's names come from Noongar
  and from nowhere else: the animal and the language belong to the same place.
  A passing European named the island after the animal and got the animal
  wrong — Willem de Vlamingh took the quokkas for large rats in 1696 and wrote
  down *'t Eylandt 't Rottenest*, "rat's nest island". Wadjemup is the older
  name, and the better one.

- **The greeting:** *Kaya* is Noongar for both "hello" and "yes", and is
  normally answered with *Kaya* in return — a call that expects a response,
  which is a fitting first word for a machine to say to you.

  Pronounced with the stress on the first syllable, **KAH-ya** — approximately
  /ˈkaja/, where the `y` is a glide. English guides render it both "KAH-yah"
  and "KY-ah"; those chase the same sound rather than disagreeing, because
  English has no tidy spelling for it. To an English ear it lands close to the
  name *Kaia*. It is never *ka-YA*.

  This pronunciation is written here as a good-faith starting point, **not as
  an authority**. Confirming it with Noongar language custodians is part of the
  acknowledgement commitment below, and takes precedence over anything in this
  paragraph.

- **Console greeting:** the kernel's first output is `Kaya!`, followed by the
  resident console critter:

  ```
    (\_/)
   ( •_•)
   / ></
  ```

- **The one clause not open to amendment:** it is a crime, within this
  project's jurisdiction, to assert that the quokka is not the happiest animal
  on earth. Submissions arguing that the smile is merely the shape of the jaw
  will be read, admired for their rigour, and rejected. Every other line in
  this document may be revised by RFC. Not this one.

- **Artwork policy:** Setonix never imitates Aboriginal visual art. Dreaming
  iconography encodes owned stories belonging to specific peoples; any
  artwork drawing on it is commissioned from, licensed by, and credited to
  Noongar artists ("Artwork: *title*, by name, nation"). Words are borrowed
  with acknowledgement; visual language is only ever commissioned.
- **Season wallpapers:** releases are codenamed after the six Noongar
  seasons, and each ships a default wallpaper in its season's palette —
  Birak (hot reds, dry golds), Bunuru (white heat, coastal blue), Djeran
  (cooling ochres), Makuru (cold rain, deep green), Djilba (first
  wildflower yellows), Kambarang (full bloom) — built from abstract
  south-west landforms, the Koodjal Koodjal Djookan star field, and a small
  hopping Kaya as the recurring easter egg. Commissioned pieces are the
  crown jewels of the set.
- **Acknowledgement:** the README and About screen acknowledge the Noongar
  people and language; naming and cultural use are checked with Noongar
  language custodians before public release.

## 9. Threat Model (seed — to be expanded)

- **Assets:** app integrity, user data confidentiality, capability integrity,
  update-channel authenticity.
- **Adversaries:** malicious or compromised apps, compromised app authors
  (malicious signed updates), network attackers, hostile inputs to servers.
- **Trust boundaries:** kernel/userspace; app/broker; app/app; device/author.
- **Out of scope initially:** physical access, hardware side channels,
  compromised toolchain.

## 10. Roadmap

- **Phase 0 — Paper.** This document; expand the threat model; settle the
  ledger.
- **Phase 1 — Iron.** Rust kernel boots on QEMU aarch64 `virt`: scheduler,
  IPC fast path, capability tables, MMU, UART console. Then the same kernel
  on QEMU x86_64 `q35`, proving the HAL boundary.
- **Phase 2 — Voice.** Scheme registry, first userspace driver (virtio),
  minimal runtime; port first Redox drivers.
- **Phase 3 — Soul.** The pillars: broker, signed content-addressed app
  format, updater. First real app runs confined.
- **Phase 4 — Company.** Publish, write the show-don't-tell posts, open to
  contributors.

## 11. Rules for Claude Code Sessions

1. Read this file as binding project law; when it conflicts with a prompt,
   raise the conflict rather than silently obeying either.
2. For any subsystem, check the Borrow Ledger before writing code; if the
   verdict is unclear or seems wrong, ask the maintainer — do not decide.
3. `unsafe` Rust only in explicitly designated modules, each with a safety
   comment; flag every new `unsafe` block in the session summary.
4. Every non-trivial change ships with an explanation the maintainer can
   verify their understanding against.
5. Preserve licence headers on vendored MIT code; new files carry the GPLv3
   header.
6. British English in all documentation and comments.
