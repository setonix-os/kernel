<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix — documentation

> *Setonix does to operating systems what Rust did to systems languages:*
> *keep the proven, prune the legacy.*

This repository is the project's paper trail. Design disagreements are settled
here, where being wrong is cheap.

## Contents

- **[CLAUDE.md](CLAUDE.md)** — the founding document and project constitution.
  Binding on every contributor, human or AI. Start here.
- [docs/rfcs/](docs/rfcs/) — one document per design decision, with its rationale
  and the alternatives rejected.
    - [RFC-0001](docs/rfcs/0001-content-addressed-store-and-the-filesystem.md) —
      the content-addressed store and the filesystem. **Proposed.**
- `docs/threat-model.md` — expansion of the constitution's seed threat model. *(to come)*
- `docs/prior-art/` — archived earlier attempts, kept as provenance. *(to come)*

## Status

Phase 0 — Paper, closing. The constitution is written and under version control.
The threat model is the last outstanding Phase 0 deliverable; RFC-0001 proposes
how to settle the Borrow Ledger's filesystem row, and awaits a verdict.

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
