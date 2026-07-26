<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0001 — The content-addressed store and the filesystem

| Field | Value |
|-------|-------|
| Status | **Accepted** — 2026-07-26, by the maintainer |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-07-26 |
| Affects | Constitution §2 (pillars 1, 4), §4 (Borrow Ledger: filesystem, app format), §10 (phases 2–3) |

> **Accepted.** The Borrow Ledger's filesystem and app-format rows have been amended
> to match the "Consequences" section below. The three open questions at the end of
> this document remain open and become RFCs of their own.

## The question

Is the content-addressed store **layered over** an ordinary filesystem, or **is** the filesystem?

The constitution leaves this open, and the two rows of the Borrow Ledger that meet here disagree in
spirit. "Filesystem — port RedoxFS initially, AI-first" is a cheap, delegable row. "App format,
signing, content-addressed store — write ourselves, human-first" is the opposite. The store sits
directly on top of the filesystem, so whichever way this goes, one of those two rows changes.

## The maintainer's instinct

> Perhaps it is baked into the filesystem, for more safety?

The instinct is sound and the rest of this document mostly agrees with it. But the reason it is sound
is not the reason it first appears to be, and that distinction decides how much work it costs.

## What "safety" actually rests on here

The apparent argument for baking the store into the filesystem is enforcement: Nix's store is
protected by being a root-owned, read-only directory, and a root compromise defeats it. Make
content-addressing the native storage primitive and mutation becomes structurally impossible rather
than merely forbidden.

**That argument does not transfer to Setonix, because Setonix has no root.**

Pillar 2 and §3 are unambiguous: capabilities are the only authority, and there is no ambient
authority anywhere. The store is a scheme service. It holds the only capability that carries write
rights to its backing storage. Nothing else in the system can write there — not because a permission
bit says so, but because no other process holds a capability naming that storage, and capabilities are
unforgeable and reducible in rights but never widenable. There is no privileged account to escalate
to. The confused-deputy path that breaks Nix's model is closed by the capability model, one layer
above the filesystem, before the filesystem is consulted at all.

So the enforcement argument for a bespoke on-disk format buys very little. Setonix already has a
stronger version of it, for free, from a pillar it was going to implement anyway.

## What baking it in *would* genuinely buy

Two properties that capabilities do not supply:

1. **Verification at rest.** A content-addressed store detects bit rot and offline tampering, because
   a block's name is its hash and reading it checks it. Capabilities protect against a live process;
   they say nothing about a disk that silently flipped a bit, or an attacker with the drive in hand.
2. **Native deduplication**, and one mechanism serving both pillar 1 (immutable binaries) and pillar 4
   (exact dependency closures) instead of two mechanisms that must be kept in agreement.

Property 1 is the real prize. But note that it is obtainable **without** a bespoke on-disk format: the
store service can hash on read and refuse to serve a blob whose content does not match its name. The
property comes from the store being authoritative and verifying, not from the blocks living in a
special filesystem.

## What baking it in would cost

- **It deletes the cheapest row in the ledger.** "Port RedoxFS initially" becomes "write a
  content-addressed filesystem ourselves", which is human-first work. In a one-maintainer project the
  maintainer's own hours are the scarcest resource in the system, and §5.3 spends them deliberately on
  the learning core — scheduler, IPC, capabilities, MMU. A storage layer is not on that list.
- **It does not remove the need for a conventional filesystem.** Pillar 1 is "immutable binaries,
  *mutable* data". Config, cache and state must be read-write. A content-addressed store cannot serve
  them. So the mutable filesystem is required either way, and baking in adds a second storage
  implementation rather than replacing one.
- **Garbage collection is where the difficulty actually lives.** Reclaiming blobs no longer reachable
  from any installed app closure is the hard part of Nix, and it is hard independently of the substrate.
  Nix does it with the luxury of a normal filesystem underneath. Doing it while also owning the on-disk
  format means debugging both at once, with no known-good layer to bisect against.
- **On-disk formats are the least reversible decision in an operating system.** Committing to one
  before the app format, the signing scheme and the broker are designed inverts §5.5: it settles in
  code the thing that should still be cheap to be wrong about on paper.

## Proposal

Split the question in two, and answer them separately, because they have different costs and different
deadlines.

**Semantics — decide now, and decide in the maintainer's direction.** The store is authoritative,
content-addressed, immutable and self-verifying. It is a scheme (`store://`) served by a userspace
service holding the sole write capability to its backing storage. Blobs are named by hash; reads verify;
there is no interface through which a blob can be modified in place, only added and garbage-collected.
Immutability is a property of the *store*, not a convention observed by its callers. This is pillar 1
and pillar 4 expressed as one mechanism, which is what coherence with the primitive requires.

**Substrate — defer, behind that interface.** Implement the store over ported RedoxFS first. The store
service owns a subtree; nothing else holds a capability to it. Later, if measurement or the threat model
justifies it, replace the substrate with a native content-addressed filesystem without changing a single
consumer, because consumers only ever spoke `store://`.

**The mutable half stays a conventional filesystem** — ported RedoxFS, per the ledger's existing verdict,
serving `file://` for config, cache and state.

The rule this follows: **commit to the interface early, commit to the format late.** It is the same
discipline §5.5 applies to documents, applied to storage.

## Consequences for the Borrow Ledger

If accepted, §4 needs two clarifications rather than a change of verdict:

| Row | Now reads | Would read |
|---|---|---|
| Filesystem | Port code initially — RedoxFS; revisit once pillars run | Port code — RedoxFS serves `file://` (mutable data) **and** backs the store's substrate. Revisit the substrate only if verification-at-rest or performance demands it, not on a schedule. |
| App format, signing, content-addressed store | Write ourselves | Unchanged — but note explicitly that this row owns the **store's semantics and interface**, and that those are what the pillars depend on. The substrate is not part of this row. |

That is the outcome I would argue for: the maintainer's instinct wins on semantics, the ledger's cheap
verdict survives on substrate, and the irreversible decision stays deferred.

## Rejected alternatives

- **Store as a plain directory convention over an ordinary filesystem (the Nix model).** Rejected: it
  makes immutability a convention its callers must respect, which contradicts pillar 1 being a property
  of the system rather than a habit of its users.
- **Store as the filesystem, immediately.** Rejected for now on cost and reversibility, not on merit.
  This RFC deliberately keeps it reachable — that is the point of fixing the interface first. If it is
  ever adopted, nothing above `store://` should need to change, and that claim is the acceptance test
  for whether this RFC's interface was designed correctly.
- **Two independent stores, one for binaries and one for dependency closures.** Rejected: two mechanisms
  serving one primitive, which §1 and §5.4 exist to prevent.

## Graves checked (§3)

None of the four apply directly — this is not a kernel change; the store is userspace. Worth recording
that the store must not acquire kernel support "for speed": that would be policy in the kernel, and it is
the most likely direction from which this design could drift into a grave.

## Open questions for the maintainer

1. Does the store verify **every** read, or only on installation and on a periodic scrub? Verifying every
   read is the strong property; on a server workload it is also a permanent hash cost on every page fault
   against a binary. This is a measurement question, and it is the one place where the substrate decision
   might come back early.
2. Is the store's namespace flat (hash → blob) or does it carry a closure structure the garbage collector
   can walk without parsing app manifests? The second is more work now and much less work at GC time.
3. Does the signing identity attach to the blob, to the closure, or to both? §2 pillar 3 says authors sign;
   it does not yet say what they sign.

These are not blocking. They are the next three RFCs.
