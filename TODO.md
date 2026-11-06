<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# TODO

Where the work stands, and work with no other home. A finished item is deleted, not struck through:
git log is the record, and its outcome goes in a changelog. RFC increment lists and open questions stay
the single source of truth for their own work.

## Resume here

*2026-11-06.* `/handoff` overwrites this block.

- **Stands:** `main` holds PRs #32 to #34: the document fixes, the boot grant rules and the design
  risks. Phase 1: the kernel boots on QEMU aarch64 `virt`; the capability crate is built, with no
  consumers. RFC-0003a, 0005, 0006 and 0007 are proposed; RFC-0003b is a draft on
  `claude/rfc-0003b-reclaim`.
- **In flight, merge in this order:** `claude/skills-and-process`, the pull request carrying this
  file, then `claude/constitution-c1-language`, not yet opened.
- **Next:** the polish items below, one small PR each.
- **Waits on the maintainer:** the verdicts under § Decisions, and the merges above.

## Order of work

Agreed with the maintainer, 2026-11-06.

1. Accept the demo parts of RFC-0005, RFC-0006 and RFC-0007 in tranches, and let real code correct
   the paper. Then the EL0 "Kaya!" demo (RFC-0007 §18). *The maintainer decides each tranche.*
2. Build the executable model of RFC-0003b's §12 as the real Pool crate, RFC-0005's increments 7
   and 8, so that the model and the kernel work are one thing. It starts with a short plan for the
   maintainer's approval. RFC-0003b is reviewed again only after it exists.
3. Boot x86_64 on `q35` early, to prove the HAL before the syscall ABI freezes. A PVH note looks
   cheapest; the x86_64 boot RFC decides.

## Polish

Approved 2026-11-06; one reviewable PR each. Sources are in `handoff/process-gap.txt` on
`claude/handoff-2026-11-06`, §(a).

- Internal link and anchor checks on every PR, and a weekly external-link check (lychee, in the
  image; extend the pin check's forbidden actions).
- `cargo doc` with `-D warnings`, by package.
- A weekly workflow that opens one tracking issue per pinned tool Dependabot cannot see: Rust, Node,
  QEMU, markdownlint-cli2.
- Coverage of `capability` inside the image, with no third-party upload.
- Pin Claude Code in `.devcontainer/Dockerfile`, version and verification, closing the gap with O-19.
- A Dependabot docker ecosystem for the digest-pinned `FROM` lines; the extension lists; one PR
  checklist; document the rulesets in `CONTRIBUTING.md`.
- A GHCR retention policy: each merge keeps an image of about 800 MB alive under `:sha`.

## Decisions

The maintainer's. The open questions of RFC-0003a §14, RFC-0003b §13, RFC-0005 §17, RFC-0006 §18 and
RFC-0007 §17 hold the design ones. These have no other home:

- Dependabot cadence; Codecov; a merge queue; Copilot's standing; security test obligations; when
  versioning starts.
- A bot identity for Claude's PRs, since one opened under the maintainer's account cannot take his
  approval.

## Before real hardware

- `boot.s` owes an explicit `CurrentEL` check and EL2 descent.
- Record the deferred release principles in a release RFC: draft first, reviewed publish, signed tag
  on `main`, re-verified checksums, provenance.
- When RFC-0007's syscalls land, an error catalogue checked against the error enum.
