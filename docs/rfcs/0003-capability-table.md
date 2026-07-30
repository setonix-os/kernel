<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0003 — The capability table

| Field | Value |
|-------|-------|
| Status | **Accepted** — 2026-07-30, by the maintainer |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-07-30 |
| Affects | Constitution §3 (kernel doctrine), pillar 2; the syscall ABI; every later subsystem |
| Discharges | Threat-model obligations O-1 (unforgeability), O-2 (non-widenability), O-3 (revocability), O-4 (no ambient authority) |

> **Accepted.** The flat per-process handle table (§4, Option B) is the capability representation;
> derivation is subset-only; transfer is a move; the syscall ABI is capability-indexed from its first
> syscall (§9). Selective transitive revocation remains **open**, deferred to RFC-0003a with B3
> (badges + shallow derivation records) as the proposal on the table — accepting this RFC does not
> pre-empt that verdict. The open questions in §14 are the next design work, and anything the first
> implementation PR proves wrong comes back here as a dated amendment, per house practice.

## 1. The question

**How does a process hold, use, narrow, hand on, and lose the authority to touch a kernel object —
such that authority can never be forged, never be widened, always be revoked, and never be exercised
without naming it?**

That is O-1 through O-4 restated as one sentence. This RFC answers it for the *mechanism*: the table,
the handle, the rights, the transfer, the revocation. It does **not** decide who grants what to whom —
that is the broker's policy, in userspace, and putting it here would be the "policy in the kernel"
grave (§3).

## 2. Which pillar

**Pillar 2 — explicit permission brokering — and the kernel doctrine that capabilities are the only
authority.** Every other pillar rests on this one: an immutable signed app (pillars 1, 3) is only as
safe as its inability to reach beyond what it was granted, and a scheme (pillar 5) is only a boundary
if a capability gates each node. This is not an addition to justify against the primitive; it is the
primitive's enforcement surface. If capabilities are weak, nothing else is strong.

## 3. What a capability is

A **capability** is a kernel-held, unforgeable reference to a kernel object, carrying a set of rights.
Concretely, three things that must never be conflated:

- The **object** — a scheduler-visible thing the kernel owns: an endpoint, a memory region, a thread,
  a scheme port. Lives in kernel memory; userspace never holds a pointer to it.
- The **capability** — `(object, rights, generation)`. Also kernel memory. The rights say *what* may
  be done to the object through this reference; the generation defends against reuse confusion (§8).
- The **handle** — a small integer a process holds, indexing its own capability table. This is the
  *only* part userspace ever touches. A handle is meaningless outside the process that holds it: handle
  7 in process A and handle 7 in process B name unrelated capabilities, or none.

The separation is the whole game. Userspace names authority by an index into a table it cannot write;
the kernel resolves the index to a capability it fully controls. There is no bit pattern a process can
construct that becomes authority it was not given (O-1).

```text
   userspace            kernel
   ┌────────┐           ┌───────────────── process A capability table ─────────────────┐
   │ handle │──7──────► │ slot 7: cap{ object=&Endpoint#42, rights=SEND, generation=3 } │
   │  = 7   │           │ slot 8: cap{ object=&Region#11,   rights=READ, generation=1 } │
   └────────┘           └──────────────────────────────────────────────────────────────┘
                                     │
                                     ▼  invoke(handle=7, SEND, msg)  — kernel checks rights, then acts
```

## 4. Representation — the load-bearing choice

Three lineages offer three shapes for "where capabilities live". This is the decision most likely to
walk into a grave, so it gets the most argument.

### Option A — seL4 CNodes (a guarded radix tree of capabilities)

Capabilities live in *CNodes*: kernel objects that are themselves arrays of capability slots, composed
into a guarded page-table-like radix structure. A capability address is a bit-string walked through
the tree. Immensely powerful: sparse 2^64 capability spaces, capabilities-to-capability-storage,
user-managed cap-space layout.

- **For:** the most rigorously verified capability system in existence; total flexibility.
- **Against:** it is, almost by definition, the thing §3 names as a grave — "**baroque capability
  hierarchies**". The radix walk, the guard bits, the CNode-of-CNodes recursion, and the user-visible
  cap-space management are a large, subtle surface. seL4 affords it because seL4 is formally verified
  and its complexity budget is spent there. Setonix is not, and §5.2 requires the maintainer explain
  every line. **Rejected** as the primary structure, on the grave and on the understandability rule.

### Option B — a flat per-process handle table (Zircon-style) — **recommended**

Each process owns one flat table: `handle -> capability`. Lookup is an array index plus a generation
check: O(1), no tree walk, no user-visible structure to manage. This is Fuchsia/Zircon's handle table,
and §3 already cites "Fuchsia's downgradeable handle rights" approvingly — the rights model this RFC
adopts (§5) is that citation's natural home.

- **For:** small, flat, fast, and legible; a reviewer can hold the whole model in their head. It is
  the direct expression of "capabilities are the only authority" without a metasystem for storing the
  authority. Handle transfer during IPC is a table-to-table move (§6), which is exactly what the "IPC
  is the product" doctrine wants.
- **Against:** no built-in hierarchy means revocation and derivation need their own bookkeeping rather
  than falling out of a tree structure (§7). That bookkeeping is where this option spends its
  complexity — deliberately, in one named place, rather than diffused through a radix walk.

### Option C — L4-style thread-local minimalism

Capabilities held per thread, minimal kernel structure. Rejected: it pushes storage policy onto
userspace prematurely and fits a message-passing microkernel less well than the per-process table,
which matches the "process holds resources, thread runs" split the earlier C++ blueprint already used.

**Verdict sought: Option B.** A flat per-process capability table, with the discipline of seL4's
invariants (unforgeability, monotone rights) applied to a structure simple enough to explain. The rest
of this RFC assumes B.

## 5. Rights and derivation (O-2)

Rights are a bitmask on the capability. The initial set, deliberately small — rights are added when a
subsystem needs one, never speculatively:

| Right | Meaning |
|-------|---------|
| `DUPLICATE` | may derive further capabilities from this one |
| `TRANSFER` | may hand this capability to another process |
| `READ` | may read the object's state / receive from it |
| `WRITE` | may modify the object's state / send to it |
| `REVOKE` | may revoke capabilities derived from this one |

**Derivation** produces a new handle to the *same* object with a rights set that is a **subset** of the
parent's: `derive(h, r) requires r ⊆ rights(h)`. The kernel refuses any `r` with a bit the parent
lacks. This is O-2 made structural — there is no operation, anywhere, that adds a right. Rights are
monotone-decreasing along every derivation chain. *(Fuchsia's downgradeable handle rights; seL4's
diminish.)*

A capability without `DUPLICATE` cannot be derived from at all — a leaf. This is how an app is handed
authority it can use but cannot subdivide and lend onward.

## 6. Transfer, and why Rust models it (O-4 support)

A capability moves between processes only through IPC, and only if it carries `TRANSFER`. Transfer is
a **move**, not a copy: the capability leaves the sender's table and arrives in the receiver's, or —
if the sender asked to keep it — a `derive` happens first and the *derived* capability moves. There is
never a moment where the same capability is live in two tables without a derivation recording it.

§3 says "Rust's ownership and move semantics model capability transfer at compile time", and this is
where that cashes out concretely. The kernel-side capability is an owned Rust value:

```rust
/// A capability is owned. Moving it out of one table and into another is a
/// Rust move; the borrow checker forbids the value existing in two places.
/// There is deliberately no `Clone`: duplication is `derive`, an explicit
/// kernel operation that records the parent, never an implicit copy.
struct Capability {
    object: ObjectRef,   // owns a counted reference to the kernel object
    rights: Rights,
    generation: Generation,
}
```

No `Clone`, no `Copy`. The one way to get a second capability to an object is `derive`, which the
kernel records. The compiler then makes the invariant "a capability is in exactly one table, or in
flight in exactly one message" a *type* property, not a runtime check — the move semantics do the work
the C++ blueprint had to assert by convention.

## 7. Revocation (O-3) — the hard part, stated honestly

Revocation is where capability systems earn their difficulty, and where Option B pays for its
flatness. Two sub-questions:

1. **Object destruction.** When the object itself is destroyed, every capability to it must become
   inert. Solved by the **generation** (§8): the object carries a generation, each capability records
   the generation it was minted at, and every invocation checks them equal. Destroying the object
   bumps its generation; all outstanding capabilities are now stale and fail closed. This is O(1) and
   needs no list of who holds what.

2. **Selective, transitive revocation.** "Revoke this capability *and everything derived from it*,
   without destroying the object." This is the genuinely hard one, and the options differ in cost:

   - **B1 — Derivation tree (seL4's CDT).** Each capability records its parent; revoke walks the
     subtree. Complete and precise, but it reintroduces a tree — the very structure Option B chose to
     avoid — and the walk is unbounded work at revoke time. The risk is re-growing a baroque hierarchy
     through the back door.
   - **B2 — Per-object generation epochs, no per-capability tree.** Revocation is coarse: bump the
     object's generation and re-mint capabilities for the holders who should keep access. Simple and
     bounded, but it cannot say "revoke this branch but not that one" without the broker re-issuing.
   - **B3 — Recommended: badges + a shallow derivation record.** Each capability carries a **badge**
     (a small tag set at derivation) and a one-level parent link, not a full tree. The broker — which
     is the thing that grants authority (pillar 2) — owns the *policy* of what a badge means and which
     badges to revoke; the kernel provides the mechanism "invalidate all capabilities whose badge
     matches, derived from this one". This keeps the kernel's bookkeeping shallow and bounded while
     giving the broker enough to implement precise revocation as *policy*, where §3 says policy
     belongs.

   **The honest position:** revocation semantics are not fully settled by this RFC, and should not be —
   getting O-3 exactly right is worth its own RFC once the broker's needs are concrete. This RFC
   commits to the **generation mechanism for O-3's destruction case** (which the kernel needs from day
   one) and **proposes B3** for selective revocation, explicitly deferring the final choice between
   B1/B2/B3 to RFC-0003a with the broker RFC as input. Naming that gap is the point of writing this on
   paper.

## 8. Unforgeability and reuse (O-1)

Two failure modes, two defences:

- **Forgery** — a process fabricating a capability it was not granted. Closed structurally: userspace
  holds only handles (table indices), the table is kernel memory, and every syscall resolves the
  handle kernel-side. There is no capability bit pattern in userspace to forge. *(seL4; KeyKOS.)*
- **Reuse / ABA** — handle 7 is closed, its slot reused for a new capability, and a stale reference to
  "7" now names the wrong authority. Closed by the **generation** counter on both slot and object: a
  handle resolution checks the generation, and a stale handle fails closed rather than resolving to
  whatever now occupies the slot. The earlier C++ blueprint (`kernel/capability.h` in the BlueOS
  prior art) already reached for exactly this — generation counters "prevent the ABA problem" — and
  that instinct is adopted here rather than rediscovered later.

## 9. No ambient authority — the syscall ABI consequence (O-4)

O-4 is not a property of the capability table alone; it is a property of **every syscall**. The table
only matters if there is no way *around* it. Therefore this RFC constrains the syscall ABI, before it
exists:

- **Every syscall that touches a resource takes a handle.** There is no syscall that names a resource
  by a global name, a path, a PID, or any ambient identifier. `send(handle, msg)`, not
  `send(pid, msg)`. `map(region_handle, ...)`, not `map(address, ...)`.
- **The three exceptions are authority-free:** yielding the CPU, halting the calling thread, and
  querying the caller's own already-held state. None of these reaches another object.
- A syscall that would need to name a resource it was not handed a capability for does not get written;
  the caller must be *given* the capability first, by the broker, as policy.

This is the clause that makes the table load-bearing rather than decorative. It is placed here, in the
first capability RFC, so that the syscall RFC inherits it as a constraint rather than discovering it as
a retrofit.

## 10. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4** | capabilities as unforgeable object references; monotone rights (diminish); the destruction-via-generation discipline | the CNode radix and full CDT — the "baroque hierarchy" grave |
| **Fuchsia / Zircon** | the flat per-process handle table; downgradeable handle rights; handle transfer as a move | the vast object-type zoo; Setonix starts with a handful |
| **KeyKOS / EROS** | that revocation is essential and must be first-class | the deep revocation machinery, deferred to its own RFC |
| **BlueOS (our own earlier C++ attempt)** | generation counters against ABA; a per-process `CapabilityTable`; rights as a bitmask with `derive` | it never booted and modelled transfer by convention, not by the type system — Rust's move semantics replace that |

## 11. Graves checked (§3)

- **Baroque capability hierarchies (KeyKOS).** The central decision (§4) exists to avoid this: flat
  table, not radix; shallow revocation records, not a full CDT. This is the grave this RFC is most at
  risk of and most deliberately steers around.
- **Policy in the kernel.** The table is pure mechanism. *Who* may grant, *which* badge means what,
  *when* to revoke — all broker policy in userspace (§7, §9). The kernel says only "this handle does
  or does not carry this right".
- **Multi-copy IPC (Mach).** Capability transfer is a move, not a copy (§6); it does not force message
  copying and composes with the zero-copy IPC the "IPC is the product" doctrine wants.
- **Bolted-on multicore.** The table is per-process; concurrent access from a process's threads on
  different cores needs a defined synchronisation story. Flagged as an open question (§13), not
  bolted on later.

## 12. Costs — what this makes harder

- **A flat table caps cheap sparse cap-spaces.** A process wanting millions of sparse capabilities is
  served worse than by seL4's radix. Accepted: no Setonix workload in the security target needs that,
  and the simplicity is worth more than the generality.
- **Revocation bookkeeping is manual.** Option B buys flatness by owing §7's revocation machinery
  explicitly. That debt is real and is named, not hidden.
- **The syscall ABI is constrained forever.** §9 forbids ambient syscalls from the first one written.
  That is a cost — some conveniences become impossible — and it is exactly the cost pillar 2 is worth
  paying.

## 13. Obligations discharged

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-1 unforgeability | handles-as-indices + kernel-owned table + generation (§3, §8) | **discharged** |
| O-2 non-widenability | subset-only derivation, no `Clone` (§5, §6) | **discharged** |
| O-3 revocability | generation for destruction now; selective revocation proposed (B3), deferred to RFC-0003a | **partially** — destruction discharged, selective revocation designed not settled |
| O-4 no ambient authority | capability-indexed syscall ABI (§9) | **discharged at the ABI level**; each future syscall must honour it, checked at review |

## 14. Open questions — the next RFCs

1. **Selective revocation** (§7): B1 vs B2 vs B3, decided with the broker RFC as input. Becomes
   **RFC-0003a**.
2. **Multi-core table synchronisation** (§11): how a process's threads on different cores share the
   table without a global lock on the IPC fast path. Likely couples to the IPC RFC.
3. **Memory capabilities and the MMU:** a `Region` capability's rights must agree with the page-table
   permissions the MMU installs (W^X, O-13). The capability table and the MMU RFC must define that
   handshake jointly.
4. **The initial object set:** exactly which kernel objects exist at first boot, and how the very
   first capabilities (the init process's) are minted without an ambient grantor — the bootstrap of a
   system with no root.

These are not blocking. They are the shape of the design conversation this RFC opens.
