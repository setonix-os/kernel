<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0003b — Pool reclaim and payer destruction

| Field | Value |
|-------|-------|
| Status | **Draft** — 2026-11-06. Held for validation against an executable model of the reclaim walk, the loan rule and the Pool rules (§12), the next planned work, before it is reviewed again |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-11-06; split from RFC-0003a (drafted 2026-10-31) by the maintainer's decision |
| Affects | RFC-0003a §7, §8, §9, §13, §14, §16 (proposed); RFC-0005 §5, §7, §8, §10, §14, §17.2–17.3, RFC-0006 §5, §7, §11, §20 and RFC-0007 §4, §8 (proposed amendments) |
| Depends on | RFC-0003a (proposed), whose forwarders, badges and Pool `REVOKE` this extends; RFC-0005, RFC-0006 and RFC-0007 (proposed) |
| Discharges | would close RFC-0005's O-7 row for reclaim; maintains O-7; contributes to O-22 and O-26; answers RFC-0005 §17.2–17.3 |

> **Proposals**, for review once the model has run.
>
> 1. **Payer destruction:** `REVOKE` on the paying Pool destroys any object, each type at its own RFC's bound (§3).
> 2. **A lent context finishes its loan:** destroyed on loan, it goes dying, takes no refill and dies at rest (§4).
> 3. **Reclaim:** `pool_reclaim` destroys all a Pool paid for, wherever it went, preemptibly, then the Pool (§5).
> 4. **Pool rules:** balance crosses one funding edge; dying Pools refuse moves; Pools nest eight deep (§5).
> 5. **Time and lines:** `SchedControl` and `IrqControl` go to a resource server behind a root-held forwarder (§7).
> 6. **Amendments:** to RFC-0003a, RFC-0005, RFC-0006 and RFC-0007, all proposed (§8).

## 1. The question

**What may a payer destroy, and how does a funder take back everything a Pool paid for, wherever its
capabilities went, in bounded and preemptible work, without stranding the servers that depend on it?**
RFC-0003a decides revocation and leaves this half to a separate paper. It is the seam with RFC-0005's
accounting and RFC-0006's scheduling: most findings of RFC-0003a's last review round sat here, so the
maintainer split it out to be checked against an executable model before review (§12). Everything below was
drafted and reviewed as part of RFC-0003a; it moves here with its rejected alternatives intact.

## 2. Which pillar

**Pillar 2, and O-7.** "Cut this app off" ends authority under RFC-0003a; ending what the app paid for is the
other half. Until it exists, RFC-0005's O-7 row keeps its reclaim gap: memory a child moved to peers outlives
the child while any capability to it lives.

## 3. Payer destruction (RFC-0005 §17.3)

**Proposed: `REVOKE` on the Pool an object is charged to authorises destroying it, whatever its type.**
RFC-0003a §7 gives `destroy(pool, obj)` that right for forwarders; here it reaches every type. `obj`, presented
with no right checked, designates, and the Pool's right authorises: Shapiro's "The party who pays must always be
permitted to deallocate" ([msg00041](https://lists.gnu.org/archive/html/l4-hurd/2005-10/msg00041.html)), and
seL4 Untyped revoke's authority without its tree. Each type is torn down as its own RFC defines, at its bound:

| Type | `destroy` does | Bound |
|------|----------------|-------|
| Forwarder | RFC-0003a §7: advance the generation, refund across at most eight closed Pools (§5) | O(1); a cascade adds at most three (RFC-0003a §4) |
| Endpoint, notification | complete every waiter `Revoked`, then advance | O(waiters) ≤ thread capacity, a point between wakes |
| Region, AddressSpace | RFC-0005 §10's teardown; an AddressSpace also returns its tag | per mapping, preemptible (RFC-0005 §17.5) |
| Thread, reply object | RFC-0006 §4's suspend and unbind; a waiting caller completes `PeerGone` | O(1) |
| SchedContext | RFC-0006 §5's unbind, its threads left `Unfunded`; one lent to a passive thread finishes its loan first (§4) | O(1) |
| `IrqHandler`, deadline object | unbind; an `IrqHandler` also masks its line and frees it for a new mint, a deadline object leaves the release queue | O(1) |
| Process | `KILL`, then advance | per table slot, preemptible (RFC-0007 §8) |
| Pool | refused while it carries charges other than contexts finishing a loan, so `pool_reclaim` empties it first, then destroys it; return the balance to the funder, then advance; while a loan is unfinished, stay as RFC-0005 §5's forwarding slot | O(1) |

Destroying a started Process is `KILL` by its payer: whoever paid for a process supervises it to that extent.
RFC-0007 §8 gives `KILL` to `WRITE` on the Process; the payer's `REVOKE` reaches it as well (P7).

- **Every post-boot object has a payer.** RFC-0005 §5 charges every object slot, so RFC-0006 §11's
  `IrqControl` mint presents a Pool that pays for the `IrqHandler` (P5), and a driver's line becomes revocable
  like anything else made after boot; RFC-0006 §8's deadline objects are charged already. What RFC-0003a §9
  leaves revocable only by a payer is then revocable for every type, `IrqHandler`s included (P1).
- **The payer's `destroy` reaches whoever depends on the object.** Every sharer of a Region faults (RFC-0005
  §10), and a notification a server bound stops waking it. RFC-0005 §7's "closing the last capability does not
  pull memory from under a live mapping" stays true; destruction by the payer does pull it (P3).
- **Kept alongside:** destruction at the last live reference (RFC-0005 §5), and `KILL`.

**Rejected:** `REVOKE` on the object itself, which would let any holder of a peer-transferred copy destroy the
object for everyone (RFC-0005 §10's worry); a new `DESTROY` right that would mean what `REVOKE` should mean;
`WRITE` on the object, which would grow `WRITE` towards the catch-all right; and a kernel keep-alive holding a
Region while another payer's AddressSpace maps it, which would let any sharer pin its payer's memory and reopen
the reclaim gap. A server therefore depends only on objects it pays for (§11), with one exception, §4.

## 4. A lent context finishes its loan (RFC-0006 §5, §7)

RFC-0006 §7 promises that a caller's death never unfunds a server mid-request, and eviction is a caller's death
plus its payer's `destroy`. So **a SchedContext destroyed while a passive thread holds it by donation goes
*dying*** (RFC-0006 §3): no entry invokes it, it takes no further refill, and it leaves its Pool's charge list
with its charge still outstanding. It still moves along the call chain as any lent context does, and the
request in hand runs on budget already released, which is all a `min_budget` sized to the request needs
(RFC-0006 §7). The destruction completes when the context comes to rest:

- a reply returns it to its bound thread;
- its holder blocks in `recv` or `reply_recv` with its caller gone, as at a caller's death;
- its holder is destroyed;
- or its released budget runs out, where RFC-0006 §7's expiry return and timeout fault apply as to any context
  with no refill due.

Then its generation advances, any thread still on it is left `Unfunded` (RFC-0006 §5) and its payer is
refunded. A context not on loan dies at once.

**Rejected:** letting the payer's `destroy` unfund the server at once, as RFC-0006 §5 alone would. Any client
paying for its context from a Pool it can reclaim, which is any client that splits one, could then strand a
shared passive server mid-request at will, `min_budget` notwithstanding; and a passive server cannot pay for the
time it runs on, so the Region's answer in §3 has no analogue. A finished loan is not the Region keep-alive
rejected in §3: it lasts no longer than RFC-0006 §7 already lets a server keep a dead caller's context, spends
no budget released after the destroy, and holds one context per server thread.

## 5. Reclaim (RFC-0005 §17.2)

- **Reclaim across a transfer.** `pool_reclaim(pool)`, needing `REVOKE`, destroys every object charged to
  `pool` wherever its capabilities went, emptying each Pool it funded first, and ends by destroying `pool`
  itself. Each Pool threads its charges on an intrusive list, two index words per object slot and no heap; the
  walk descends into a funded Pool and climbs back by its funder index, needing no stack. A funder keeps the
  capability `pool_split` returns, carrying `REVOKE`, and hands the child a derived `READ | WRITE`, so the Pool
  never becomes RFC-0005 §5's unheld forwarding slot. Notice is a userspace protocol: request release, then
  reclaim (research/0002 Part 6); the victim sees `Revoked` and faults on torn-down mappings (RFC-0005 §10).
- **Balance stays within reach.** RFC-0005 §5 lets `WRITE` move balance anywhere, so a child could pass its
  whole budget to a Pool a colluder funds, beyond any walk from its own funder. Here `pool_move` and
  `pool_merge` cross one funding edge only, from a Pool to its funder or to a Pool it funds, one index compared
  (P2). Balance in a subtree can then leave it only towards the funder that put it there; moving it between
  unrelated subtrees takes `WRITE` on each Pool it leaves on the way through a common funder: the funder's act.
- **The walk under preemption.** It marks a Pool *dying* on entering it, before destroying anything in it; a
  dying Pool takes no new charge and refuses `pool_split`, `pool_move` and `pool_merge` out of it, so nothing
  refunded during the walk can be moved away. It takes a preemption point between objects besides each
  teardown's own, and the revoker's entry pays. **No cursor survives a point:** a restart begins again at
  `pool`'s list head and re-descends. Every destroyed object left its list and was refunded once, in the entry
  that destroyed it, or for a context on loan left at once and is refunded once, at rest; so a restart repeats
  nothing, and a concurrent `destroy`, a death at the last live reference or a second walk over the same Pools
  finds only what is still listed; a walk that finds `pool` already destroyed by another completes `Revoked`.
  **Pools nest at most eight deep** (P2), so the re-descent, like RFC-0005 §5's forwarding chain of closed
  Pools that every refund follows, is bounded by a constant; that bound also makes RFC-0003a's forwarder
  refund constant. If the walk reaches the revoker's own Thread or Process it destroys it and the call never
  returns; its own SchedContext leaves it `Unfunded` (RFC-0006 §5), to restart if funded again. Either way the
  Pools stay dying, and a later `pool_reclaim` finishes the work.
- **It ends while the victim still runs.** The walk enters every Pool below `pool` depth first, and a Pool once
  entered takes no charge, so its list only shrinks, and lets no balance out. What the victim does in a Pool
  not yet entered spends units those Pools already hold, a fixed total, since balance crosses only funding
  edges. Each entry completes at least one step, an object or one step of its teardown, before its first
  point, so the work is bounded by the subtree's units, plus at most eight levels of re-descent per entry. §6
  destroys the victim's Process first all the same, which stops the churn.
- **The walk ends by destroying `pool`**, its balance returned to its funder. It must: the revoker holds
  `REVOKE` on `pool` but not, by the convention above, on its funder, so it could not `destroy` `pool`
  through the funder; and RFC-0005 §5's closure waits for every capability to `pool` to close, the victim's
  own and any copy it passed on included. Every capability to `pool` then answers `Revoked`, so the victim's
  `READ | WRITE` spends nothing again. While a context on loan is still coming to rest, `pool` stays as
  RFC-0005 §5's forwarding slot and the context's refund forwards to the funder. The root Pool's balance
  goes to the kernel, as at its closure. A funder that funds again splits a fresh Pool. **RFC-0005's O-7 row
  would close for reclaim.**
- **Donation stays unproposed.** No user moves a charge: a loader creates a child's Regions from the child's
  Pool, and reclaim needs no `reissue`. When one appears, a charge move needs `REVOKE` on the old payer and
  `WRITE` on the new.

**Rejected:** reclaim that needs the payer to keep a capability to every object, one slot per object and lost
to any lapse; moving balance anywhere given `REVOKE` on its source, since `pool_split` hands that right to any
`WRITE` holder for its own sub-Pools; compressing RFC-0005 §5's forwarding chain when a Pool closes, which
repoints every slot forwarding to it, O(children), where a depth bound costs a comparison; and leaving a
reclaimed Pool for its funder to destroy, with revokers funding only from Pools whose `REVOKE` they hold, which
spends a level of nesting per revoker against the eight and rests recovery on a convention the kernel cannot
check.

## 6. Eviction's last step (RFC-0003a §8)

RFC-0003a §8 ends authority in five steps. This RFC adds a sixth (P1):

> **6. Reclaim.** If the broker funded the app, it first destroys the app's Process through the Pool that
> paid for it, which is `KILL` (§3) and stops the app's threads, then runs `pool_reclaim` on the app's Pool.
> That returns every object the app paid for, including those it moved to peers, and then the Pool's balance
> (§5); a context the app lent a server comes back when the server has finished with it (§4).

Step 5 gains the means to end what a server paid for under a client: a session endpoint or a shared Region the
server paid for it can now `destroy`, where RFC-0003a alone has it wrap or wait for the last reference.

## 7. Time and interrupt lines: a resource server (RFC-0003a §9)

**Proposed: time and interrupt lines go through a resource server.** Configuring an app's SchedContext needs a
`SchedControl`, and a driver's line needs the `IrqControl` to mint it; both are boot-minted, and neither is a
type a forwarder wraps (RFC-0003a §4). The root therefore hands them, under RFC-0003a §9's rule, to one process
inside its trust boundary, the *resource server*, which holds no other boot capability and serves the broker
alone, through an endpoint the root hands the broker inside a root-held forwarder. A context it configures was
made by `sc_create` from some Pool, and a line it mints is charged to the Pool presented at mint (P5), so the
root's `destroy` of that forwarder ends new requests, and `pool_reclaim` by whoever funds those Pools ends what
they produced. The resource server's own copies are irrevocable, so the reasoning behind the root's
precondition binds it too, in the only form a server can meet: one client, the broker, and a protocol narrow
enough to review whole. The root paid for its Process and can `KILL` it, which drops those copies unless it has
moved them on, as any recipient may (RFC-0003 §6); its compromise is the residual that nothing revokes.

**Rejected:** the root serving these requests itself, breaking RFC-0003a §9's precondition with every boot
capability at stake; handing both to the broker, as RFC-0006 §20's "the broker gets levers … `SchedControl`"
reads, which leaves a compromised broker's hold on time and lines outside O-22's switch; and forwarders over
`SchedControl` and `IrqControl`, which would widen RFC-0003a §4's one mechanism to method-bearing kernel
objects, each needing its own account of what revoking it reaches. The maintainer's call (§13).

## 8. Amendments proposed

Numbered apart from RFC-0003a's own P1–P6 and cited elsewhere as "RFC-0003b P*n*". All targets are proposed, so
each absorbs its change before its own verdict; each change is logged in `docs/CHANGELOG.md`.

| # | Target | Change | Why |
|---|--------|--------|-----|
| P1 | RFC-0003a §7, §8, §9, §13, §14, §16 | `destroy` reaches every type, not forwarders alone; §8 gains step 6 and step 5 its `destroy`; §9's post-boot objects, `IrqHandler`s included, become revocable by their payer, and `SchedControl` and `IrqControl` go to the resource server; §13's "eviction ends authority, not memory" gives way to this RFC's costs; §14's open question 4 is answered; §16's servers inherit the duty to depend only on what they pay for | §3–§7 |
| P2 | RFC-0005 §5, §8 | object slots gain charge-list links; `pool_reclaim` added, ending by destroying the Pool, its balance to the funder, as a forwarding slot while a loan is unfinished; `pool_move` and `pool_merge` cross one funding edge, to the funder or a Pool funded, `into` presented with `WRITE`, which §5 leaves unstated; a dying Pool refuses them and `pool_split`; `pool_split` refuses a ninth level of nesting, bounding §5's forwarding chain | §5 |
| P3 | RFC-0005 §7, §10, §14, §17.2–17.3 | destruction's right is Pool `REVOKE`; §7's "closing the last capability does not pull memory from under a live mapping" adds "its payer's `destroy` or `pool_reclaim` does, and a sharer faults (§10)"; donation stays unproposed; the O-7 row's reclaim gap closes | §3, §5 |
| P4 | RFC-0006 §5, §7 | §5's Lifecycle and §7's "only its capability holder may rebind or destroy it" gain: its payer destroys it, and one lent to a passive thread finishes its loan, dying, taking no refill and destroyed at rest | §4 |
| P5 | RFC-0006 §11, §20 | `IrqControl`'s mint presents a Pool that pays for the `IrqHandler`; §20's levers reach `SchedControl` through the resource server | §3, §7 |
| P6 | RFC-0007 §4 | Pool rows: `WRITE` gains `pool_split` (reply: the child with `DUPLICATE`, `TRANSFER`, `READ`, `WRITE` and `REVOKE`, and `EXECUTE` only if the invoked capability has it), `pool_move` (MR0 `into`, presented `WRITE`; MR1 the resource; MR2 the count) and `pool_merge` (MR0 `into`, presented `WRITE`); `REVOKE`'s `destroy` reaches every type, and `REVOKE` gains `pool_reclaim` | §3, §5 |
| P7 | RFC-0007 §8 | `KILL` is reached by `WRITE` on the Process or by its payer's `destroy` | §3 |

## 9. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-7 no unprivileged exhaustion | one object per `destroy`, each type at its own RFC's preemptible cost, a lent context at rest; `pool_reclaim` preemptible, restarting from the list head, ending within the subtree's units, paid by the revoker, destroying the Pool last; balance crosses one funding edge; Pools ≤ 8 | **maintained**; RFC-0005's reclaim gap closed, but for a lent context, which a server may hold as long as RFC-0006 §7 lets it hold a dead caller's |
| O-22 broker containment | the root wraps the resource server's endpoint too; a revoker holds `REVOKE` without spending (§5, §7) | **contributes** |
| O-26 bounded service per client | a lent context finishes its loan, so no client's payer unfunds a shared server mid-request (§4) | **contributes**; servers pay for what they map, so each bounds its own spending per client (§11) |

## 10. Graves checked (§3)

- **Baroque capability hierarchies (KeyKOS).** A Pool's funder chain is accounting, not a capability tree: at
  most eight deep, walked without a stack.
- **The catch-all right.** `REVOKE` means "deallocate what this Pool paid for", one object per `destroy` or one
  Pool's charges and then the Pool per `pool_reclaim`, and confers no use.
- **Policy in the kernel.** The kernel reclaims nothing unasked; whom to reclaim, and when, is the broker's.

## 11. Costs — what this makes harder

- **Memory:** two index words per object slot for the charge list.
- **Pools nest eight deep:** a ninth level of subdivision is refused, and with the root's, the broker's and an
  app's Pools taking three levels, an app's own process tree can subdivide five more.
- **A payer's `destroy` reaches whoever depends on what it paid for**, so a client paying for a Region or a
  notification can disrupt a server at will. A server therefore depends only on objects it pays for: it maps
  only its own memory, or copies from a client's Region with `region_copy`, which fails `Revoked` where a mapped
  access would fault. The one exception is the context a passive server is lent (§4).
- **A finished loan holds the evicted payer's slot:** a context destroyed on loan stays charged until it comes
  to rest, and a server that waits elsewhere without `recv` holds it as long as it would a dead caller's
  (RFC-0006 §7), one context per server thread, with its Pool kept as a forwarding slot meanwhile.
- **The payer can always reclaim**, objects and balance alike, so a child funded by its spawner is never
  sovereign over that memory; **balance crosses one funding edge at a time**, so a client cannot pay a server
  from its own Pool unless a holder of their common funder moves it, and a reclaimed Pool is destroyed.
- **Servers pay for per-client memory,** and no client can fund them directly, so a client that opens many
  sessions spends its server's Pool, not its own: each server bounds what it spends per badge (O-26), or the
  broker moves balance to it through their common funder.
- **A resource server holds `SchedControl` and `IrqControl` irrevocably** and parses the broker's requests, so
  its compromise reaches time and interrupt lines, revocable only by `KILL`, and not by that once it has moved
  its copies on (§7).

## 12. What the executable model must show

The maintainer held this RFC for a model before review, because each round's holes were interactions no reader
checked by hand. The model is the next planned work; it should exercise at least:

1. **Conservation.** RFC-0005 §5's `Σ balance(r) == units of r free` and "charged to exactly one Pool" hold
   after every step of `destroy`, `pool_reclaim`, `pool_move`, `pool_merge`, `pool_split` and every refund,
   forwarding slots included.
2. **The walk.** Interrupted at every preemption point and restarted from the head, it repeats no refund and
   ends at the same state, beside a concurrent `destroy`, a second walk and a victim still creating objects
   in Pools not yet entered; its work stays within the bound §5 states.
3. **Escape.** No sequence of splits, moves, merges and transfers takes balance out of a funder's subtree
   except towards that funder, and no dying Pool lets balance out.
4. **The loan.** For every way a context can come to rest (§4), including nested donation, `reply_recv`, a
   holder destroyed and an exhausted context with no refill, the destruction completes exactly once, refunds
   once and leaves no thread on a dead context.
5. **Recovery.** After `pool_reclaim`, every unit the Pool held is back with its funder, or in a forwarding
   slot that returns it once the last loan comes to rest.

## 13. Open questions

1. **The Pool depth bound**: eight, from root, broker and app with room to subdivide. A Pool bound is new to
   RFC-0005: is a constant the right price for constant-time refunds and reclaim restarts?
2. **Sovereignty under the payer rule** (RFC-0007 open question 3). No child is sovereign over memory: holding
   the only `REVOKE` capability to its own Pool does not help, since `pool_reclaim` on any Pool above it
   reaches it, and the root holds `REVOKE` on the root Pool, above everyone. What a child can check is
   narrower: that no one else holds a capability to its AddressSpace, the count RFC-0007 asks about. Is
   sovereignty against other holders, never against the funder chain, the property that check should promise?
3. **What a server may depend on that a client pays for**: §11 has a server depend only on objects it pays
   for, a protocol rule that keeps reclaim whole and costs servers memory per client. A kernel keep-alive for
   Regions (§3, rejected) would spare servers the rule at the price of letting any sharer pin its payer's
   memory. A lent context is the exception, finishing its loan (§4), because a passive server cannot pay for
   its time; the price is a slot a server may hold. Are those the trades the maintainer wants?
4. **Who holds `SchedControl` and `IrqControl` after boot** (§7): a resource server serving the broker alone
   (proposed), whose compromise nothing revokes; or the broker itself, as RFC-0006 §20 reads, leaving time and
   interrupt lines outside the root's switch; or forwarders widened to method-bearing kernel objects?

## 14. Implementation increments

After RFC-0003a's increments, and after the model (§12). Each is one reviewable PR.

1. **`memory`: charge lists and the reclaim walk**, with RFC-0005's Pool increment. Host tests: nested Pools
   emptied leaves first; a walk interrupted anywhere and restarted from the head repeats no refund and ends at
   the same result, beside a concurrent `destroy` and a second walk; a dying Pool refuses a split and every move
   out; a move off a funding edge refused; a ninth level refused; a reclaimed Pool is destroyed, its balance
   back with its funder and a copy of its capability passed on answering `Revoked`; a walk ends beside a
   victim that keeps creating objects in Pools not yet entered.
2. **Kernel: `destroy` of every type and `pool_reclaim`**, with RFC-0005's destruction increment. With
   RFC-0006's increment 8, a `scheduler` host test: a context destroyed on loan lets a passive server finish its
   request and is refunded at the server's next `recv`. Boot-test: the root reclaims a client's Pool and prints
   `[root] reclaimed` once a `READ` query of its own Pool shows the client's balance returned.

## 15. What this unblocks

Eviction that takes back memory as well as authority, and with it RFC-0005's O-7 row for reclaim, once the
model has run, this RFC is reviewed and its increments land. The broker RFC inherits one more duty, reclaiming
what it funded (§6), and servers one, depending only on what they pay for (§11).
