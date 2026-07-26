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

- `rfcs/0001-content-addressed-store-and-the-filesystem.md` — **accepted**. Asks whether the
  content-addressed store is layered over an ordinary filesystem or is the filesystem, and answers
  both: fix the store's semantics now (authoritative, content-addressed, immutable, self-verifying,
  reached through `store://`) and defer the on-disk substrate behind that interface. The finding that
  decides it is that the usual argument for baking the store into the filesystem — enforcement — does
  not transfer to Setonix, because there is no root to escalate to; pillar 2 already supplies a
  stronger version of that guarantee one layer higher. What baking it in would genuinely add is
  verification at rest, and that is obtainable without committing to an on-disk format. Commit to the
  interface early, the format late. Three open questions remain and become RFCs of their own.
- `rfcs/0002-documentation-scope-and-publication.md` — **accepted**. Settles which documents live here
  and which live beside the code, using an atomicity test: if it must change in the same commit as
  some code, it belongs with that code. Rejects a wiki outright, on governance rather than tooling
  grounds — wikis accept commits without pull requests, which would make project law changeable
  without review. Commits to publishing by rendering this repository in place (mdBook, at Phase 4)
  rather than feeding a mirror repository, because a mirror is the same divergence this project exists
  to prevent. Records that `CLAUDE.md`'s position at the repository root is load-bearing for session
  auto-loading, so the renderer must accommodate it and not the reverse.

### Changed

- **§4 Borrow Ledger amended**, per RFC-0001. The filesystem row loses "initially" and its
  revisit-on-a-schedule clause: RedoxFS now serves `file://` for mutable data *and* backs the store's
  substrate, to be revisited only if verification at rest or measurement demands it. The app-format row
  gains an explicit note that it owns the store's semantics and interface — which is what the pillars
  rest on — and not its on-disk substrate. A preamble sentence records that where a row cites an RFC,
  the RFC holds the reasoning and the row holds only the verdict.
- **§7 amended.** The clause stating that `.devcontainer/` "lives in the repo beside this file" was made
  false by moving the constitution here while the devcontainer serves the kernel repository; it now
  names the kernel repository explicitly.
    - §7 also claimed "CI runs the same image", which was untrue when written — CI installed a
      toolchain on a bare runner and took QEMU from the runner's own packages. The first draft of this
      amendment weakened the clause to describe that reality. **That was the wrong direction**, and the
      maintainer rejected it: the promise was right and the implementation was wrong, so the
      implementation changed. CI now runs inside the devcontainer image built from the same Dockerfile,
      and §7 states the guarantee more strongly than before, along with its consequence — that the image
      must be able to run *every* check a contributor is asked to run.
    - The general principle, worth stating once here because it will come up again: when a document and
      the world disagree, establish which one is wrong before editing either. Amending law to match a
      shortcut is how a constitution becomes decoration.
    - §7 gained a clause recording that the workshop now practises the pillars on itself: tools are
      fetched from their authors at pinned versions and verified by hash or signature, rather than
      taken from the host distribution. The occasion was concrete rather than theoretical. While
      setting the development environment up, Debian 13 turned out to ship QEMU 10.0.11 against
      upstream's 11.0.3 — a whole major version of the emulator this kernel is tested on, withheld by a
      packaging decision nobody on this project made. Pillar 3's gatekeeper problem appeared inside our
      own build before the kernel had finished printing its first line, which is a better argument for
      the pillar than anything that could have been written about it. Where a tool publishes no upstream
      binary at all, the distribution's build is used and labelled as a gatekeeper not yet removed.
- Layout flattened per RFC-0002: `docs/rfcs/` became `rfcs/`, and `threat-model.md` and `prior-art/`
  will sit at the repository root. The repository is already named `docs`; a nested `docs/` inside it
  was redundant and would have had to move again when the book's source root was chosen.
- The constitution's two opening epigraphs became one blockquote with two paragraphs, so that
  markdownlint's `MD028` passes without the rule being disabled. Reformatting project law is the
  cheaper of the two options: a loosened rule stops catching the same class of problem everywhere else,
  permanently, to avoid one edit here.

### Outstanding Phase 0 deliverables

The constitution's roadmap §10 named two items before Phase 1 may begin. One remains:

- `threat-model.md` — expansion of the §9 seed.
- ~~Settling the Borrow Ledger~~ — done, by RFC-0001 and the §4 amendment above.

[Unreleased]: https://github.com/setonix-os/docs/commits/main
