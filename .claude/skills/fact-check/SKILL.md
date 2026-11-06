---
# SPDX-License-Identifier: GPL-3.0-or-later
name: fact-check
description: "Verify what a document claims: citations, numbers, file paths and statements about the tree. Use on any RFC or research note before review, and on any claim about hardware, QEMU or another system."
---

If `$ARGUMENTS` names documents, check those. Otherwise check the current diff.

For each claim, record **verified**, **wrong**, or **could not check**. Only the first needs no
further word.

## What to check

- **Primary sources.** A claim about an architecture, an emulator or another system is checked
  against the manual or the source, not against a summary or memory. Name the document and section.
- **Quotations.** A quoted sentence appears, word for word, at the place cited.
- **Numbers.** Recompute what can be recomputed: bit fields, sizes, durations.
- **The tree.** Every path, command, flag and symbol exists and does what the sentence says.
- **Cross-references.** The cited section says what the sentence claims.
- **Status.** Nothing is called built, accepted or tested that is not.

Licences bind while checking: Linux, seL4 and QEMU may be read and cited, never copied
(`CLAUDE.md` § Licence).

Report what could not be checked plainly. An unverified claim stays marked as such, not smoothed
over.

## Lessons

- **2026-10-02 — a reference that resolves is not a reference that is right.** See
  `/check-coherence`'s lesson of that date.
