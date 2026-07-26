# Changelog

All notable changes to the Setonix kernel are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Release codenames follow the six Noongar seasons — Birak, Bunuru, Djeran, Makuru, Djilba, Kambarang.

## [Unreleased]

### Added

- Repository skeleton and pinned development environment.
    - `.devcontainer/` with QEMU for both Tier-1 architectures, `gdb-multiarch`, OVMF and AAVMF UEFI
      firmware, `mtools` and `dosfstools`.
    - `rust-toolchain.toml` pinning Rust and both bare-metal targets.
    - Workspace `Cargo.toml` carrying the lint policy, so the first crate to land is already bound by
      it rather than retro-fitted.
- Lint policy enforcing the constitution mechanically.
    - `unsafe_code = "deny"` at the workspace level, so a designated module must opt in visibly with
      `#![allow(unsafe_code)]` and the full set of such modules is greppable in one command.
    - `undocumented_unsafe_blocks` and `missing_safety_doc` denied, making the `// SAFETY:` convention
      a compiler-checked requirement rather than a review habit.
- CI for pull requests and `main`.
    - Build and Clippy across both Tier-1 bare-metal targets.
    - `boot` job that runs the kernel under QEMU and expects the console greeting. Activates
      automatically once an `xtask` harness exists.
    - Every action pinned to a commit SHA with a version comment.
- `.github/scripts/check-toolchain-pin.sh`, verifying that the Rust version named in
  `rust-toolchain.toml`, `.devcontainer/Dockerfile` and the CI action pins all agree, and that both
  Tier-1 targets are still present. Closes the drift gap the constitution §7 promises against but that
  three separate files would otherwise allow.
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

[Unreleased]: https://github.com/setonix-os/kernel/commits/main
