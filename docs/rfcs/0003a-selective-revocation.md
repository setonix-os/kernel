<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0003a — Selective revocation

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-10-31, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-10-31 |
| Affects | RFC-0003 §3, §5, §6, §7, §14.1, RFC-0004 §8, §10 and threat-model O-3 (dated amendments on acceptance); RFC-0005 §5, §7, §8, §10, §14, §17.2–17.3, RFC-0006 §4, §7, §11, A6 and RFC-0007 §3, §4, §7, §9, §16, §18 (proposed amendments); `capability/src/rights.rs`, `object.rs` |
| Depends on | RFC-0003 and its 2026-07-30 amendment; RFC-0004; RFC-0005, RFC-0006 and RFC-0007 (proposed), whose open questions it answers |
| Discharges | O-3 for grants made through forwarders (text amendment proposed, §10); maintains O-1, O-2, O-4, O-7, O-23, O-27; partly meets O-21; contributes to O-10, O-22; enables O-26; answers RFC-0003 §14.1, RFC-0005 §17.2–17.3, RFC-0007 §17.2 |

> **Proposed verdicts.**
>
> 1. **The fork:** option (a), revocable authority routed through kernel forwarders; no derivation tree, no parent link (§3).
> 2. **Forwarder:** wraps an endpoint, notification or forwarder; send side only; four deep; O(1) to destroy; a queued send must still reach its endpoint (§4).
> 3. **`TRANSFER` unchanged:** nothing unwraps a forwarder, so moved and derived copies stay revocable; a direct grant forgoes revocation (§5).
> 4. **Badges:** one word on endpoints, notifications and forwarders of either; set once at derive, never by a sender; ORed on signal, delivered on `recv` (§6).
> 5. **Destruction:** `REVOKE` on the Pool that paid, each type at its own RFC's bound; `REVOKE` is refused at mint on every other type (§7).
> 6. **Reclaim:** `pool_reclaim` destroys everything a Pool paid for, wherever it went, preemptibly; balance crosses one funding edge; donation stays unproposed (§7).
> 7. **Eviction:** the broker destroys an app's forwarders, tells its servers and reclaims its Pool; peer-transferred copies die too (§8).
> 8. **Root:** nothing revokes it, sound while it serves no client; what it makes after boot is revocable, boot-minted objects are not; O-22 gains a switch (§9).
> 9. **Amendments:** badge register and Pool methods in RFC-0007; forwarders and charge lists in RFC-0005; RFC-0003, RFC-0004, O-3 dated (§10).
> 10. **Demo:** needs nothing; the badge register is reserved in RFC-0007's first `abi` increment so the ABI never changes shape (§15).

## 1. The question

**How does a grantor withdraw authority from one holder, and from everyone that holder passed it to, without
destroying the object for everyone else and without the kernel doing unbounded work?** RFC-0003 §7 deferred
this; its 2026-07-30 amendment turned the deferral into a choice between **(a)** broker-interposed forwarders
with `TRANSFER` unable to bypass them and **(b)** a derivation tree with unbounded revoke. This RFC makes that
choice, and settles what hangs on it before RFC-0007 freezes the syscall ABI. *Who* is revoked and *when* is
the broker's policy; the kernel only makes revocation possible. RFC-0003 §14.1 planned this "with the broker
RFC as input"; the maintainer chose to decide first, so §14.1 is superseded by the duties the broker inherits (§16).

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
| Revoke latency | O(1) at revoke; dead waits cleared at the next rendezvous, bounded by the thread capacity and preemptible (§4) | O(descendants), made restartable by preemption points |
| O-7 exposure | bounded by the depth limit and the thread capacity; the grantor pays for its own forwarders | revoke work grows with descendants other processes created, in tables the revoker cannot see |
| Multi-core | under RFC-0006's big lock both are serialised; (a) holds it for one generation write and a bounded refund | the same lock, held for O(descendants) between preemption points |
| Review (§5.3) | one object type, one resolve loop, one refusal rule | every move, derive and process death maintains cross-table links |
| Precision | reaches only what a grantor routed through a forwarder | reaches everything derived, wherever it went |

**Verdict sought: (a).** In B-terms this is **B2 made selective**: a forwarder is an object whose only job is
to have a generation that can be bumped, so B2's epoch applies to one grant, not a whole object. B1 is
rejected. B3 splits: the badge stays, the parent link goes, and the crate's
`a_removed_parent_leaves_its_derived_sibling_untouched` stays true permanently. **The honest cost is the last
row:** (b) protects a grantor who forgot to interpose; (a) only one who did, a discipline the broker RFC owes.

## 4. What a forwarder is

- **An object** in RFC-0005's typed arrays, charged to the Pool that created it. It holds a generation-checked
  reference to what it wraps, its depth and its payer. The reference is counted when it names a forwarder and
  uncounted when it names an endpoint or notification, so a server's endpoint still dies with the server's last
  capability, its queued senders wake (RFC-0006 §4), and every forwarder over it answers `Revoked`.
- **Created by a Pool method**, `forwarder_create(pool, cap, rights)`, needing `WRITE` on the Pool. `cap` is
  presented, not moved, carries `DUPLICATE` (wrapping is a derivation) and names an endpoint, a notification,
  or a forwarder of depth under four. The reply is a forwarder capability with `rights ⊆ rights(cap)` and
  `cap`'s badge.
- **Use resolves; wrapping and moving do not.** `send`, `call`, a signal, and fault or timeout delivery resolve
  the chain to the leaf, checking each link's generation, and act with the invoked capability's rights and
  badge; a dead link answers `Revoked`. The loop takes at most the invoked forwarder's stored depth in hops and
  fails closed short of a leaf, so a cycle is stopped by that bound as well as by the generations.
  `forwarder_create`, `cap_derive`, `set_fault_ep`, `GRANT` and every transfer act on the forwarder itself:
  wrapping a depth-2 forwarder yields depth 3, never a forwarder of the leaf (§5). A forwarded `call` stays on
  RFC-0004 §5's fast path, one more slot read and compare per hop.
- **Send side only.** `recv`, `nb_recv`, `reply_recv`, `bind_notification` and an `IrqHandler`'s binding
  answer `WrongType` for a forwarder. The last two leave a link that works without the capability (RFC-0006
  §4, §11), which destroying a forwarder could not reach; the others serve no step of §8, and refusing them
  leaves no dead receiver to clear. A forwarder capability may rest in `fault_ep` or `timeout_ep` (A6).
- **Dead waits.** A thread blocked sending, through a forwarder or directly, keeps only the invoked handle, in
  its frame (RFC-0007 §5), and re-resolves it at rendezvous; a fault or timeout sender re-resolves its
  `fault_ep` or `timeout_ep` slot. **The leaf must be the endpoint the thread is queued on**, its `blocked_on`
  (RFC-0006 §4): whatever `write_regs` does to the frame, or `set_fault_ep` and `set_timeout_ep` to the slots,
  a queued message reaches only that endpoint, under the badge of a capability to it that the frame's handle or
  the slot names at rendezvous. A handle closed or moved meanwhile (the move is permitted) completes the wait
  `InvalidHandle`; a dead chain, or one now leading elsewhere, `Revoked`; a fault or timeout whose slot no
  longer leads there leaves its thread `Inactive`, as a fault with no handler does; and nothing is delivered. A
  sender that rewrites its handle to another of its own capabilities to the same endpoint sends under that
  one's badge, as it could by sending again. A rendezvous finding dead senders at the queue's head completes
  each and continues, with RFC-0006 §3's preemption point between pops: restartable, as nothing was delivered,
  bounded by the thread capacity and counted in the worst path, like RFC-0006 §6's queue insertion. seL4
  instead cancels badged sends eagerly at the revoker's cost (`cancelBadgedSends`, `src/object/endpoint.c`);
  here destruction stays O(1).
- **Not a cached resolution.** RFC-0003's 2026-07-30 amendment forbids caching a resolved capability outside
  the table, and `object.rs` acting on a reference after its resolve. A forwarder's link confers nothing alone:
  authority comes from the invoking capability, resolved and held across check→act, and every hop re-checks the
  link's generation inside that resolve, in one entry under RFC-0006's big lock. It is a fourth place a
  reference rests, after the table, a message and A6's thread slots, as §10 records.
- **Four deep at most.** "EROS imposes a constant bound on this depth in order to ensure that all kernel
  operations execute in bounded time", and deep chains were "VERY rare" in Shapiro's experience
  ([msg00040](https://lists.gnu.org/archive/html/l4-hurd/2005-10/msg00040.html)). Root, broker, app and helper
  fill the four levels (§9); measurement may revise it. Destroying a forwarder (§7) advances its generation,
  reuses its slot and refunds its payer, the refund crossing at most eight closed Pools (§7); one also dies at
  its last live reference (RFC-0005 §5), cascading at most four links, so destruction stays O(1).

**Rejected:** forwarders over other types: a Region or Pool reached through one creates objects and mappings
that answer to no forwarder, so revoking it would promise more than it delivers; memory is reclaimed by its
payer instead (§7). Also a kernel **membrane** wrapping every capability a forwarded message carries: an
allocation per message (O-7), and it would still miss what servers return, since a reply never crosses the
forwarder; a capability a server hands back is the server's to make revocable (§8 step 5). Also, for a queued
sender: snapshotting its badge and leaf into the TCB at block time *(seL4's `blockingIPCBadge`)*, a resolution
kept outside the table, which RFC-0003's amendment forbids, buying nothing the leaf check misses; and making
`write_regs` or `set_fault_ep` cancel the wait, a side effect RFC-0006 §4 does not give them, which would abort
the IPC of any thread a supervisor adjusts.

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
  the notification capability presented, or bit 0 (P3).
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

## 7. Destruction, reclaim and donation (RFC-0005 §17.2–17.3)

**Verdict sought: `REVOKE` on the Pool an object is charged to authorises destroying it.** `destroy(pool, obj)`
is invoked on the Pool; `obj`, presented with no right checked, designates, and the Pool's right authorises:
Shapiro's "The party who pays must always be permitted to deallocate" (msg00041), and seL4 Untyped revoke's
authority without its tree. Either reading of RFC-0006 open question 7 fits, so it stays open: the
designating capability is asked for nothing. Each type is torn down as its own RFC defines, at its bound:

| Type | `destroy` does | Bound |
|------|----------------|-------|
| Forwarder | advance the generation, refund across at most eight closed Pools | O(1); a cascade adds at most three (§4) |
| Endpoint, notification | complete every waiter `Revoked`, then advance | O(waiters) ≤ thread capacity, a point between wakes |
| Region, AddressSpace | RFC-0005 §10's teardown; an AddressSpace also returns its tag | per mapping, preemptible (RFC-0005 §17.5) |
| Thread, SchedContext, reply object | RFC-0006 §4 and §7's suspend and unbind; a waiting caller completes `PeerGone` | O(1) |
| `IrqHandler`, deadline object | unbind; an `IrqHandler` also masks its line and frees it for a new mint, a deadline object leaves the release queue | O(1) |
| Process | `KILL`, then advance | per table slot, preemptible (RFC-0007 §8) |
| Pool | refused while it carries charges, so `pool_reclaim` empties it first; return the balance to the funder, then advance | O(1) |

Destroying a started Process is `KILL` by its payer: whoever paid for a process supervises it to that extent.

- **`REVOKE` means one thing on one type.** It joins `WRITE` (spend) and `READ` (query) on Pools, is refused
  at mint on every other type, as `EXECUTE` is off-type (RFC-0005 §8), and Regions lose it. Splitting reclaim
  from spending lets a revoker hold no budget (O-22's least privilege). It replaces RFC-0003 §5's "revoke
  capabilities derived from this one", a relation the kernel no longer records.
- **Every post-boot object has a payer.** RFC-0005 §5 charges every object slot, so RFC-0006 §11's
  `IrqControl` mint presents a Pool that pays for the `IrqHandler` (P3), and a driver's line is revocable like
  anything else made after boot; RFC-0006 §8's deadline objects are charged already.
- **Reclaim across a transfer (§17.2).** `pool_reclaim(pool)`, needing `REVOKE`, destroys every object charged
  to `pool` wherever its capabilities went, emptying each Pool it funded first. Each Pool threads its charges
  on an intrusive list, two index words per object slot and no heap; the walk descends into a funded Pool and
  climbs back by its funder index, needing no stack. A funder keeps the capability `pool_split` returns,
  carrying `REVOKE`, and hands the child a derived `READ | WRITE`, so the Pool never becomes RFC-0005 §5's
  unheld forwarding slot. Notice is a userspace protocol: request release, then reclaim (research/0002 Part 6);
  the victim sees `Revoked` and faults on torn-down mappings (RFC-0005 §10).
- **Balance stays within reach.** RFC-0005 §5 lets `WRITE` move balance anywhere, so a child could pass its
  whole budget to a Pool a colluder funds, beyond any walk from its own funder. Here `pool_move` and
  `pool_merge` cross one funding edge only, from a Pool to its funder or to a Pool it funds, one index compared
  (P1). Balance in a subtree can then leave it only towards the funder that put it there; moving it between
  unrelated subtrees takes `WRITE` on each Pool it leaves on the way through a common funder: the funder's act.
- **The walk under preemption.** It marks a Pool *dying* on entering it, before destroying anything in it; a
  dying Pool takes no new charge and refuses `pool_split`, `pool_move` and `pool_merge` out of it, so nothing
  refunded during the walk can be moved away. It takes a preemption point between objects besides each
  teardown's own, and the revoker's entry pays. **No cursor survives a point:** a restart begins again at
  `pool`'s list head and re-descends. Every destroyed object left its list and was refunded once, in the entry
  that destroyed it, so a restart repeats nothing, and a concurrent `destroy`, a death at the last live
  reference or a second walk over the same Pools finds only what is still listed. **Pools nest at most eight
  deep** (P1), so the re-descent, like RFC-0005 §5's forwarding chain of closed Pools that every refund
  follows, is bounded by a constant. If the walk reaches the revoker's own Thread or Process it destroys it
  and the call never returns; its own SchedContext leaves it `Unfunded` (RFC-0006 §5), to restart if funded
  again. Either way the Pools stay dying, and a later `pool_reclaim` finishes the work.
- **After the walk** `pool` is empty and keeps its balance, and stays dying: the victim's `READ | WRITE` cannot
  spend it again, and `destroy` through `pool`'s funder returns the balance there (the table above). A funder
  that funds again splits a fresh Pool. **RFC-0005's O-7 row closes for reclaim.**
- **Donation stays unproposed.** No user moves a charge: a loader creates a child's Regions from the child's
  Pool, and reclaim needs no `reissue`. When one appears, a charge move needs `REVOKE` on the old payer and
  `WRITE` on the new. **Kept alongside:** destruction at the last live reference (RFC-0005 §5), and `KILL`.

**Rejected:** `REVOKE` on the object itself, which would let any holder of a peer-transferred copy destroy the
object for everyone (RFC-0005 §10's worry); a new `DESTROY` right that would mean what `REVOKE` should mean;
`WRITE` on the object, which would grow `WRITE` towards the catch-all right; reclaim that needs the payer
to keep a capability to every object, one slot per object and lost to any lapse; moving balance anywhere given
`REVOKE` on its source, since `pool_split` hands that right to any `WRITE` holder for its own sub-Pools;
compressing RFC-0005 §5's forwarding chain when a Pool closes, which repoints every slot forwarding to it,
O(children), where a depth bound costs a comparison; and a kernel keep-alive holding a Region while another
payer's AddressSpace maps it, which would let any sharer pin its payer's memory and reopen the reclaim gap. The
payer's `destroy` faults a Region's sharers instead (RFC-0005 §10), and a server maps only memory it pays for
(§13, §16).

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
   waits complete `Revoked` at the next rendezvous, and requests in service run to their reply.
5. **Tell the servers** (protocol, the broker RFC's): each drops the badge's state and `destroy`s what it paid
   for on that client's behalf, Regions and forwarders included. Capabilities a server returned to the client
   are direct, untouched by step 4, so a server that returns per-client authority (a session, a shared Region)
   wraps it in a forwarder of its own or ends it here (§16).
6. **Reclaim.** If the broker funded the app, `pool_reclaim` on the app's Pool returns every object the app
   paid for, including those it moved to peers (§7); `KILL` stops its threads.

A server's death needs no step: its endpoint dies with its last capability (§4). Eviction does not reach
authority minted without a forwarder, which is its minter's to revoke, what a server returned until step 5,
nor data already read.

## 9. Boot capabilities and the root (RFC-0007 §9)

**Nothing revokes the root, and that is correct while the root serves no one.** Revocation needs a revoker
with authority over the revoked, and the root has no superior; making the kernel that revoker is the "policy
in the kernel" grave. The precondition is that the root runs no client-facing code: a root that parses requests
from untrusted processes while holding every boot capability is a compromise nothing can undo. RFC-0007
§18's demo interim, the root doubling as server until that RFC's increment 15, breaks it for as long as it
lasts (P7). What the root creates after boot is charged to the root Pool, minted with `REVOKE`, so it is
revocable by forwarder or `pool_reclaim`, an `IrqHandler` included (§7). **Boot-minted objects are not:**
kept, never charged (RFC-0005 §5) and not forwardable, so the module and DTB Regions, the Console, each
`SchedControl` and the `IrqControl` are irrevocable once handed on, RFC-0006 §17's "exclusivity is checkable,
not revocable" made general. **The root's rule:** hand them on only inside its own trust boundary, as copies
without `DUPLICATE`, or copy module data into charged Regions first, and never to the broker. A move needs
`TRANSFER` (RFC-0003 §6), so a recipient can still move its one copy on; it cannot multiply it. **One
exception stands until RFC-0007's increment 13 deletes the Console:** the demo client's Console copy (RFC-0007
§11), outside the root's trust boundary, irrevocable short of `KILL`ing whoever holds it and bounded by what a
Console does, 24-byte writes to the operator's line. Of research/0002 Part 3's "ordinary, enumerable,
revocable" init capabilities, this meets the first two in full and the third for all made after boot.

**The root wraps what it gives the broker.** With each server endpoint handed to the broker inside a root-held
forwarder, one `destroy` by the root cuts off the broker's own use of those endpoints and every grant it made
through them, however far the copies moved. It does not reach capabilities servers returned through them, to
the broker or to its grantees, which the broker may have granted on directly: those end only when the servers
that returned them are told, as in §8 step 5. That gives O-22 a recovery switch, not its bound: the
broker's standing capability to every scheme endpoint is revocable but present, and the broker RFC must argue
the blast radius from that. With the root's rule it answers part of RFC-0007 open question 6, and it is why the
depth limit is four, not two.

## 10. Amendments proposed to other documents

Accepted documents gain dated amendments when this RFC is accepted; proposed ones absorb these before their own
verdict. Each change is logged in `docs/CHANGELOG.md`.

| # | Target | Change | Why |
|---|--------|--------|-----|
| R1 | RFC-0003 §3, §5, §6, §7, §14.1 | §3's capability gains a badge; §5's `REVOKE` becomes "destroy what this Pool paid for"; §6's derivation record is never built, and a capability may also rest in a forwarder's link; §7's choice is (a); §14.1 superseded (§1) | the deferral ends here |
| R2 | RFC-0003 2026-07-30 amendment; `object.rs` | the no-cache invariant and `ObjectRef`'s contract add: "a forwarder's link, re-checked on every hop inside the invoking resolve, is not a cached resolution" | §4 |
| R3 | RFC-0004 §8, §10 | a signal ORs the capability's badge, or bit 0 if unbadged; `recv` delivers the sender's badge | §6 |
| T1 | threat-model O-3 | "reaches capabilities transitively derived from the revoked one" becomes "reaches every capability derived, moved or chained from a grant made through a revocable indirection; a capability a server returns in answer is that server's to revoke" | (a) does not reach direct derivation (§3), nor what servers return (§4, §8) |
| P1 | RFC-0005 §5, §7, §8 | `Forwarder` joins the arrays; object slots gain charge-list links; Pool rights gain `REVOKE`, Region rights lose it; the root Pool and `pool_split`'s returned capability carry it; `pool_reclaim` added; `pool_move` and `pool_merge` cross one funding edge, to the funder or a Pool funded; a dying Pool refuses them and `pool_split`; `pool_split` refuses a ninth level of nesting, bounding §5's forwarding chain | §4, §7 |
| P2 | RFC-0005 §7, §10, §14, §17.2–17.3 | destruction's right is Pool `REVOKE`; §7's "closing the last capability does not pull memory from under a live mapping" adds "its payer's `destroy` or `pool_reclaim` does, and a sharer faults (§10)"; donation stays unproposed; the O-7 row's reclaim gap closes | §7 |
| P3 | RFC-0006 §4, §7, §11, A6 | a forwarder capability may rest in `fault_ep`/`timeout_ep`; fault and timeout messages carry its badge, replacing §7's "badges are reserved zero"; a queued sender's leaf must be its `blocked_on` at rendezvous; `bind` loses `bits` and ORs the presented notification capability's badge, or bit 0; `IrqControl`'s mint presents a Pool that pays for the `IrqHandler`; open question 7 untouched | §4, §6, §7 |
| P4 | RFC-0007 §3 | **badge (out): AArch64 `x9`, x86_64 `r14`**, written by `recv`, `nb_recv` and `reply_recv`, a fault or timeout message included, and zero on every other syscall's return; a return from an interrupt or exception, a resumption after a fault reply and a PC-rewind restart restore it from the frame, under §3's rule; `abi` marks it clobbered by every syscall | open question 2; neither register is in use or reserved by Rust inline assembly; `r14` is callee-saved under SysV, so a live value is saved around the call |
| P5 | RFC-0007 §4 | "A signal sets bit 0 … until badges exist" becomes "a signal ORs the capability's badge, or bit 0 if unbadged"; `cap_derive`'s MR1 goes live; an `IrqHandler` ORs its bound capability's badge, not its bound bits; `send`, `nb_send` and `call` accept a forwarder, the `recv` rows and `bind_notification` refuse one; Pool `WRITE` gains `forwarder_create`, Pool `REVOKE` `destroy` and `pool_reclaim` | §4, §6, §7 |
| P6 | RFC-0007 §7 | `Revoked`: "the invoked capability no longer reaches its object: one in its chain was destroyed or, for a queued sender, it now leads elsewhere; a completed wait included"; `PeerGone` narrows to "a party (caller, reply binding) went away" | one status per event, however indirect |
| P7 | RFC-0007 §9, §16, §18 | §9's "not revocable until RFC-0003a" becomes "what the root creates after boot is revocable; boot-minted objects are not (RFC-0003a §9)"; §16's root bullet drops the same clause; §16's "a forwarder for a method" becomes "a stand-in server", whose partial interposition is off the revocation path; §18's root-as-server interim gains a cost: the root's irrevocability assumes it serves no client | §9; one meaning per word |

**RFC-0007's verdicts 2, 7 and 9 need nothing further.** Presented handles gain only P5's refusals, the 20/44
handle word is untouched because forwarder generations are object generations held in kernel memory, and a
spawner that wants revocable grants simply `GRANT`s forwarder capabilities inside the window.

## 11. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-3 revocability | per-grant forwarders, O(1), reaching derived, moved and chained copies; payer destruction and reclaim (§4, §7) | **discharged for grants made through forwarders** under T1's text, not for what servers return through them, which they revoke (§8 step 5); under today's text, partial |
| O-7 no unprivileged exhaustion | forwarders charged to their creator; depth ≤ 4, Pools ≤ 8; dead waits cleared preemptibly within the thread capacity; one object per `destroy`, a forwarder in O(1), every other type at its own RFC's preemptible cost; `pool_reclaim` preemptible, restarting from the list head, paid by the revoker; balance crosses one funding edge | **maintained**; RFC-0005's reclaim gap closed |
| O-1, O-2, O-4 | forwarder rights ⊆ the wrapped capability's; no unwrap; badges never sender-set, not even by an `IrqHandler`'s binder; a queued send reaches only its own endpoint | **maintained** |
| O-10 no confused deputy | a request presents the app's own forwarder capability, whose badge the server sees | **contributes**; the broker RFC's |
| O-21 mediation auditability | the kernel delivers the grant's badge on every message (§6) | **partial:** the badge names the grant; delegation collapses actor identity unless the grant lacks `TRANSFER` |
| O-22 broker containment | the root wraps the broker's endpoints, reaching its grants but not what servers returned to it; a revoker holds `REVOKE` without spending (§7, §9) | **contributes:** the broker's standing authority is made revocable, not removed; the broker RFC argues the bound |
| O-23 fail-closed mediation | no per-use broker consultation (§8) | **maintained** |
| O-26 bounded service per client | badges give multiplexing servers a per-client key | **enables** |
| O-27 explicit authority at spawn | revocable grants are forwarder capabilities moved through the `GRANT` window | **maintained** |

## 12. Graves checked (§3)

- **Baroque capability hierarchies (KeyKOS).** No derivation tree or parent link; the table stays flat. The
  grave is radix capability storage (research/0001), not one bounded, single-type indirection. The grant tree
  lives in the broker's policy; a Pool's funder chain is accounting, at most eight deep, walked without a stack.
- **The catch-all right.** `REVOKE` means "deallocate what this Pool paid for", one object per `destroy` or one
  Pool's charges per `pool_reclaim`, and confers no use. Designation without a right confers nothing.
- **Policy in the kernel.** The kernel revokes nothing unasked; whom to cut off, and when, is the broker's.
- **Bolted-on multicore.** A revocation writes one generation under RFC-0006's big lock; resolve→check→act
  holds its resolution (`capability/src/object.rs`), now across at most four links. No global index.
- **Multi-copy IPC.** Unchanged: forwarders resolve, they do not relay.

## 13. Costs — what this makes harder

- **Revocability is a grantor's discipline.** A grant without a forwarder is irrevocable short of destroying
  the object, which is the price of rejecting (b).
- **Memory:** one word per capability for the badge, two per object slot for the charge list, and one slot
  per revocable grant, charged to the grantor; one slot read per hop on every forwarded call.
- **Revocable sub-delegation is four levels deep**; moves and derives are unlimited, deeper wrapping proxies.
  **Pools nest eight deep:** a ninth level of subdivision is refused.
- **Forwarders are send-side:** a notification granted through one cannot be bound to a thread or an IRQ line.
- **Boot-minted objects are irrevocable once handed on** (§9); the root's rule is all that guards them, and
  the root's own irrevocability is sound only while it serves no client, which the demo's root does not meet.
- **The next rendezvous clears dead waits:** a victim's `recv` pays for them, bounded and preemptible.
- **Eviction needs servers to cooperate** for state and memory they minted per client, and for every
  capability they returned to a client, which never crossed the forwarder (§8 step 5); **memory revokes by the
  Region**, not per sharer: a server shares one Region per client to evict one.
- **A payer's `destroy` faults every sharer of a Region** (RFC-0005 §10), so a client paying for a buffer can
  make a server that maps it fault at will. A server therefore maps only memory it pays for, or copies from a
  client's Region with `region_copy`, which fails `Revoked` where a mapped access would fault.
- **The payer can always reclaim**, objects and balance alike, so a child funded by its spawner is never
  sovereign over that memory; **balance crosses one funding edge at a time**, so a client cannot pay a server
  from its own Pool unless a holder of their common funder moves it, and a reclaimed Pool stays closed.
- **The badge register costs a clobber:** `abi` marks `x9` or `r14` clobbered by every syscall, so on x86_64
  a live `r14`, callee-saved under SysV, is saved around each.
- **A badge of 1 and an unbadged signal look alike**; grantors that need the difference avoid badge 1.
- **Requests in service finish**: revocation stops new invocations, not one already accepted.

## 14. Open questions

1. **The depth limits**: four for forwarders, from Shapiro's report and §9's chain, and eight for Pools, from
   root, broker and app with room to subdivide; both to revisit with measurement. A Pool bound is new to
   RFC-0005: is a constant the right price for constant-time refunds and reclaim restarts?
2. **Eviction notice to servers**: a broker protocol (proposed, as §8 assumes) or a kernel notification?
3. **Sovereignty under the payer rule** (RFC-0007 open question 3): a sovereign child holds the only `REVOKE`
   capability to its Pool, and the check should report payer as well as count.
4. **Eager or lazy clearing**: does measurement favour seL4's revoker-paid queue walk over §4's dead waits?
5. **Who pays for a shared buffer**: §13 has a server map only memory it pays for, a protocol rule that keeps
   reclaim whole and costs servers memory per client. A kernel keep-alive (§7, rejected) would spare servers
   the rule at the price of letting any sharer pin its payer's memory. Is that the trade the maintainer wants?

## 15. Implementation increments

None is demo work, **except that RFC-0007's increment 2 reserves the badge register**, so the ABI keeps one
shape. Each increment below is one reviewable PR.

1. **`capability`: the badge and `REVOKE`.** The set-once badge on `Capability`; `REVOKE` documented in
   `rights.rs`, refused at mint off Pools, dropped from Region rights; `object.rs` qualified (R2). Host tests:
   rebadging refused; badges inherited; the pinning test unchanged.
2. **`capability`: chained resolution** over a test-double forwarder. Host tests: depth five refused; a dead
   link fails closed; wrapping a forwarder gives depth + 1, never a forwarder of the leaf; cascades stop at four;
   links forced into a cycle stop at the stored depth.
3. **`memory`: charge lists and the reclaim walk**, with RFC-0005's Pool increment. Host tests: nested Pools
   emptied leaves first; a walk interrupted anywhere and restarted from the head repeats no refund and ends at
   the same result, beside a concurrent `destroy` and a second walk; a dying Pool refuses a split and every move
   out; a move off a funding edge refused; a ninth level refused; a reclaimed Pool charges nothing more.
4. **Kernel: `Forwarder` and `forwarder_create`**, with RFC-0007's increment 10: forwarded calls, dead waits
   cleared between preemption points, the send-side refusals; a queued sender whose frame is rewritten to
   another endpoint's handle completes `Revoked` and delivers nothing.
5. **Kernel: `destroy` and `pool_reclaim`**, with RFC-0005's destruction increment and RFC-0007's increment 14
   (`Revoked`). Boot-test: a client prints `[client] revoked` after the root destroys its forwarder.
6. **`abi`: the badge register live**, with notifications ORing badges, `IrqHandler`s their bound badge and
   fault messages carrying them; a thread preempted mid-loop finds its `x9` intact.

## 16. What this unblocks

RFC-0007's verdicts 2, 7 and 9 can be decided without revocation open around a frozen ABI. The broker RFC
inherits a mechanism and three duties: forward every revocable grant, badge before giving, run §8's eviction.
Servers inherit two, for the scheme-registry and broker RFCs to make protocol: make what they return per client
revocable, by a forwarder of their own or by ending it when told (§8 step 5), and map only memory they pay
for (§13). O-3 closes for grants made through forwarders, and RFC-0005's O-7 row for reclaim, once increments
4 and 5 land.
