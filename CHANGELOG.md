# Changelog

All notable changes to the Setonix kernel are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Release codenames follow the six Noongar seasons — Birak, Bunuru, Djeran, Makuru, Djilba, Kambarang.

## [Unreleased]

### Added

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

- Rust pin moved from 1.88.0 to 1.95.0, matching the maintainer's other projects and the verified
  `dtolnay/rust-toolchain` action pin. Recorded here because a toolchain bump is a deliberate act
  under constitution §7, not a detail.
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
