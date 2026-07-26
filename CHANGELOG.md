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

### Outstanding Phase 0 deliverables

The constitution's own roadmap §10 names two items before Phase 1 may begin. Neither is done:

- `docs/threat-model.md` — expansion of the §9 seed.
- Settling the Borrow Ledger. In particular the filesystem row: the content-addressed store is a
  "write ourselves" item sitting directly on top of a "port RedoxFS initially" item, and it is not yet
  decided whether the store is layered over an ordinary filesystem or *is* the filesystem. That choice
  probably forces the ledger's revisit earlier than "once the pillars run".

### Known amendments needed

- §7 states that `.devcontainer/` lives "in the repo beside this file". Since the constitution now
  lives here and the devcontainer serves the kernel repository, this clause needs a one-line
  correction. Recorded rather than silently fixed, because §4 and §11.1 reserve amendments to the
  maintainer.

[Unreleased]: https://github.com/setonix-os/docs/commits/main
