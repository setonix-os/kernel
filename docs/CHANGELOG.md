# Changelog

All notable changes to the Setonix documents — the constitution, the RFCs, the threat model — are
recorded here. The code's changelog is [../CHANGELOG.md](../CHANGELOG.md).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Amendments to the constitution are always listed, however small. A change to project law that is not
recorded is indistinguishable from law that was never agreed.

## [Unreleased]

### Added

- `docs/research/0001-capabilities-and-ipc-prior-art.md` — a literature and source review that
  stress-tests RFC-0003 and RFC-0004 against the systems that already fought these fights (seL4,
  Zircon/Fuchsia, KeyKOS/EROS, Barrelfish, CHERI, Genode, QNX, L4, and the Rust-OS field: Tock,
  Hubris, RedLeaf, Asterinas, Theseus, Redox). The constitution's "keep the proven, prune the legacy"
  turned into an actual audit. Verdict: both RFCs' spines are validated and correctly cited, with three
  bounded corrections — one legacy mechanism to **prune** (RFC-0004's page-mapping large transfer is
  L4's abandoned "long IPC"), proven refinements to **adopt** (first-class reply objects, virtual
  message registers, priority-aware direct switch, seL4 badges by name), and one design question the
  research **forces to a decision rather than a deferral** (RFC-0003's revocation knot). Every claim
  carries a primary source.

- `docs/rfcs/0003-capability-table.md` — **accepted** (2026-07-30; selective revocation's final
  verdict expressly deferred to RFC-0003a), the first Phase 1 design RFC and the first to
  cite the threat model's obligations by number, discharging O-1, O-2 and O-4 and part of O-3. Argues
  for a flat per-process handle table (Zircon lineage) carrying seL4's invariants — handles as
  indices, kernel-owned capabilities, monotone subset-only derivation, generation counters against
  reuse — while explicitly steering around the "baroque capability hierarchies" grave that seL4's
  CNode radix would risk. Capability transfer is a Rust *move* (no `Clone`), cashing out §3's claim
  that ownership models transfer at compile time. Constrains the future syscall ABI to
  capability-indexed operations only, so O-4 is inherited as a birth constraint rather than
  retrofitted. States honestly that selective transitive revocation is proposed (badges + shallow
  derivation records) but not settled, deferring it to RFC-0003a with the broker's needs as input —
  naming that gap on paper being precisely what §5.5 is for.

- **The last of the retired docs repository, recovered by an exhaustive re-audit** (file-by-file and
  git-history, not just prose shingles). The provenance check confirmed all eleven docs commits are
  ancestry of `main` bar the retirement-banner commit, which correctly stays only in the retired repo;
  the content check found three genuine omissions, now closed:
    - The pull-request template gains an **"If this changes a document or the constitution"** section —
      the constitution-amendment guidance the docs repository's own template carried and the code-first
      kernel template lacked: state what an amendment supersedes and invalidates, leave no stale clause,
      renumber no cited section. Its `CLAUDE.md` section references, stale since the rename, are
      corrected to `CONSTITUTION.md` in the same pass.
    - `.gitignore` ignores `book/` and `_site/`, the rendered-documentation output RFC-0002 commits to
      at Phase 4, so a first local mdBook render cannot be committed by accident.
    - `.gitattributes` marks `*.pdf` binary.
  Everything else the audit surfaced was reworded-in-place, superseded by a newer kernel version, or
  docs-repo-specific framing that correctly does not belong. The retired repository now holds nothing of
  substance absent here.

- **`docs/threat-model.md` — the last Phase 0 deliverable.** Expands the constitution's §9 seed into
  the authoritative statement of what Setonix defends: five assets, five adversaries, six trust
  boundaries, and twenty numbered obligations (`O-1` … `O-20`) the design must discharge. The
  obligations are the contract between the model and the code — RFCs cite them by number ("this
  mechanism discharges O-3 at B1"), and each carries an honest status: **Built** (three, today —
  kernel memory safety, the pinned build, signed commits), **Designed** (most), or **Deferred** (with
  a stated precondition, e.g. DMA confinement needs an IOMMU). §8 states which obligations bind at
  which roadmap phase, so the gap between designed and built is never hidden. Writing it now, mostly
  unbuilt, is §5.5: the capability, IPC and app-format RFCs are about to be argued, and this is the
  paper they are argued against.
- Two pieces of guidance recovered from the retired docs repository during a completeness sweep, both
  describing practices already followed but written down nowhere in this repository: `CONTRIBUTING.md`
  gains **Amending the constitution** (leave no stale clause, check cross-references, never renumber a
  cited section — the discipline `/check-coherence` automates), and `STYLE.md` gains a **Tables** rule
  (preferred for parallel attributes; exempt from the line-length limit).
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

- **RFC-0003 amended** (prior-art review, 2026-07-30): its spine holds, but the revocation deferral
  (§7-B3) is upgraded to a decision RFC-0003a must make — §6's broker-bypassing `TRANSFER` makes
  broker-mediated revocation unreachable, and "badges + one-level link" cannot express multi-level
  transitive revocation; per-client eviction is named a day-one gap (O-3 half-discharged); §6's
  compile-time claim is scoped to the kernel's internal representation (the cross-process transfer is
  runtime, as RedLeaf's own group concluded ownership types cannot span protection domains); and the
  generation scheme's load-bearing invariants are stated. Full reasoning in the research note.
    - **Flagged for the maintainer, not taken — Constitution §3.** Its slogan "Rust's ownership and
      move semantics model capability transfer at compile time" overreaches: the borrow checker sees
      one compilation unit, so it models the kernel's *internal* capability handling, not the
      userspace-observable cross-process transfer (a runtime table operation). Recommended reword:
      *"…model the kernel's own capability handling at compile time — no accidental duplication or
      use-after-move; cross-process transfer is a runtime table operation the generation scheme
      secures."* §3 is constitution text, so this is the maintainer's to take or leave.
- **Constitution touched, twice, on the maintainer's authorisation, when the threat model landed.**
  Both are factual or pointer updates rather than changes to any clause, and both are logged because
  constitutional amendments always are, however small.
    - The **Status line** no longer calls the threat model "the last outstanding Phase 0 deliverable" —
      false the moment it merged — and now reads that Phase 0 is complete: the constitution, the settled
      Borrow Ledger and the threat model are all written, with the kernel booting, greeting and
      reporting its own faults.
    - **§9's header** was "(seed — to be expanded)"; it now reads "(seed; expanded in
      `docs/threat-model.md`)". §9's body is unchanged and remains the seed and the one-screen summary
      the full document expands — the pointer just stops the header promising an expansion that now
      exists.
- **The documents moved into the kernel repository, and the docs repository is retired** (2026-07-26,
  maintainer's decision; RFC-0002 amended to record it). The two-repository split was double overhead
  for a single maintainer — every cross-reference was a URL, every convention existed twice — and the
  project is young enough that nothing external breaks. The docs repository's full history was
  **merged, not copied**: all of its signed commits are ancestry of this repository now, so the paper
  trail keeps its provenance and its signatures in public. The constitution lives at
  [`CONSTITUTION.md`](../CONSTITUTION.md) in the repository root, bound into every session by the
  repo-root `CLAUDE.md`; RFCs stay under `docs/rfcs/`; this file continues as the documents'
  changelog. A dedicated GitHub Pages repository can still be added later if fancied — RFC-0002's
  Decision 4 anticipated exactly that, and it survives the consolidation unchanged.
- **§5.3 amended: authorship of the kernel core may be delegated; understanding and merge authority
  may not.** The clause previously read "Hand-write the learning core", reserving kernel hot paths to
  the maintainer. It now reads "Review the learning core line by line". The maintainer's decision,
  and a narrower change than it first appears:
    - §5.1 and §5.2 are untouched in substance and are now the load-bearing safeguards. The maintainer
      still directs, decides, and **alone merges**; nothing lands that they cannot explain. §5.1 gains
      the words "and alone merges" to say so explicitly rather than by implication.
    - §4's **Author** column flips to "AI-first, maintainer reviews line by line" for the rows that
      previously read human-first. The **Verdict** column is untouched: "write ourselves" is a claim
      about where a design comes from, not about whose hands are on the keyboard, and the ledger's
      preamble now says so, because conflating the two was the ambiguity that made the amendment
      necessary.
    - The amendment adds a pacing constraint rather than only removing a restriction: work arrives in
      increments small enough to review honestly, one subsystem at a time, design settled on paper
      first. **A change too large to review is not delegation, it is abdication** — and the reviewer,
      not the author, decides what is too large. Without that clause the amendment would preserve the
      letter of §5.2 while destroying its point, since an unreviewable diff is un-understood by
      construction.
    - Deliberately amended *in place* rather than by inserting a new principle: §5.4 and §5.5 are
      cross-referenced from a dozen files across both repositories, and renumbering would have broken
      every one of them silently.
- **Status line corrected.** The opening said "Pre-code. Design phase. This document precedes the first
  commit." It no longer does — the kernel boots on QEMU aarch64 `virt` and CI proves it on every push.
  A document that misstates the state of the thing it governs is the first place a reader loses trust.
- **The former name pruned from the opening.** The parenthetical recording that the project began as
  BlueOS and was renamed in July 2026 has been removed. It told a reader nothing about what Setonix is
  or why, which is the only job that opening paragraph has, and §1's coherence rule applies to the
  document's own prose as much as to its subsystems. Nothing is lost — the sentence remains in this
  repository's git history, which is where provenance belongs. Provenance is part of the record, not
  part of the statement of what the project is.
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
- Layout flattened per RFC-0002: `docs/rfcs/` became `rfcs/`, and `threat-model.md` will sit at the
  repository root.
- **`prior-art/` dropped** before it was ever built, and RFC-0002 amended to say so. The plan had been
  to archive the earlier attempts here as provenance. They were dead ends, were never public
  repositories, and Setonix does not draw on them — so keeping their archives would be preserving a
  lineage the project has no use for, which is exactly what §1 and §5.4 argue against. The Borrow
  Ledger already records lineage for everything Setonix genuinely borrows from, which is the provenance
  that matters. The promise is removed from `README.md` and `CONTRIBUTING.md` rather than left standing
  as a directory that never appears: a document that promises something it never delivers teaches its
  readers to discount the rest of it. The repository is already named `docs`; a nested `docs/` inside it
  was redundant and would have had to move again when the book's source root was chosen.
- The constitution's two opening epigraphs became one blockquote with two paragraphs, so that
  markdownlint's `MD028` passes without the rule being disabled. Reformatting project law is the
  cheaper of the two options: a loosened rule stops catching the same class of problem everywhere else,
  permanently, to avoid one edit here.

### Outstanding Phase 0 deliverables

The constitution's roadmap §10 named two items before Phase 1 may begin. One remains:

- `threat-model.md` — expansion of the §9 seed.
- ~~Settling the Borrow Ledger~~ — done, by RFC-0001 and the §4 amendment above.

[Unreleased]: https://github.com/setonix-os/kernel/commits/main
