# Changelog

All notable changes to the Setonix documents are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Amendments to the constitution are always listed, however small. A change to project law that is not
recorded is indistinguishable from law that was never agreed.

## [Unreleased]

### Added

- `CLAUDE.md` — the founding document, placed under version control for the first time. This
  repository is now its single canonical home; the organisation superfolder holds only a pointer to it,
  so the two can never diverge.
- Repository scaffolding adopted from the maintainer's other projects: `CONTRIBUTING.md` with the RFC
  guidance, `STYLE.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue and pull-request templates,
  `.editorconfig`, markdownlint configuration.
- `.github/scripts/check-british-spelling.sh` and CI workflows enforcing constitution §11.6 and
  markdown lint on every pull request.

- `docs/rfcs/0001-content-addressed-store-and-the-filesystem.md` — **proposed**, awaiting the
  maintainer's verdict. Asks whether the content-addressed store is layered over an ordinary
  filesystem or is the filesystem, and argues for a split answer: fix the store's semantics now
  (authoritative, content-addressed, immutable, self-verifying, reached through `store://`), and defer
  the on-disk format behind that interface. The key finding is that the usual argument for baking the
  store into the filesystem — enforcement — does not transfer to Setonix, because there is no root to
  escalate to; the capability model already supplies a stronger version of that guarantee. What baking
  it in would genuinely add is verification at rest, and that is obtainable without committing to an
  on-disk format. Commit to the interface early, the format late.

### Changed

- **§7 amended.** The clause stating that `.devcontainer/` "lives in the repo beside this file" was
  made false by moving the constitution into this repository while the devcontainer serves the kernel
  repository. It now names the kernel repository explicitly, and records that the Rust version appears
  in three places with a CI script that fails the build if they disagree — so "pinned, never drifting"
  is checked rather than merely intended.

### Outstanding Phase 0 deliverables

The constitution's roadmap §10 names two items before Phase 1 may begin. One remains:

- `docs/threat-model.md` — expansion of the §9 seed.
- Settling the Borrow Ledger: RFC-0001 above proposes the two clarifications the filesystem and
  app-format rows need, but the verdict is the maintainer's and the ledger is unchanged until then.

[Unreleased]: https://github.com/setonix-os/docs/commits/main
