<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0003a — Selective revocation

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-10-31, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-10-31 |
| Affects | RFC-0003 §3, §5, §6, §7, §14.1, RFC-0004 §8, §10 and threat-model O-3 (dated amendments on acceptance); RFC-0005 §5, §7, §8, RFC-0006 §4, §7, §11, A6 and RFC-0007 §3, §4, §5, §7, §9, §16, §18 (proposed amendments); `capability/src/rights.rs`, `object.rs` |
| Depends on | RFC-0003 and its 2026-07-30 amendment; RFC-0004; RFC-0005, RFC-0006 and RFC-0007 (proposed), whose open questions it answers |
| Leaves to | RFC-0003b (draft): payer destruction of every other type, `pool_reclaim`, and the Pool rules reclaim needs |
| Discharges | O-3 for grants made through forwarders (text amendment proposed, §10); maintains O-1, O-2, O-4, O-5, O-7, O-23, O-27; partly meets O-21; contributes to O-10, O-22; enables O-26; answers RFC-0003 §14.1 and RFC-0007 §17.2; leaves RFC-0005 §17.2–17.3 to RFC-0003b (draft) |

> **Proposed verdicts.**
>
> 1. **The fork:** option (a), revocable authority routed through kernel forwarders; no derivation tree, no parent link (§3).
> 2. **Forwarder:** wraps an endpoint, notification or forwarder; send side only; four deep; one generation write to revoke; a queued send is checked again (§4).
> 3. **`TRANSFER` unchanged:** nothing unwraps a forwarder, so moved and derived copies stay revocable; a direct grant forgoes revocation (§5).
> 4. **Badges:** one word on endpoints, notifications and forwarders of either; set once at derive, never by a sender; ORed on signal, delivered on `recv` (§6).
> 5. **Destroying a forwarder:** `REVOKE` on the Pool that paid for it; `REVOKE` lives on Pools alone; RFC-0003b (draft) proposes the rest (§7).
> 6. **Eviction:** the broker destroys an app's forwarders and tells its servers; peer-transferred copies die too; RFC-0003b (draft) proposes reclaim (§8).
> 7. **Root:** irrevocable, sound while it serves no client; boot-minted objects are irrevocable once handed on; O-22 gains a switch (§9).
> 8. **Amendments:** badge register and forwarder methods in RFC-0007; forwarders and Pool `REVOKE` in RFC-0005; RFC-0003, RFC-0004, O-3 dated (§10).
> 9. **Demo:** needs nothing; RFC-0007's increments 2 and 7 reserve the badge register so the ABI never changes shape (§15).

## 1. The question

**How does a grantor withdraw authority from one holder, and from everyone that holder passed it to, without
destroying the object for everyone else and without the kernel doing unbounded work?** RFC-0003 §7 deferred
this; its 2026-07-30 amendment turned the deferral into a choice between **(a)** broker-interposed forwarders
with `TRANSFER` unable to bypass them and **(b)** a derivation tree with unbounded revoke. This RFC makes that
choice, and settles what hangs on it before RFC-0007 freezes the syscall ABI. *Who* is revoked and *when* is
the broker's policy; the kernel only makes revocation possible. RFC-0003 §14.1 planned this "with the broker
RFC as input"; the maintainer chose to decide first, so §14.1 is superseded by the duties the broker inherits (§16).
**What a payer may destroy beyond a forwarder, and how a funder takes back everything a Pool paid for, RFC-0003b
(draft) proposes**, split out so that this core can be decided on its own; nothing here depends on it.

## 2. Which pillar

**Pillar 2.** "Cut this app off" is the broker's most basic act, and today the only revocation is destroying
an object for every holder (RFC-0003 §7.1). Pillar 5 decides the scope: every resource an app touches is a
scheme served over an endpoint, so revoking endpoint capabilities precisely is most of what pillar 2 needs.

## 3. The fork: (a) or (b)

- **(b), RFC-0003's B1: a capability derivation tree** *(seL4's CDT)*. seL4 implements it as an `mdb_node` of
  two words in every capability slot (`include/object/structures_64.bf`), and `seL4_CNode_Revoke` deletes
  every CDT child, taking preemption points as it walks (`cteRevoke`, `src/object/cnode.c`); research/0001
  records worst cases of hundreds of thousands of cycles. Barrelfish keeps capabilities in CNodes, a two-level
  L1/L2 table, and every copy in a per-core mapping database, so across cores its delete and revoke run a
  two-phase commit, a mark and sweep sequenced through each capability's owner core
  ([TN-013](https://barrelfish.org/publications/TN-013-CapabilityManagement.pdf) §3.2.1, §4.1, §6.3.5). **Rejected.**
- **B3 as written: badges plus a one-level parent link.** Research/0001 refutes it: one level cannot revoke
  A→B→C, and peer-to-peer transfer bypasses a broker that only interposed at grant time. **Rejected**, but
  the seL4 **badge** it reinvented survives as identity, not as a revocation key (§6).
- **(a) in userspace: a membrane process relaying every call** *(Redell's caretaker run as a domain)*. It
  costs an extra round trip per call, the gVisor tax research/0002 Part 3 measured. A relay re-originates
  requests, which severs O-21's trail, and it inherits RFC-0007 §16's partial interposition. **Rejected as
  the mechanism**; a broker may still run one as policy, for audit, and pay for it.
- **One endpoint per client, destroyed to evict.** Zircon channel endpoints "are not duplicatable"
  ([channel.md](https://fuchsia.googlesource.com/fuchsia/+/HEAD/docs/reference/kernel_objects/channel.md)) and
  no handle operation withdraws another holder's handle
  ([handles.md](https://fuchsia.googlesource.com/fuchsia/+/HEAD/docs/concepts/kernel/handles.md)), so a server
  evicts by closing a client's channel. Closing a Genode session's RPC object means "all capabilities referring
  to the RPC object - however delegated - are implicitly revoked" ([Genode Foundations, "Ownership and lifetime
  of a session"](https://genode.org/documentation/genode-foundations/20.05/architecture/Recursive_system_structure.html)).
  Right idea, wrong layer: an RFC-0004 server receives on one endpoint, with no wait-on-many; in the kernel it
  *is* the forwarder.
- **(a) in the kernel: per-grant forwarder objects. Verdict sought.** Redell's revocation by indirection
  through "link" segments: "indirection through such a link domain is much like calling a simple caretaker
  which merely exercises its capability on demand. (Note, however, that the cost of an actual domain-call has
  been avoided.)" (D. D. Redell, *Naming and Protection in Extendible Operating Systems*, Project MAC TR-140,
  1974, §2.3, p. 54; the scan at [MIT CSAIL](https://publications.csail.mit.edu/lcs/pubs/pdf/MIT-LCS-TR-140.pdf),
  also [DSpace 1721.1/149438](https://dspace.mit.edu/handle/1721.1/149438)). Shapiro, l4-hurd, 2005-10-09:
  "destroy the wrapper, you are guaranteed that no further use of the wrapped capability through that wrapper is
  possible" ([msg00044](https://lists.gnu.org/archive/html/l4-hurd/2005-10/msg00044.html)). The wrapper's "main
  problem" was performance, "it requires a number of IPCs" ([msg00041](https://lists.gnu.org/archive/html/l4-hurd/2005-10/msg00041.html)),
  which he does not place at creation or at use; here creation is one Pool method (§4) and use adds no IPC.
  Miller's forwarding and revoking facets are the same pair; he credits Redell with "exactly this method"
  ([Capability Myths Demolished](https://papers.agoric.com/assets/pdf/papers/capability-myths-demolished.pdf), 2003).

| Cost | (a) kernel forwarders | (b) derivation tree |
|------|-----------------------|---------------------|
| Kernel memory | one object slot per *revocable grant*, charged to the grantor; one badge word per capability (§6) | two link words in *every* capability slot, revoked or not, plus the badge |
| Revoke latency | O(1) at revoke; a dead wait cleared when it reaches the head of its queue, O(1) per pop but a short caller's release-queue insertion, counted in the worst path (§4) | O(descendants), made restartable by preemption points |
| O-7 exposure | bounded by the depth limit and the thread capacity; the grantor pays for its own forwarders | revoke work grows with descendants other processes created, in tables the revoker cannot see |
| Multi-core | under RFC-0006's big lock both are serialised; (a) holds it for one generation write and RFC-0005 §5's refund | the same lock, held for O(descendants) between preemption points |
| Review (§5.3) | one object type, one resolve loop, one refusal rule | every move, derive and process death maintains cross-table links |
| Precision | reaches only what a grantor routed through a forwarder | reaches everything derived, wherever it went |

**Verdict sought: (a).** In B-terms this is **B2 made selective**: a forwarder is an object whose only job is
to have a generation that can be bumped, so B2's epoch applies to one grant, not a whole object. B1 is
rejected. B3 splits: the badge stays, the parent link goes, and the crate's
`a_removed_parent_leaves_its_derived_sibling_untouched` stays true permanently. **The honest cost is the last
row:** (b) protects a grantor who forgot to interpose; (a) only one who did, a discipline the broker RFC owes.

## 4. What a forwarder is

- **An object** in RFC-0005's typed arrays, charged to the Pool that created it. It holds a link to what it
  wraps, its depth and its payer. The link is an object reference, not a capability: counted when it names a
  forwarder, and an `(index, generation)` pair checked on use when it names an endpoint or notification, as
  RFC-0006 §4's internal links are. An endpoint's life is therefore decided by direct capabilities alone: it dies
  with the last of them, which is the server's when every grant went through a forwarder and no one else, the
  root included (§9), kept a direct copy; its queued senders then wake (RFC-0006 §4), and every forwarder over it
  answers `Revoked`.
- **Created by a Pool method**, `forwarder_create(pool, cap, rights)`, needing `WRITE` on the Pool. `cap` is
  presented, not moved, carries `DUPLICATE` (wrapping is a derivation) and names an endpoint, a notification,
  or a forwarder of depth under four. The reply is a forwarder capability with `rights ⊆ rights(cap)` and
  `cap`'s badge.
- **Use resolves; wrapping and moving do not.** `send`, `call`, a signal, and fault or timeout delivery first
  check the invoked capability's right, then resolve the chain to the leaf, checking each link's generation, and
  act with the invoked capability's rights and badge; a dead link answers `Revoked`. Checking the right first
  means a holder without it learns nothing about whether the grant is still live, at entry and at rendezvous
  alike. The loop takes at most the invoked forwarder's stored depth in hops and fails closed short of a leaf, so
  a cycle is stopped by that bound as well as by the generations. `forwarder_create`, `cap_derive`,
  `set_fault_ep`, `set_timeout_ep`, `GRANT` and every transfer act on the forwarder itself: wrapping a depth-2
  forwarder yields depth 3, never a forwarder of the leaf (§5). A forwarded `call` stays on RFC-0004 §5's fast
  path, one more slot read and compare per hop.
- **Send side only.** `recv`, `nb_recv`, `reply_recv`, `bind_notification` and an `IrqHandler`'s binding answer
  `WrongType` for a forwarder. The last two leave a link that works without the capability (RFC-0006 §4, §11),
  which destroying a forwarder could not reach; the others serve no step of §8, and refusing them keeps every
  receiver's authority direct, so no revocation kills a waiting receiver; the receive-side check below covers
  what a receiver's own siblings change. A forwarder capability may rest in `fault_ep` or `timeout_ep` (A6),
  moved in by `set_fault_ep` or `set_timeout_ep` with `WRITE` and `TRANSFER` (P2).
- **Queued parties are checked again at rendezvous.** A queued thread keeps its handles in its frame (RFC-0007
  §5) or, for a fault, in its TCB slot, and `write_regs`, `set_fault_ep`, `set_timeout_ep`, `cap_close`, a move
  or a revocation can change what they reach while it waits. So when a partner arrives the kernel checks the
  queued party at the head of the queue again, in the order of the tables below, and stops at the first check
  that fails. seL4 instead cancels badged sends eagerly at the revoker's cost (`cancelBadgedSends`,
  `src/object/endpoint.c`); here revocation stays one generation write and the queue clears itself.
- **Not a cached resolution.** RFC-0003's 2026-07-30 amendment forbids caching a resolved capability outside the
  table, and `object.rs` acting on a reference after its resolve. A forwarder's link confers nothing alone:
  authority comes from the invoking capability, resolved and held across check→act, and every hop re-checks the
  link's generation inside that resolve, in one entry under RFC-0006's big lock. The link is an object reference
  of the kinds RFC-0005 §5 and RFC-0006 §4 already keep, beside Mappings, IPC-buffer bindings, a reply object's
  record of a lent context and an `IrqHandler`'s notification; RFC-0005 §5's list of live references gains it
  (P1).
- **Four deep at most.** "EROS imposes a constant bound on this depth in order to ensure that all kernel
  operations execute in bounded time", and deep chains were "VERY rare" in Shapiro's experience
  ([msg00040](https://lists.gnu.org/archive/html/l4-hurd/2005-10/msg00040.html)). Root, broker, app and helper
  fill the four levels (§9); measurement may revise it. Destroying a forwarder (§7) advances its generation,
  reuses its slot and refunds its payer; one also dies at its last live reference (RFC-0005 §5), cascading at
  most four links. The revocation is one generation write; the refund follows RFC-0005 §5's forwarding chain of
  closed Pools, as every object's refund does, which that RFC bounds by the Pool slot count and RFC-0003b
  proposes to bound by a constant.

**The rendezvous rule.** The TCB records a queued sender's kind, written by the kernel when it blocks: send,
call, fault or timeout fault, two bits (P2). The kind comes from there, never from `x8`, and decides which
handle is re-resolved and which checks apply. At the head of an endpoint's send queue:

| # | Check | Kinds | On failure | Effect |
|---|-------|-------|------------|--------|
| 1 | the handle resolves: frame `x0` for send and call, the `fault_ep` or `timeout_ep` slot for a fault | all | send, call: `InvalidHandle`; fault: the thread goes `Inactive`, as a fault with no handler does (RFC-0006 §4); timeout fault: it is left `Unfunded`, as with no live `timeout_ep` (RFC-0006 §7) | nothing delivered; pop; next |
| 2 | the capability holds `WRITE`, checked before its chain is walked, as at entry | all | `InsufficientRights`; a fault or timeout fault as in row 1 | the same |
| 3 | every link in its chain is live, within the stored depth | all | `Revoked`; a fault or timeout fault as in row 1 | the same |
| 4 | the leaf is the thread's `blocked_on` | all | `Revoked`; a fault or timeout fault as in row 1 | the same |
| 5 | the frame's tag is valid as at entry: bits 9–15 zero, a length of at most 64, at most three capabilities | send, call | `InvalidArgument` | the same |
| 6 | a tag needing the IPC buffer (a length over four, or any capability) has one | send, call | `NoBuffer` | the same |
| 7 | a context is held, and it can ever meet the endpoint's `min_budget` | call | `BudgetRefused` | the same |
| 8 | that context meets `min_budget` now | call | none, as RFC-0006 §7's A2 has a short caller do: with a partial context the caller leaves the queue `Throttled`, its refills merged and deferred, and restarts its `call` when the head refill suffices; with a full context, which is never throttled (RFC-0006 §5), it goes `Ready` at the tail of its level and restarts on its next timeslice | nothing delivered; pop; next. The partial case costs A2's merge, O(refills) ≤ 8, and an insertion into RFC-0006 §6's sorted release queue, O(n) in contexts and deadline objects |
| 9 | `caps[]` are fetched and checked as RFC-0007 §5 says | send, call | RFC-0007 §5's status | the same |
| — | every check passes | | | deliver under the badge of the capability resolved in row 1; a send or call carries its frame's tag and MR0–MR3, a fault or timeout fault the kernel's fault message, whose tag the kernel builds (P5) |

Faults and timeout faults skip rows 5 to 9, since their `x1` is user data and both bypass `min_budget` (RFC-0006
§4). A sender that rewrites its handle to another of its own capabilities to the same endpoint, one with
`WRITE`, sends under that one's badge, as it could by sending again; MR0–MR3 are payload, which a holder of
`WRITE` on the thread could have sent anyway. A thread waiting behind live senders is checked only when it
reaches the head. When a sender arrives and finds receivers queued, the head receiver is checked likewise:

| # | Check | On failure | Effect |
|---|-------|------------|--------|
| R1 | the receiver's frame `x0` resolves to its `blocked_on` | `InvalidHandle`, or `Revoked` if it names another object | the receiver completes with that status, a passive one left `Unfunded` as any passive thread woken without a donation (RFC-0006 §4); pop; the sender meets the next receiver, or queues |
| R2 | that capability holds `READ` | `InsufficientRights` | the same |
| R3 | for a sender awaiting a reply, a call or a fault, the receiver's reply-object handle (`x6`, `r12`) resolves and is checked as at entry | that check's status | the same |
| — | every check passes | | the rendezvous proceeds, its reply object bound |

A thread waiting on a notification by handle, `BlockedOnNotification` after a `recv` on it, is checked by R1 and
R2 when a signal would wake it: one whose handle no longer names that notification with `READ` completes with
that row's status, and the signal wakes the next waiter or stays pending in the word, each such pop O(1) and
counted like the others. A **bound** notification is exempt: `bind_notification` checked `READ` when it bound,
and the binding is a link that works without the capability by design (RFC-0006 §4), ended only by
`unbind_notification`, which is why a forwarder cannot be bound (above).

**The pops take no preemption point.** A pop is O(1), at most four hops, a few compares, a status write and a
run-queue insert, except a row-8 pop of a partial context, which costs a release-queue insertion, O(*C*) in the
release queue's length *C*. A rendezvous pops at most the thread capacity *N*, so its worst case is O(*N*·*C*)
with interrupts masked, counted in the worst path as RFC-0006 §6 counts its own queue walks. Each pop ends the
wait it pops: a caller popped at row 8 restarts and may queue again, as a new wait.

**Rejected:** forwarders over other types: a Region or Pool reached through one creates objects and mappings that
answer to no forwarder, so revoking it would promise more than it delivers; ending memory is its payer's, which
RFC-0003b (draft) takes up. Also a kernel **membrane** wrapping every capability a forwarded message carries: an
allocation per message (O-7), and it would still miss what servers return, since a reply never crosses the
forwarder; a capability a server hands back is the server's to make revocable (§8 step 5). Also, for a queued
sender: snapshotting its badge and leaf into the TCB at block time *(seL4's `blockingIPCBadge`)*. That would not
be a forbidden cache, since `blocked_on` and an `IrqHandler`'s binding already keep such references, but it pins
little: the destination is pinned by `blocked_on` already, the badge only against a sender that could send again
under its other capability, and a revocation is still seen only by resolving the chain again. It would cost two
words per TCB. Snapshotting the tag costs a word for checks entry already makes, and leaves MR0–MR3 in the frame
regardless; only the sender's kind, two bits, is kept in the TCB state (P2). Also making `write_regs` or
`set_fault_ep` cancel the wait, a side effect RFC-0006 §4 does not give them, which would abort the IPC of any
thread a supervisor adjusts. Also a preemption point between pops: a `reply_recv` preempted after its reply half
cannot restart, since rewinding its PC replays the reply on a reply object already unbound, and a passive server
that has handed its context back cannot run to restart at all, so it would be left neither receiving nor funded;
keeping the receiver `BlockedOnRecv` with the queue marked instead needs someone to resume the pops, and no one
is there to.

## 5. `TRANSFER` under (a)

**`TRANSFER` keeps RFC-0003 §6's meaning. The constraint the amendment asked for is structural: no operation
yields a capability to the leaf from a capability to a forwarder (§4).** A forwarder capability that is moved,
derived or `GRANT`ed still names the forwarder, so a peer-to-peer transfer stays inside the revocation.
Redell names the failure this rule excludes: a holder that "can obtain a copy of a capability stored in the
chain, thus circumventing subsequent revocation" (TR-140, §2.4, p. 56). Miller states the property kept: "Since
Bob only ever has access to F, not to Carol herself, Bob can only give F to Ted". What remains is policy. A
grantor that hands over any direct capability has given up selective revocation of it, and `TRANSFER` on such
a grant also spreads it, so the broker RFC must route every revocable grant through a forwarder. Withholding
`TRANSFER` stops a holder handing the capability on, not acting on another's behalf: a holder can always proxy.

**Rejected:** refusing `TRANSFER` on direct endpoint capabilities, which proxying defeats, and routing every
transfer through the broker, back on the hot path (O-23); both write grant policy into the kernel. **For the
maintainer:** reading "constrain `TRANSFER`" as a structural rule, not a change to the right, is a verdict sought.

## 6. Badges

**A badge is one word (64 bits) on a capability to an endpoint, a notification, or a forwarder of either,
naming the grant the capability belongs to.** That is seL4's width on 64-bit platforms, under its rule that
badged capabilities "cannot be unbadged, rebadged or used to create child capabilities with different badges"
(seL4 manual, `manual/parts/ipc.tex`). Zero means unbadged.

- **Set once, at derive, never by a sender.** `cap_derive` from an unbadged capability may set one; from a
  badged one it inherits, refusing a different value. `forwarder_create`, transfers and `GRANT`s carry it, so
  a forwarder capability takes badges as the object it resolves to; on any other type a badge is refused.
  **Consequence:** unbadged with `DUPLICATE` is the authority to choose badges, so a grantor badges first.
- **On a signal:** a notification ORs in the badge of the capability signalled through
  (`manual/parts/notifications.tex`). An unbadged signal sets bit 0, keeping RFC-0007's interim, so a thread
  that waits later still finds it pending; seL4 instead "degrades to just waking up the first thread". An
  interrupt signals as its binder would: RFC-0006 §11's `bind` loses its `bits`, and the line ORs the badge of
  the notification capability presented, or bit 0 (P2).
- **On a message:** `recv`, `nb_recv` and `reply_recv` deliver the badge of the capability the sender invoked,
  in a dedicated result register (§10). A fault or timeout message delivers the badge of the capability resting
  in the thread's slot. A notification wake, a reply delivered to a `call`er and every other syscall leave the
  register zero; a thread resumed after an interrupt, an exception or a fault reply gets its own value back from
  its frame. **This answers RFC-0007 open question 2.**
- **What it identifies:** the grant, not the sending process: O-21's "specific capability that authorised it",
  bound by the kernel, and the per-client key O-26's servers and RFC-0006 §7's timeout handler lack. When B
  delegates its forwarder capability to C, the server sees B's grant: the actor is lost unless `TRANSFER` was
  withheld (§11).

**Rejected:** sender-chosen badge bits (the forgery RFC-0007 refuses), an `IrqHandler`'s binder included, and
bits limited to a subset of the presented badge, which can still spell another grant's; badges on forwarders
only, making every identified client cost a forwarder; the badge in the error-detail register, one register
with two meanings.

## 7. Destroying a forwarder

**Verdict sought: `REVOKE` on the Pool a forwarder is charged to authorises destroying it.** `destroy(pool, fwd)`
is invoked on the Pool; `fwd`, presented with no right checked, designates, and the Pool's right authorises:
Shapiro's "The party who pays must always be permitted to deallocate" (msg00041), and seL4 Untyped revoke's
authority without its tree. Either reading of RFC-0006 open question 7 fits, so it stays open: the designating
capability is asked for nothing. `destroy` advances the forwarder's generation and refunds its payer (§4), so
every capability through it fails `Revoked` on next use and a wait through it completes when it reaches the head
of its queue. Here `destroy` accepts a forwarder alone and answers `WrongType` for anything else, and
`InsufficientRights` for a forwarder charged to another Pool: a funder's `REVOKE` does not reach the forwarders
its child Pools paid for. RFC-0003b (draft) proposes extending `destroy` to every type and a walk that reaches
down the funding tree.

- **`REVOKE` means one thing on one type.** It joins `WRITE` (spend) and `READ` (query) on Pools, is refused
  at mint on every other type, as `EXECUTE` is off-type (RFC-0005 §8), and Regions lose it. The root Pool and
  the capability `pool_split` returns carry it. Splitting it from spending lets a revoker hold no budget (O-22's
  least privilege). It replaces RFC-0003 §5's "revoke capabilities derived from this one", a relation the kernel
  no longer records; its reach in this RFC is the forwarders a Pool paid for.
- **The payer, not the holder.** The forwarder is the grantor's object and the capabilities to it are the
  grantee's, so the grantee can use a grant and pass it on but never end it for the others it reached.
- **Kept alongside:** destruction at the last live reference (RFC-0005 §5), which ends a forwarder nobody can
  name any more.

**Rejected:** `REVOKE` on the forwarder itself, which would let any holder of a peer-transferred copy cut off
everyone the grant reached; a new `DESTROY` right that would mean what `REVOKE` should mean; `WRITE` on the
forwarder, which would grow `WRITE` towards the catch-all right.

## 8. Cutting an app off, end to end

1. **Grant.** For each scheme capability, the broker derives a capability badged with the grant's identity,
   wraps it in a forwarder charged to a Pool whose `REVOKE` it holds, and hands the app a forwarder capability
   carrying `DUPLICATE`, and `TRANSFER` only where policy allows delegation, keeping a rightless one to name it
   later.
2. **Use.** The app calls the server directly. The broker is off the hot path and outage stops only new
   grants (O-23). The server sees the badge on every request.
3. **Peer transfer.** If the app moves its capability to a peer, the peer's handle still names the forwarder.
4. **Evict.** The broker `destroy`s the forwarder through its Pool: one generation advance. Every capability
   through it fails `Revoked` on next use (the app's, the peer's, one in a fault slot, every chained forwarder);
   waits complete `Revoked` as each reaches the head of its queue, and requests in service run to their reply.
5. **Tell the servers** (protocol, the broker RFC's): each drops the badge's state and ends what it made on that
   client's behalf: its own forwarders by `destroy`, through a Pool whose `REVOKE` it holds, such as a
   `pool_split` child of its own, and the rest as RFC-0005 ends objects today, at the last live reference.
   Capabilities a server returned to the client are direct, untouched by step 4, so a server that returns
   per-client authority wraps it in a forwarder of its own before returning it. A shared Region, which no
   forwarder wraps (§4), lives while the client holds a capability to it; ending it under the client needs payer
   destruction, which RFC-0003b (draft) proposes.

A server's death needs no step: its endpoint dies with its last direct capability, the server's when every grant
went through a forwarder and the root closed its own copy (§4, §9). Eviction does not reach authority minted
without a forwarder, which is its minter's to revoke, what a server returned without wrapping it, nor data
already read. **Nor does it reach the app's own objects:** a broker that spawned the app keeps its Process
capability and can `KILL` it (RFC-0007 §8), but what the app paid for and moved to peers outlives it while any
capability to it lives. Taking that back, wherever it went, is the `pool_reclaim` RFC-0003b (draft) proposes.

## 9. Boot capabilities and the root (RFC-0007 §9)

**Nothing revokes the root, and that is correct while the root serves no one.** Revocation needs a revoker with
authority over the revoked, and the root has no superior; making the kernel that revoker is the "policy in the
kernel" grave. The precondition is that the root runs no client-facing code: a root that parses requests from
untrusted processes while holding every boot capability is a compromise nothing can undo. RFC-0007 §18's demo
interim, the root doubling as server until that RFC's increment 15, breaks it for as long as it lasts (P6). An
endpoint or notification the root creates after boot it can hand on inside a forwarder, which makes the grant
revocable; any other object it creates after boot ends as RFC-0005 ends it, at its last live reference; RFC-0003b
(draft) proposes giving its payer `destroy`. **Boot-minted objects are irrevocable once handed on:** kept, never
charged (RFC-0005 §5) and not forwardable, so the module and DTB Regions, the Console, each `SchedControl` and
the `IrqControl` stay with whoever receives them, RFC-0006 §17's "exclusivity is checkable, not revocable" made
general. So does an `IrqHandler`, minted after boot with no Pool presented (RFC-0006 §11), unless RFC-0003b's
draft proposal to charge it is accepted. **The root's rule:** hand boot-minted objects on only inside its own
trust boundary, as copies without `DUPLICATE`, or copy module data into Regions first, and never to the broker. A
move needs `TRANSFER` (RFC-0003 §6), so a recipient can still move its one copy on; it cannot multiply it. **One
exception stands until RFC-0007's increment 13 deletes the Console:** the demo client's Console copy (RFC-0007
§11), outside the root's trust boundary, irrevocable short of `KILL`ing whoever holds it and bounded by what a
Console does, 24-byte writes to the operator's line. Of research/0002 Part 3's "ordinary, enumerable, revocable"
init capabilities, this meets the first two in full and the third for what the root hands on through forwarders.

**Who holds `SchedControl` and `IrqControl` once the root serves no one is left open here.** Both are boot-minted
and no forwarder wraps them, so whoever holds them holds them irrevocably; RFC-0003b (draft) proposes a
resource server for them.

**The root wraps what it gives the broker.** With each server endpoint handed to the broker inside a root-held
forwarder, one `destroy` by the root cuts off the broker's own use of those endpoints and every grant it made
through them, however far the copies moved. The root then closes its own direct copy, so the endpoint still dies
with its server (§4); one it keeps to wrap again later keeps the endpoint alive after the server dies, and
senders then wait on a server that is gone. The root's `destroy` does not reach capabilities servers returned
through them, to the broker or to its grantees, which the broker may have granted on directly: those end only
when the servers that returned them are told, as in §8 step 5. That gives O-22 a recovery switch, not its bound:
the broker's standing capability to every scheme endpoint is revocable but present, and the broker RFC must argue
the blast radius from that. With the root's rule it answers part of RFC-0007 open question 6, and it is why the
depth limit is four, not two.

## 10. Amendments proposed to other documents

Accepted documents gain dated amendments when this RFC is accepted; proposed ones absorb these before their own
verdict. Each change is logged in `docs/CHANGELOG.md`. RFC-0003b proposes its own, on top of these.

| # | Target | Change | Why |
|---|--------|--------|-----|
| R1 | RFC-0003 §3, §5, §6, §7, §14.1 | §3's capability gains a badge; §5's `REVOKE` becomes "on a Pool, destroy what it paid for", reaching forwarders; §6's derivation record is never built, and a forwarder's link is an object reference, not a capability, so §6's invariant gains no new place; §7's choice is (a); §14.1 superseded (§1) | the deferral ends here |
| R2 | RFC-0003 2026-07-30 amendment; `object.rs` | the no-cache invariant and `ObjectRef`'s contract add: "a forwarder's link, re-checked on every hop inside the invoking resolve, is not a cached resolution" | §4 |
| R3 | RFC-0004 §8, §10 | a signal ORs the capability's badge, or bit 0 if unbadged; `recv` delivers the sender's badge | §6 |
| T1 | threat-model O-3 | "reaches capabilities transitively derived from the revoked one" becomes "reaches every capability derived, moved or chained from a grant made through a revocable indirection; a capability a server returns in answer is that server's to revoke" | (a) does not reach direct derivation (§3), nor what servers return (§4, §8) |
| P1 | RFC-0005 §5, §7, §8, §10, §14, §16, §17.2–17.3 and verdict 10 | `Forwarder` joins the arrays, and §5's list of live references gains a forwarder's link to a forwarder; Pool rights gain `REVOKE`, refused at mint off Pools, and Region rights lose it, so §10's "without `EXECUTE`, `DUPLICATE` or `REVOKE`" drops `REVOKE`; the root Pool and `pool_split`'s returned capability carry it; §17.3's right of explicit destruction is answered for forwarders, and the pointers that hand donation, reclaim and the rest of §17.3 to RFC-0003a (verdict 10, §10, §14's O-7 row, §16, §17.2–17.3) point at RFC-0003b (draft) instead | §4, §7 |
| P2 | RFC-0006 §4, §7, §11, A6 | `set_fault_ep` and `set_timeout_ep` accept a forwarder capability, moved with `WRITE` and `TRANSFER`, to rest in `fault_ep`/`timeout_ep`; fault and timeout messages carry its badge, replacing §7's "badges are reserved zero"; `BlockedOnSend` records the sender's kind, send, call, fault or timeout fault, and a queued sender or receiver, or a thread `BlockedOnNotification` by handle at a signal, is checked again by RFC-0003a §4's tables, the right before the chain, with no preemption point between pops, a short caller deferred by A2 as at entry; a timeout fault that cannot be delivered leaves its thread `Unfunded`; `bind` loses `bits` and ORs the presented notification capability's badge, or bit 0; open question 7 untouched | §4, §6 |
| P3 | RFC-0007 §3 | **badge (out): AArch64 `x9`, x86_64 `r14`**, written by `recv`, `nb_recv` and `reply_recv`, a fault or timeout message included, and zero on every other syscall's return; a return from an interrupt or exception, a resumption after a fault reply and a PC-rewind restart restore it from the frame, under §3's rule; `abi` marks it clobbered by every syscall | open question 2; neither register is in use or reserved by Rust inline assembly; `r14` is callee-saved under SysV, so a live value is saved around the call |
| P4 | RFC-0007 §4, §5 | "A signal sets bit 0 … until badges exist" becomes "a signal ORs the capability's badge, or bit 0 if unbadged"; `cap_derive`'s MR1 goes live; an `IrqHandler` ORs its bound capability's badge, not its bound bits; `send`, `nb_send` and `call` accept a forwarder, the `recv` rows and `bind_notification` refuse one, and the `set_fault_ep` and `set_timeout_ep` rows accept one; Pool rows, labelled in `abi`'s table of record: `WRITE` gains `forwarder_create` (MR0 the capability, presented `DUPLICATE`; MR1 the rights), `REVOKE` gains `destroy` (MR0 the forwarder, presented, no right checked); §5's blocked sender has its tag checked again at rendezvous, and a blocked receiver its endpoint and reply-object handles, or its notification handle | §4, §6, §7 |
| P5 | RFC-0007 §7 | `Revoked`: "the invoked capability no longer reaches its object: one in its chain was destroyed or, for a queued sender or receiver, it now leads elsewhere; a completed wait included"; `PeerGone` narrows to "a party (caller, reply binding) went away"; `BudgetRefused` adds "or a queued `call` that, at rendezvous, holds no context"; a fault message's tag is the kernel's, never a frame word; a forwarder's right is checked before its chain, so `InsufficientRights` precedes `Revoked` | one status per event, however indirect; §4's tables |
| P6 | RFC-0007 §9, §16, §18, §19 | §9's "not revocable until RFC-0003a" becomes "what the root hands on through a forwarder is revocable; boot-minted objects are not (RFC-0003a §9)"; §16's root bullet drops the same clause; §16's "a forwarder for a method" becomes "a stand-in server", whose partial interposition is off the revocation path; §18's root-as-server interim gains a cost: the root's irrevocability assumes it serves no client; §19's sovereignty check goes to RFC-0003b (draft) as input, not to RFC-0003a | §9; one meaning per word |

**RFC-0007's verdicts 2, 7 and 9 need nothing further.** Presented handles gain only P4's refusals and rows, the
20/44 handle word is untouched because forwarder generations are object generations held in kernel memory, and a
spawner that wants revocable grants simply `GRANT`s forwarder capabilities inside the window.

## 11. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-3 revocability | per-grant forwarders, one generation write, reaching derived, moved and chained copies; destruction by the forwarder's payer (§4, §7) | **discharged for grants made through forwarders** under T1's text, not for what servers return through them, which they revoke (§8 step 5); under today's text, partial |
| O-7 no unprivileged exhaustion | forwarders charged to their creator; depth ≤ 4; dead waits cleared at the queue head, O(1) per pop but a short caller's release-queue insertion, at worst O(threads × contexts) per rendezvous, counted in the worst path; a revocation is one generation write, its refund RFC-0005 §5's | **maintained**; RFC-0005's reclaim gap stays open, for RFC-0003b |
| O-1, O-2, O-4, O-5 | forwarder rights ⊆ the wrapped capability's; no unwrap; badges never sender-set, not even by an `IrqHandler`'s binder; a queued send reaches only its own endpoint, under a capability that still holds `WRITE`, its frame words validated again at rendezvous | **maintained** |
| O-10 no confused deputy | a request presents the app's own forwarder capability, whose badge the server sees | **contributes**; the broker RFC's |
| O-21 mediation auditability | the kernel delivers the grant's badge on every message (§6) | **partial:** the badge names the grant; delegation collapses actor identity unless the grant lacks `TRANSFER` |
| O-22 broker containment | the root wraps the broker's endpoints, reaching its grants but not what servers returned to it; a revoker holds `REVOKE` without spending (§7, §9) | **contributes:** the broker's standing authority is made revocable, not removed; the broker RFC argues the bound |
| O-23 fail-closed mediation | no per-use broker consultation (§8) | **maintained** |
| O-26 bounded service per client | badges give multiplexing servers a per-client key | **enables** |
| O-27 explicit authority at spawn | revocable grants are forwarder capabilities moved through the `GRANT` window | **maintained** |

## 12. Graves checked (§3)

- **Baroque capability hierarchies (KeyKOS).** No derivation tree or parent link; the table stays flat. The
  grave is radix capability storage (research/0001), not one bounded, single-type indirection. The grant tree
  lives in the broker's policy.
- **The catch-all right.** `REVOKE` means "deallocate what this Pool paid for", here one forwarder per
  `destroy`, and confers no use. Designation without a right confers nothing.
- **Policy in the kernel.** The kernel revokes nothing unasked; whom to cut off, and when, is the broker's.
- **Bolted-on multicore.** A revocation writes one generation under RFC-0006's big lock; resolve→check→act
  holds its resolution (`capability/src/object.rs`), now across at most four links. No global index.
- **Multi-copy IPC.** Unchanged: forwarders resolve, they do not relay.

## 13. Costs — what this makes harder

- **Revocability is a grantor's discipline.** A grant without a forwarder is irrevocable short of destroying
  the object, which is the price of rejecting (b).
- **Memory:** one word per capability for the badge and one slot per revocable grant, charged to the grantor;
  one slot read per hop on every forwarded call.
- **Revocable sub-delegation is four levels deep**; moves and derives are unlimited, deeper wrapping proxies.
- **Forwarders are send-side:** a notification granted through one cannot be bound to a thread or an IRQ line.
- **Boot-minted objects are irrevocable once handed on** (§9), and so are `IrqHandler`s unless RFC-0003b's draft
  is accepted; the root's rule is all that guards them, and the root's own irrevocability is sound only while it
  serves no client, which the demo's root does not meet.
- **The queue clears dead waits:** a receiver's entry pays for each it pops, O(1) apiece except a short partial
  caller's release-queue insertion, so one rendezvous is O(threads × contexts) at worst, with interrupts masked,
  and the published worst path grows by that much; a dead wait behind live senders lingers until it reaches the
  head.
- **Revocation needs the payer's capabilities to outlive the broker.** `destroy` needs `REVOKE` on the paying
  Pool and a capability naming the forwarder. If the broker crashes or is `KILL`ed, its table drops both, and
  every forwarder it paid for becomes irrevocable short of the root's switch, which cuts off every app at once.
  The broker RFC should therefore keep a `REVOKE`-only capability to each paying Pool, and the naming handles,
  outside the broker; RFC-0003b (draft) proposes a charge list that would make the naming handles unnecessary.
- **Eviction ends authority, not memory.** Servers must cooperate for state they made per client and must wrap
  what they return (§8 step 5); **memory revokes by the Region**, not per sharer, so a server shares one Region
  per client to evict one; and an evicted app's own objects live on while any capability to them does, short of
  the reclaim RFC-0003b (draft) proposes.
- **The badge register costs a clobber:** `abi` marks `x9` or `r14` clobbered by every syscall, so on x86_64
  a live `r14`, callee-saved under SysV, is saved around each.
- **Notification badges identify only when grants use disjoint bits:** signals OR together, so 0b11 is both
  badge 3 and badges 1 and 2 at once, and an unbadged signal sets bit 0. One notification therefore tells
  apart at most 64 signallers, 63 beside unbadged ones; endpoint badges are whole words and not ORed.
- **Requests in service finish**: revocation stops new invocations, not one already accepted.

## 14. Open questions

1. **The depth limit**: four for forwarders, from Shapiro's report and §9's chain; to revisit with measurement.
2. **Eviction notice to servers**: a broker protocol (proposed, as §8 assumes) or a kernel notification?
3. **Eager or lazy clearing**: does measurement favour seL4's revoker-paid queue walk over §4's dead waits?
4. **`REVOKE` before reclaim**: this RFC gives Pool `REVOKE` one reach, the forwarders a Pool paid for, and takes
   `REVOKE` off Regions. RFC-0003b would widen the reach to every type and add `pool_reclaim`. Is a right
   defined now with a reach that may grow later the right shape to accept first, and does it still stand if
   RFC-0003b is rejected?

## 15. Implementation increments

None is demo work, **except that RFC-0007 reserves the badge register**, so the ABI keeps one shape: its
increment 2 names it in `abi`'s data half and its increment 7's `svc`/`syscall` wrappers mark it clobbered.
Each increment below is one reviewable PR.

1. **`capability`: the badge and `REVOKE`.** The set-once badge on `Capability`; `REVOKE` documented in
   `rights.rs`, refused at mint off Pools, dropped from Region rights; `object.rs` qualified (R2). Host tests:
   rebadging refused; badges inherited; the pinning test unchanged.
2. **`capability`: chained resolution** over a test-double forwarder. Host tests: depth five refused; a dead
   link fails closed; wrapping a forwarder gives depth + 1, never a forwarder of the leaf; cascades stop at four;
   links forced into a cycle stop at the stored depth.
3. **Kernel: `Forwarder` and `forwarder_create`**, with RFC-0007's increment 10: forwarded calls, the send-side
   refusals, and §4's tables for send and call senders and for receivers, popping at the queue head with no
   preemption point. A queued sender whose frame is rewritten completes and delivers nothing: to another
   endpoint's handle, `Revoked`; to a capability without `WRITE`, `InsufficientRights`; to a tag with bit 9 set,
   `InvalidArgument`; to a tag needing a buffer it lacks, `NoBuffer`. A receiver whose handle is rewritten
   completes `Revoked` and the sender meets the next receiver.
4. **Kernel: the budget and fault rows of §4's tables**, with RFC-0006's increments 8 and 10 and RFC-0007's
   increment 11. A queued `call` whose context is swapped for one that can never pass completes `BudgetRefused`,
   for a partial one short only now waits `Throttled` and restarts, for a full one short only now goes `Ready` at
   the tail of its level and restarts; a fault whose slot was replaced leaves its thread `Inactive`, a timeout
   fault its server `Unfunded`; a fault whose `x1` has bit 9 set is delivered, not refused; a passive server
   whose `reply_recv` pops many dead senders is still receiving.
5. **Kernel: `destroy` of a forwarder**, with RFC-0005's destruction increment and RFC-0007's increment 14
   (`Revoked`); any other type answers `WrongType`, a forwarder another Pool paid for `InsufficientRights`.
   Boot-test: a client prints `[client] revoked` after the root destroys its forwarder.
6. **`abi`: the badge register live**, with RFC-0006's increment 12 for `IrqHandler`s: notifications ORing
   badges, `IrqHandler`s their bound badge and fault messages carrying them; a thread preempted mid-loop finds
   its `x9` intact.

## 16. What this unblocks

RFC-0007's verdicts 2, 7 and 9 can be decided without revocation open around a frozen ABI. The broker RFC
inherits a mechanism and three duties: forward every revocable grant, badge before giving, run §8's eviction.
Servers inherit one, for the scheme-registry and broker RFCs to make protocol: wrap what they return per client
in a forwarder of their own (§8 step 5). O-3 closes for grants made through forwarders once increments 3 to 5
land. RFC-0003b (draft) builds on this core: payer destruction of every other type, `pool_reclaim`, and with them
RFC-0005's O-7 row for reclaim.
