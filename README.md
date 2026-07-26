<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix — documentation

> *Setonix does to operating systems what Rust did to systems languages:*
> *keep the proven, prune the legacy.*

This repository is the project's paper trail. Design disagreements are settled
here, where being wrong is cheap.

## Contents

- **[CLAUDE.md](CLAUDE.md)** — the founding document and project constitution.
  Binding on every contributor, human or AI. Start here.
- [rfcs/](rfcs/) — one document per design decision, with its rationale
  and the alternatives rejected.
    - [RFC-0001](rfcs/0001-content-addressed-store-and-the-filesystem.md) —
      the content-addressed store and the filesystem. **Accepted.**
    - [RFC-0002](rfcs/0002-documentation-scope-and-publication.md) —
      documentation scope and publication. **Accepted.**
- `threat-model.md` — expansion of the constitution's seed threat model. *(to come)*
- `prior-art/` — archived earlier attempts, kept as provenance. *(to come)*

## Status

Phase 0 — Paper, closing. The constitution is written and under version control,
and the Borrow Ledger is settled: RFC-0001 fixed the content-addressed store's
semantics while deferring its on-disk substrate, and the ledger's filesystem and
app-format rows now say so.

**The threat model is the last outstanding Phase 0 deliverable.**

Phase 1 has begun in the [kernel repository](https://github.com/setonix-os/kernel):
the kernel boots on QEMU aarch64 `virt`.

## Acknowledgement

Setonix borrows words from the Noongar language with acknowledgement, and
commissions rather than imitates visual language. We acknowledge the Noongar
people as the traditional custodians of the country whose language and seasons
give this project its names, and pay our respects to their elders past and
present.

## Licence

GPLv3 — see [LICENSE](LICENSE).
