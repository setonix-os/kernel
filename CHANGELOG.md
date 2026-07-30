# Changelog

All notable changes to the Setonix kernel are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Release codenames follow the six Noongar seasons — Birak, Bunuru, Djeran, Makuru, Djilba, Kambarang.

## [Unreleased]

### Added

- **Exception vectors, and a reporter that says what went wrong in one line** — the instrument the
  soft-float bug had to be diagnosed without. `boot.s` installs a 2 KiB-aligned sixteen-entry vector
  table into `VBAR_EL1` before the first Rust instruction runs, so even the earliest fault is reported
  rather than vectoring to address zero and dying mute. Every entry routes to a reporter that prints
  which vector fired, the exception class decoded into a sentence — EC 0x07 names `CPACR_EL1.FPEN`
  outright — and raw `ESR`/`ELR`/`FAR`/`SPSR`, then halts.
    - The common stub forces `SPSel` back to SP_ELx first, so even the never-used SP_EL0 group lands
      on a real stack instead of the uninitialised SP_EL0.
    - **Self-testing in CI**: a `provoke-exception` feature (never on by default) boots, greets, then
      executes `brk #0`; the boot job greps the console for the decoded "BRK instruction" report. The
      handler is proven on every pull request, not trusted.
    - `cargo xtask` grows `--features` passthrough for exactly that.
    - Terminal by design: until there is a scheduler, every exception is a report and a halt —
      recovery is policy, and there is nothing yet to recover to. When the timer interrupt arrives,
      the IRQ entries grow a real save/restore frame; the clobber-freely discipline in `vectors.s` is
      documented as ending on that day.
    - The EL1 assumption in `boot.s` is now load-bearing (`VBAR_EL1`, `SPSel`) and its header says so:
      an explicit CurrentEL check and EL2 descent are owed before real hardware or
      `virtualization=on`.
- **The kernel boots.** On QEMU aarch64 `virt` it reaches the PL011 UART and
  prints `Kaya!` and the resident critter (constitution §8). Nothing else: no
  scheduler, no IPC, no capability table, no MMU. What it establishes is the chain
  underneath all of those — linker script, boot stub, stack, `.bss` zeroing, the
  hardware-abstraction boundary and the console device.
    - `kernel/src/arch/mod.rs` — the hardware-abstraction boundary, deliberately
      two functions wide. It grows one function at a time, as callers above
      genuinely need them, because a HAL designed ahead of its callers ends up
      shaped like whichever architecture was written first.
    - `kernel/src/arch/aarch64/` — boot stub, PL011 console, core control. The
      boot stub parks every core but the first, installs the stack, zeroes `.bss`,
      and enters Rust.
    - `kernel/src/arch/x86_64/` — compiles and links against the same
      architecture-independent kernel, keeping the boundary honest, but does not
      boot: `q35` has no bare-ELF equivalent of `-kernel`.
    - `kernel/link/aarch64.ld` — load address 0x4000_0000, `.text.boot` first and
      `KEEP`-ed, 8-byte-aligned `.bss` bounds to match the zeroing loop's stride,
      64 KiB boot stack.
    - Panic handler that reports location and message on the console before
      halting the core, written as separate lines so that a fault partway through
      still leaves evidence on the wire.
- `xtask` — build, run and boot-test automation, with **no dependencies at all**.
  A tool that builds a system whose fourth pillar is exact, auditable dependency
  closures should not itself pull in eighty crates.
    - `cargo xtask boot-test` boots under QEMU, watches the serial console for an
      expected string against a deadline, and reaps the emulator afterwards — the
      kernel halts rather than exits, so an unreaped QEMU would wedge a CI runner.
    - Failure messages are advice rather than errno: a missing QEMU says how to
      install it, and `--arch x86_64` explains why that path cannot boot yet
      instead of failing obscurely.
- Repository skeleton and pinned development environment.
    - `.devcontainer/` with QEMU for both Tier-1 architectures, `gdb-multiarch`, OVMF and AAVMF UEFI
      firmware, `mtools`, `dosfstools`, and a pinned Node plus `markdownlint-cli2`. The linter is there
      because `CONTRIBUTING.md` asks contributors to run it: since CI runs this image, a tool missing
      from it is an instruction that cannot be followed inside the canonical environment.
    - `rust-toolchain.toml` pinning Rust and both bare-metal targets.
    - Workspace `Cargo.toml` carrying the lint policy, so the first crate to land is already bound by
      it rather than retro-fitted.
- Lint policy enforcing the constitution mechanically.
    - `unsafe_code = "deny"` at the workspace level, so a designated module must opt in visibly with
      `#![allow(unsafe_code)]` and the full set of such modules is greppable in one command.
    - `undocumented_unsafe_blocks` and `missing_safety_doc` denied, making the `// SAFETY:` convention
      a compiler-checked requirement rather than a review habit.
- CI for pull requests and `main`, running **inside the devcontainer image**.
    - Every job executes in the image built from `.devcontainer/Dockerfile` — the same one
      contributors get from "Reopen in Container". Nothing installs a toolchain of its own, so no tool
      version has a second source of truth and there is no "works on my machine" gap to argue about.
      This is constitution §7 taken literally.
    - The image is built from the Dockerfile *as it appears in the pull request*, so a change to the
      environment is tested by the run that proposes it. Layers come from the image `main` publishes,
      so an unchanged Dockerfile costs a pull rather than a build.
    - `main` publishes the image only after it has both built the kernel **and** booted it, so a pull
      request never caches from an image that was not itself proven.
    - Build and Clippy across both Tier-1 bare-metal targets on every pull request.
    - `boot` job runs the kernel under QEMU and requires the console greeting; `main` additionally
      boots the release image, which differs enough in layout and inlining that debug-only boot tests
      have let real faults through in other kernels.
    - Every action pinned to a commit SHA with a version comment.
- `.github/scripts/check-toolchain-pin.sh`, verifying that the Rust version named in
  `rust-toolchain.toml` and in `.devcontainer/Dockerfile` agree, and that both Tier-1 targets are still
  present. Its third check is an assertion of *absence*: no workflow may use `dtolnay/rust-toolchain`,
  `actions/setup-node` or similar, nor install a toolchain by hand. Since CI runs the devcontainer
  image, reintroducing any of those would silently recreate the second source of truth §7 exists to
  remove — silently, because everything would still build.
- `.github/scripts/check-british-spelling.sh`, enforcing constitution §11.6 in CI, with `vendor/`
  excluded so that vendored MIT code is never reworded.
- Custom Claude Code commands: `/audit-kernel`, `/check-style`, `/check-spelling` and
  `/check-primitive` — the last auditing a change against the primitive, the Borrow Ledger and the
  known graves.
- Contributor documentation: `CONTRIBUTING.md`, `STYLE.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue
  and pull-request templates.

### Changed

- Contributor documents no longer assume a permanent team of one. `SECURITY.md`, `CONTRIBUTING.md` and
  `CLAUDE.md` now say "deliberately small team" — the project anticipates at least one more member, and
  a document that promises "one maintainer" would start lying the day they join. The constitution's §5
  is deliberately untouched: its wording is role-based ("the maintainer") and still true, and the real
  §5 amendment — who reviews, who may merge — depends on the member's actual role, which is the
  maintainer's to write when that person exists.
- **The docs repository was merged into this repository, history and all.** The constitution now lives
  at `CONSTITUTION.md` in the repository root, bound into every session by the repo-root `CLAUDE.md`;
  RFCs and the documents' changelog live under `docs/`. The import was a true history merge rather than
  a file copy, so every signed commit of the paper trail is ancestry of `main` and remains publicly
  reachable after the old repository goes private. Rationale and consequences are recorded where they
  belong: `docs/CHANGELOG.md` and RFC-0002's second amendment.
- **Both Tier-1 targets are now soft-float**, `aarch64-unknown-none` becoming
  `aarch64-unknown-none-softfloat`. Found by booting: the kernel reached
  `console::write_str` and then took a synchronous exception to `0x200` — the
  `VBAR_EL1 = 0` vector — because LLVM had emitted an FP/SIMD instruction for
  ordinary data movement inside `read_volatile`'s debug precondition check, and
  `CPACR_EL1.FPEN` is 0 at reset. No console output at all, since the fault landed
  one instruction before the first character.
    - The plain `aarch64-unknown-none` target permits NEON; its `-softfloat`
      variant does not. `x86_64-unknown-none` was already soft-float, so the two
      Tier-1 architectures had been quietly asymmetric — precisely the class of
      difference building both from day one is meant to expose.
    - Right on the merits regardless of the bug: a microkernel whose IPC fast path
      is the product must not touch registers it would then have to save and
      restore on every context switch. FP/SIMD belongs to userspace, enabled per
      process and saved lazily, once there is a userspace to enable it for.
    - Nothing in the kernel source changed. Only the target triple did.
- **The devcontainer no longer builds itself out of distribution packages.** Debian 13 ships QEMU
  10.0.11; upstream is at 11.0.3. A whole major version of the emulator this kernel is tested on,
  withheld by a packaging decision nobody on this project made — pillar 3's gatekeeper problem turning
  up inside our own build, before the kernel had finished printing its first line. Rust, Node and QEMU
  now come from their authors at pinned versions, each verified before use:
    - QEMU is built from the upstream source tarball, checked against a SHA-256 pinned in the
      Dockerfile *and* against the project's own detached GPG signature. Author signing is pillar 3 as
      QEMU already practises it, so it costs nothing to honour. Only the two Tier-1 softmmu targets are
      built.
    - Rust comes from `rustup-init` in `static.rust-lang.org`'s versioned archive, verified against the
      checksum published beside it, installed into `/opt/rust` so the toolchain belongs to the image
      rather than to a user.
    - Node comes from `nodejs.org`, verified against the release's own `SHASUMS256.txt`.
    - Multi-stage, so the download and build tooling appears in no final layer, and the base image is
      pinned by digest rather than by the moving `debian:13-slim` tag.
    - apt remains for shared libraries and for tools that publish no upstream binary at all — GDB, the
      UEFI firmware images, `mtools`, `dosfstools`. Each is labelled in the Dockerfile as a gatekeeper
      not yet removed, with the reason it has not been.
    - The image ends with a verification block that invokes every tool, so a missing shared library or
      a botched `COPY` fails the build rather than someone's afternoon.
- Base image moved from Debian 12 (bookworm) to **Debian 13 (trixie)**, which also brings gdb 16.3 in
  place of 13.x.
- The container now runs as root with the workspace at `/root/kernel`, matching the maintainer's other
  embedded projects. The toolchain in `/opt` belongs to the image, which keeps `CARGO_HOME` writable
  without chown games when CI runs the same image.
- Rust pin moved 1.88.0 → 1.95.0 → **1.97.1**, the current stable. Recorded because a toolchain bump is
  a deliberate act under constitution §7, not a detail. Verified: `fmt` and `clippy` clean on both
  Tier-1 targets and on `xtask` under `-D warnings` with the new compiler, no source changes needed.
- Every GitHub Action pin refreshed to its latest release: `actions/checkout` v6.0.3 → v7.0.1,
  `actions/setup-node` → v7.0.0, `actions/cache` → v6.1.0, all still full-SHA pinned.
- CI no longer guards its compile steps on the workspace having members, and the boot job no longer
  guards on `xtask` existing — both now do.
- `clippy::redundant_pub_crate` and, in one documented instance,
  `clippy::missing_const_for_fn` are allowed. The first suggests plain `pub` for
  every `pub(crate)` item in a private module, which in a binary crate is not
  merely noisy but wrong. The second would have made the console function `const`
  on x86_64 only, where it is a no-op — a divergence in the hardware-abstraction
  boundary's signature, which is precisely what the boundary exists to prevent.
- `clippy.toml` adds architecture, firmware and peripheral names to
  `doc-valid-idents`, so that `doc_markdown` stops demanding backticks around
  proper nouns like AArch64 and OVMF.

[Unreleased]: https://github.com/setonix-os/kernel/commits/main
