<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0002 — Documentation scope and publication

| Field | Value |
|-------|-------|
| Status | **Accepted** — 2026-07-26, by the maintainer |
| Author | Drafted by Claude Code; verdict the maintainer's |
| Date | 2026-07-26 |
| Affects | Repository layout; constitution §7, §10 (phase 4) |

## The question

Which documents live in this repository, which live beside the code, and what does
this repository become when the project is published?

## Decision 1 — the split is decided by atomicity

**Documentation that describes code lives with the code.** Rustdoc, build commands,
the `unsafe` policy, style conventions, the security policy and the changelog all
belong in the repository they describe, because a change in behaviour must be able
to update them **in the same commit**. Separate them and a sync step exists; a sync
step that a human must remember is a sync step that will be forgotten, and the
documentation then describes a system that no longer exists.

**This repository holds what is true about the project**, not about a particular
crate: the constitution, the threat model, decision records, lineage, prior art.
These change on their own cadence and are not invalidated by a refactor.

The test, when it is unclear: *if this must change in the same commit as some code,
it belongs with that code.*

This is the single-source-of-truth rule from `STYLE.md` with a locality corollary
attached. Both exist for the same reason as pillar 4 — divergent copies of a truth
are worse than an inconvenient single one.

## Decision 2 — no wiki, ever

GitHub wikis accept commits without pull requests. That contradicts §5.2 and §11.4
directly: nothing merges un-understood, and every non-trivial change ships an
explanation to review. A wiki would be a place where project law could change
without review — which is the precise failure a written constitution exists to
prevent. Rejected on governance grounds, not on tooling preference.

## Decision 3 — publish by rendering in place, never by mirroring

When Phase 4 arrives, this repository gains a static site generator and publishes
itself. It does **not** feed a separate website repository that holds a copy of the
prose.

A mirror repository is a divergence machine. Two copies of the constitution, one
rendered and one canonical, is the same failure mode as two copies of a dependency
closure — and this project's entire thesis is that such copies must be made
structurally impossible rather than merely discouraged. Rendering in place also
means a change that breaks the documents breaks CI, which a mirror cannot offer.

**mdBook** is the intended generator: this is a Rust project, the sources are
already Markdown, it is a single binary with no theme ecosystem to chase, and it
renders a numbered `rfcs/` directory as a chapter list without configuration. Zola
or Astro if real design control is ever wanted; that decision is deferred and
cheap, because the input is plain Markdown either way.

## Decision 4 — a landing page, if ever wanted, is additive

`setonix-os.github.io` is a distinct GitHub repository that serves at the
organisation root. It does not conflict with this repository publishing at
`setonix-os.github.io/docs`, so a marketing or landing site can be added later
without restructuring anything.

Deliberately not now. The constitution published verbatim is a stronger artefact
than anything written *about* the constitution, and §10's Phase 4 is
"show-don't-tell" for exactly that reason.

## Decision 5 — `CLAUDE.md` stays at the repository root

Its position is load-bearing for a reason that has nothing to do with
documentation. Claude Code loads `CLAUDE.md` from the working directory and walks
**upward**, never sideways into sibling directories. The organisation superfolder
holds a pointer file naming `docs/CLAUDE.md`; a session working in the `kernel`
repository finds that pointer by walking up, and follows it.

Most site generators expect their chapters under a configured source directory. The
book layout must therefore accommodate the constitution staying at the root — by
configuring the source directory, not by moving project law to suit a renderer.
Getting this backwards would silently unbind every future session in the
organisation from the constitution, which is a far worse outcome than an awkward
`book.toml`.

## Consequences

- `docs/rfcs/` flattened to `rfcs/`. The repository is already named `docs`; a
  nested `docs/` inside it was a reflex, and it would have had to move again when
  the book's source root was chosen. Flattened now, while there are two commits and
  no external links to break. `threat-model.md` likewise sits at the root.
- No change to the kernel repository, which already holds its own documentation.
- Phase 4 gains a concrete task: add `book.toml`, a `SUMMARY.md`, and a Pages
  workflow. Nothing before Phase 4 depends on it.

## Rejected alternatives

- **All documentation in one repository, including per-crate docs.** Rejected: it
  breaks atomicity, which is the whole basis of Decision 1.
- **Per-repository wiki for contributor guides.** Rejected under Decision 2.
- **A separate `setonix-os/website` repository that vendors the prose.** Rejected
  under Decision 3 — it is the divergence this project is built to prevent.
- **Moving `CLAUDE.md` under a `src/` directory to suit mdBook's defaults.**
  Rejected under Decision 5; the renderer accommodates the constitution, not the
  reverse.

## Amendments

- **2026-07-26 — `prior-art/` dropped.** As accepted, this RFC listed a
  `prior-art/` directory holding archives of earlier attempts. It is removed
  rather than left unbuilt. The earlier attempts were dead ends, were never public
  repositories, and carrying their archives here would preserve provenance for a
  lineage the project does not draw on — §1's coherence rule cuts against it, and
  the Borrow Ledger already records lineage for everything Setonix *does* borrow
  from. Recorded as an amendment rather than edited away silently, because an
  accepted RFC is a record of a decision and quietly changing one is how a
  decision log stops being trustworthy.

- **2026-07-26 — the two-repository split is reversed; the documents move into
  the kernel repository.** The maintainer's decision, taken while the project is
  fresh enough that nothing external breaks. Two repositories were double
  overhead for one maintainer: every cross-reference was a URL, every convention
  was maintained twice — and the atomicity test of Decision 1, the genuinely
  load-bearing idea, never required separate *repositories*, only separate
  ownership of concerns. It survives intact as a placement principle within one
  repository: documentation that describes code changes with the code, while
  the constitution, RFCs and threat model live at the root and under `docs/`,
  on their own cadence.

  What else survives, and how:

    - **Decision 2 (no wiki)** — unchanged.
    - **Decision 3 (render in place, never mirror)** — unchanged in principle;
      the thing rendered is now this repository's document set. A dedicated
      GitHub Pages repository remains possible later if the project fancies
      one — that was always additive, per Decision 4.
    - **Decision 5 (auto-load position is load-bearing)** — the mechanism
      changed shape but not intent: the constitution is `CONSTITUTION.md` at
      the repository root, and the repo-root `CLAUDE.md`, which Claude Code
      auto-loads, binds every session to it in its opening lines.

  The docs repository is retired to private, not deleted. Its history was merged
  into this repository as ancestry first, so every signed commit of the paper
  trail remains publicly reachable; the retired repository is a frozen
  duplicate, not the canonical record.
