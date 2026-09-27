<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0006 — Threads and scheduling

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 ("IPC is the product", priority inheritance, bolted-on multicore); the HAL (`kernel/src/arch/**`), `boot.s`'s park loop and `aarch64.ld`'s stack plan; RFC-0003 §14.2 (a dated note); RFC-0004 §4, §8, §9.2, §9.3 and its 2026-08-01 amendment (three proposed amendments, §13); research/0002 Part 6 (one correction); RFC-0005 §13 step 5 and §18.2; `CLAUDE.md` § Architectures (FP enabled per thread) and § Layout (gains `scheduler/`); every userspace driver |
| Depends on | RFC-0003 (capability table); RFC-0004 (IPC) and its 2026-08-01 amendment; RFC-0005 (proposed) for object memory, kernel stacks, discovery and address-space switching; RFC-0007 (proposed) for the trap path, the syscall encoding, the Process object and the root task |
| Discharges | O-7 for CPU time and the scheduler's own structures; O-4 for time; both commitments of RFC-0004's 2026-08-01 amendment **as amended by verdict 12**; answers RFC-0004 §9.2 in shape; contributes to O-5, O-9, O-17 |

> **Proposed verdicts.**
>
> 1. **Event kernel:** one kernel stack per core; the kernel is not preemptible but its long paths take
>    preemption points; every wait is a thread state (§3).
> 2. **Thread:** an RFC-0003 object holding its user frame; ten states, one of them `Unfunded`; born at
>    priority 0 with ceiling 0; its fault and timeout endpoints held in the thread, not its process (§4).
> 3. **Time is a capability:** a `SchedContext` (budget and period in nanoseconds, at most eight refills),
>    configured only through a per-core `SchedControl` whose one method sells time on that core (§5).
> 4. **Budgets are caps, not minima:** the kernel enforces the cap; a guaranteed minimum is admission
>    policy — a stated departure from research/0002 (§5).
> 5. **Priorities:** 256 fixed levels; any capability to a thread lends its priority ceiling (seL4's MCP);
>    no scheduling right; O(1) bitmap run queues (§6).
> 6. **Endpoint queues are priority-ordered,** first-come within a level, as seL4 MCS (§6).
> 7. **Donation:** `call` lends time to a *passive* server, which runs at the higher of its own and its
>    caller's priority; a caller's death never takes time from a server mid-request (§7).
> 8. **Budget guard:** an endpoint's `min_budget` is checked at `call`; a caller short *now* waits; expiry
>    inside a server hands time back one hop with `BudgetExpired` and a timeout fault (§7).
> 9. **Timer:** tickless per-core deadlines — the AArch64 virtual timer; the x86_64 LAPIC, TSC-deadline
>    only with an invariant TSC; userspace timers will be deadline objects on the same queue (§8).
> 10. **FP/SIMD:** off by default, enabled per thread, saved lazily; the kernel's only FP instructions
>     are its save and restore stubs (§10).
> 11. **Interrupts and cores:** GICv3 first, LAPIC and I/O APIC; a device line signals a notification and
>     stays masked until `ack`; a big lock whose waiters serve IPIs; cores started by PSCI or INIT-SIPI
>     when first funded (§11, §12).
> 12. **Amendments proposed:** three to RFC-0004 and its 2026-08-01 amendment, one note on RFC-0003
>     §14.2, one correction to research/0002 (§13).

## 1. The question

**Who runs next, on which core, for how long, and on whose authority — such that no thread runs on time
it was not granted, a server never runs on time nobody paid for, and the kernel never waits on anything
a process controls?**

That is O-7's scheduler half plus RFC-0004's three scheduling hazards: priority-aware direct switch,
donation, and budget expiry inside a server. This RFC designs the mechanism. It chooses no priorities,
admits no workloads and balances no load; that is for whoever holds the capabilities defined here.

## 2. Which pillar

**The kernel doctrine (§3) directly, and pillar 2 by consequence.** "IPC is the product" holds only if a
`call` hands over the CPU and the budget without a scheduler round trip (RFC-0004). Time is the one
resource every thread consumes; if it is not a capability, pillar 2 has a hole exactly where
availability (A5) lives. seL4 MCS made time a capability, and this RFC adopts that shape. Borrow Ledger:
scheduler, timer and interrupt-controller code are the **microkernel core — write ourselves**; Redox's
drivers are read for reference only, since the kernel masks and signals but never services a device.

## 3. The execution model — one kernel stack per core (verdict 1)

- **Option A — process kernel** (a kernel stack per thread: Linux, Zircon, original L4). **Rejected:** a
  guarded stack per thread in a kernel with no heap, and blocking in the kernel is how "hold a lock across
  a wait a process controls" (O-7) becomes possible.
- **Option B — event kernel** (one stack per core: seL4, OKL4, NOVA; Fluke's "interrupt model").
  **Verdict sought.** The kernel is entered by exception, interrupt or syscall, runs to completion with
  interrupts masked, and leaves by restoring *some* thread's user frame. A syscall that must wait records
  a state and the object waited on; on wake the result is written into the waiter's frame, or the call
  restarts by rewinding the saved PC one instruction (4 bytes for `svc #0`, 2 for `syscall`). *(Heiser &
  Elphinstone, TOCS 2016; Ford et al., "Interface and Execution Models in the Fluke Kernel", OSDI 1999.)*

**The cost:** every kernel path must be bounded. Long ones — object destruction, RFC-0005 (proposed)'s
`reissue` and teardown, RFC-0007 (proposed)'s console write — take **preemption points**: on a pending
interrupt (`ISR_EL1.I`; the LAPIC IRR) they leave the operation restartable and exit. **A restarted
operation re-resolves every handle and re-checks every right from scratch**, and an object mid-teardown
carries a *dying* mark so no other entry invokes it between the exit and the restart. The worst path
between preemption points is measured and published: it is the interrupt latency. This **reverses
`aarch64.ld`'s plan** that the boot stack "will be replaced by per-thread stacks": it becomes core 0's
kernel stack, moved into RFC-0005 (proposed)'s guarded window (increment 5).

**Idle is a kernel loop, not a thread,** billing nobody. It runs `wfi` with `PSTATE.I` set — a pending
interrupt still wakes it — then unmasks for one instruction; Linux warns that masking at `ICC_PMR_EL1`
would *not* wake the core (`arch/arm64/kernel/idle.c`), so this kernel masks with DAIF only. x86_64 uses
`sti; hlt`, whose STI shadow issues the `hlt` before delivery. **An interrupt taken in idle abandons it:**
the current-EL IRQ stub (this RFC's; RFC-0007 owns the lower-EL and `syscall` entries) saves nothing,
resets `SP` to the per-core stack top from the `TPIDR_EL1` block (`TSS.RSP0`'s twin on x86_64) and enters
the dispatcher; idle is never resumed. Idle is the only place the kernel unmasks, and it drops the big
lock (§12) before waiting.

## 4. The Thread object (verdict 2)

A **Thread** is an RFC-0003 object belonging to one Process, which RFC-0007 (proposed) provides. Its TCB
is a slot in RFC-0005 (proposed)'s typed array for threads, charged to the Pool presented at creation.

| Field | Meaning |
|-------|---------|
| `frame` | the full user register file, written at every kernel entry (§9) |
| `state`, `blocked_on`, links | a state below; the endpoint, reply object or notification waited on; intrusive queue links |
| `priority`, `mcp` | base priority and maximum controlled priority (§6); both **0 at creation** |
| `home_sc`, `sc`, `effective` | the bound context, if any; the context held now, own or donated; the priority run at now (§7) |
| `bound_notification`, `ipc_buffer` | the notification a `recv` may also wake on (RFC-0004 §10); the IPC-buffer binding (RFC-0007, proposed) |
| `fault_ep`, `timeout_ep` | **two kernel-held capability slots in the TCB** *(seL4 MCS)*: the capability is moved in by its binder, generation-checked when a fault is raised, and fails closed if its endpoint died; the thread cannot close it |
| `fp_enabled`, `fp_state` | the FP/SIMD flag and save area (§10) |

**Every internal link is safe against slot reuse.** A queue link, `sc`, `home_sc`, a reply object's
record of a lent context, an `IrqHandler`'s notification and a core's FP owner are each either a counted
reference that holds the slot (RFC-0005 §5) or an `(index, generation)` pair checked on use — never a bare
index. This closes the ABA holes a slot array otherwise opens (§10's FP owner is the sharp one).

| State | Meaning | Left by |
|-------|---------|---------|
| `Inactive` | created, suspended, or faulted with no handler; in no queue | `resume` |
| `Ready` | runnable with released budget; in its core's run queue | dispatch; `suspend`; context removed (→ `Unfunded`) |
| `Running` | the current thread on its core; in no queue | preemption, timeslice end, budget exhaustion (→ `Throttled`), block, `yield`, fault, `suspend`, `thread_exit`, context removed |
| `Throttled` | runnable, holding a context whose released budget is exhausted or below what its pending `call` needs; in the release queue | refill release; `suspend`; context removed |
| `Unfunded` | runnable but holding no context — a passive thread woken by a non-donating `send` or `notify`, one `resume`d with none, or a server left stranded (§7); in no queue | `bind_sc`, a donation, `suspend` |
| `BlockedOnSend` | queued on an endpoint to `send` or `call` | rendezvous, endpoint destruction, `suspend` |
| `BlockedOnRecv` | waiting on an endpoint (and any bound notification) | rendezvous, signal, `suspend` |
| `BlockedOnReply` | waiting on a reply object after `call` or a fault message | reply, error completion (§7) |
| `BlockedOnNotification` | waiting on a notification alone | signal, `suspend` |
| `Exited` | finished; in no queue; the object lingers until destroyed | never |

Faults reuse IPC: the kernel `call`s `fault_ep` on the thread's behalf, the thread waits `BlockedOnReply`,
and RFC-0007 (proposed) defines the message. Fault and timeout messages bypass `min_budget`. Destroying a
thread bumps its generation (RFC-0003 §7), unlinks it in O(1) from whichever doubly linked queue holds it,
and destroys any reply object it waits on (RFC-0004 §8) — with §7's rule for a context it had lent.

Every operation is a method `call` on the **invoked** capability (RFC-0007 (proposed) verdict 1). Some
also name a second, **presented** capability, which is resolved in the caller's table and checked but
never moved and needs no `TRANSFER` — RFC-0007 (proposed) provides that encoding (seL4's `extraCaps`).

| Operation | Invoked: right | Presented: right | Effect |
|-----------|----------------|------------------|--------|
| `set_priority(t, auth, p)` / `set_mcp(t, auth, m)` | `t`: `WRITE` | `auth`: any capability to a Thread, no right checked | value `≤ mcp(auth)` (§6) |
| `bind_sc(t, sc)` / `unbind_sc(t)` | `t`: `WRITE` | `sc`: `WRITE` / — | active ↔ passive |
| `bind_notification(t, n)` / unbind | `t`: `WRITE` | `n`: `READ` | a `recv` also wakes on `n` |
| `set_fault_ep(t, ep)` / `set_timeout_ep(t, ep)` | `t`: `WRITE` | `ep`: `WRITE`, **moved** into the TCB slot | the caller derives first to keep a copy |
| `resume(t)` / `suspend(t)` | `t`: `WRITE` | — | `Inactive` ↔ runnable; a first `resume` closes the Process's grant window (RFC-0007) |
| `read_regs(t)` / `write_regs(t, …)` | `t`: `READ` / `WRITE` | — | frame and `TPIDR_EL0`/`FS_BASE` only, sanitised (§9) |
| `set_fp(t, on)` | `t`: `WRITE` | — | §10 |
| `sc_configure(ctl, sc, B, T, refills, 0)` | `ctl`: `WRITE` | `sc`: `WRITE` | §5; the last word is the placement class, reserved zero (§12) |
| `sc_stats(sc)` / `ctl_info(ctl)` | `sc` / `ctl`: `READ` | — | §5's counters / §5's exclusivity check |
| `yield()` / `thread_exit()` | none — two of RFC-0003 §9's three authority-free exceptions | — | tail of own level / `Exited` |

No right joins RFC-0003's set. **The supervision cost:** a process holding `WRITE` on its own thread can
rebind that thread's `fault_ep`. A supervisor keeps the binding by handing the child no `WRITE` on threads
it created for it; threads the child creates itself are supervised through the Process (RFC-0007 open
question 1). `thread_exit` takes one `WRITE` on self for a TLS runtime to set `FS_BASE` via `write_regs`.

## 5. Scheduling contexts and `SchedControl` — time as a capability (verdicts 3, 4)

**Options.** A timeslice on the thread (classic L4, Zircon) makes creating a thread *acquiring CPU*, with
nothing to grant or withhold — **rejected** on O-4. Partition budgets (QNX adaptive partitioning) are
**subsumed**: a partition is what a `SchedControl` holder builds from several contexts. **Verdict sought:**
contexts as objects *(seL4 MCS: Lyons, McLeod, Almatary & Heiser, "Scheduling-context capabilities",
EuroSys 2018)*.

A **SchedContext** holds budget `B` and period `T` (`B ≤ T`), at most eight refills, its core and its
bound thread. A thread with a bound context is **active**; one without is **passive** and runs only on
donated time (§7). **The ABI unit is the nanosecond** (`u64`) for `B`, `T` and `min_budget`, converted to
counter ticks once, in `sc_configure` and at endpoint creation — budgets round down, periods up — so no
per-machine frequency (62.5 MHz on QEMU's `cortex-a72`, a TSC rate on x86_64) enters the ABI.

- **`SchedControl`, one per core, minted to the root task** (RFC-0007 (proposed) lists them in BootInfo),
  has exactly two methods. `sc_configure` (`WRITE`) binds a context to that core with its parameters —
  moving a context between cores is `sc_configure` through another core's `SchedControl`, and the first
  context configured onto a parked core starts it (§12). `ctl_info` (`READ`) returns the number of contexts
  configured on the core and the object's reference count, so a recipient promised exclusivity can check
  it (§12). Topology is BootInfo *data* from RFC-0005 (proposed)'s discovery, not a method; interrupt
  routing is not a `SchedControl` method (§11). No syscall names a core by number. *(seL4 `SchedControl`;
  research/0002's "core-set capability".)*
- **`MIN_BUDGET`:** `B` below twice the published worst-case kernel path is refused — seL4 MCS's rule,
  `2u * getKernelWcetTicks() * CONFIG_KERNEL_WCET_SCALE` (`include/kernel/sporadic.h`), without the scale.
  The path is measured *between preemption points*, which is why RFC-0007 (proposed)'s console write must
  take them (§11); a 44 ms console hold would otherwise make `MIN_BUDGET` 88 ms.
- **Full contexts** (`B = T`) are never throttled and round-robin with timeslice `T`. **Partial contexts**
  (`B < T`) are sporadic servers: within any window of length `T` the context consumes at most `B`. Refills
  fall due a period after consumption; when all eight are in use they merge, forfeiting bandwidth rather
  than exceeding it. *(Sprunt, Sha & Lehoczky 1989.)*
- **What a budget means — caps, not minima (verdict 4).** research/0002 Part 6 asks for guaranteed
  reservations with capping as broker policy. This kernel enforces the **cap**, because a sporadic cap is
  what makes fixed-priority admission sound: a holder can prove a minimum only if every higher-priority
  context is capped. The **minimum** follows from admission, the `SchedControl` holder's policy. Dispatch
  is work-conserving *only among threads with released budget*: a partial context idles its core by
  design when its budget is spent. Unused budget never carries past its window; there is no burst.
- **Accounting window and billing granularity**, stated as design (the QNX caveat, ECRTS '21): the window
  is each context's own sliding period; billing is exact, from the counter (`CNTVCT_EL0`; the TSC) read at
  every entry and exit — 16 ns on QEMU's `cortex-a72`, which keeps the pre-Armv8.6 62.5 MHz default (QEMU
  `target/arm/tcg/cpu64.c`, `target/arm/cpu.c`). Kernel time, **interrupts included, is billed to the
  context running on entry** — a cross-domain leak costed in §16 *(as seL4)*.
- **Stall counters from the first version** (research/0002 Part 6, after PSI and oomd): per context, time
  consumed, time and count throttled, time ready but waiting, and budget expiries, read-only via
  `sc_stats` — the O-17 supervisor's wedged-server signal. The kernel measures; userspace acts.
- **Lifecycle.** Destroying or reconfiguring a context (a) preempts the thread running on it — an IPI on
  SMP; (b) clears every link to it — the thread's `sc` or `home_sc`, a reply object's record, its
  release-queue entry — before its slot's count can reach zero; and (c) leaves affected threads
  `Unfunded`. A reply to a reply object whose recorded context has gone moves nothing.
- **Accounting domain — decided: separate for Phase 1.** A context's *memory* is a slot charged to a Pool
  (RFC-0005); its *time* comes only from a `SchedControl`. One domain holding both, as research/0002
  wants, is deferred to the broker RFC; the cost is a broker reasoning over two ledgers. This answers
  RFC-0005 (proposed) open question 2.

## 6. Priorities and dispatch (verdicts 5, 6)

**256 levels**, 255 most urgent, meaningful within a core. `set_priority(t, auth, p)` needs `WRITE` on
`t`, any capability to the Thread `auth`, and `p ≤ mcp(auth)`; MCP is set the same way, so authority over
urgency only narrows as it is delegated — O-2's shape *(seL4 `TCB_SetPriority`, whose authority is any
TCB capability)*. **Every thread is born at priority 0 and MCP 0**, so a fresh thread is no authority;
the root task's thread starts at priority 255 and MCP 255 with a full context on core 0, the one
bootstrap act (RFC-0007 (proposed) §9). To delegate priority authority, derive a **rightless** capability
to a thread with the MCP wanted — it confers the ceiling and nothing else.

**Rejected: `READ` or `WRITE` on `auth`** — each would make priority authority and register access
(`read_regs`, `write_regs`) the same token. **Rejected: a `SCHEDULE` or `PRIORITY` right** — the catch-all
right in embryo, one bit whose meaning grows with every scheduling operation (the CAP_SYS_ADMIN grave).
**Rejected: dynamic priorities** (CFS, BSD decay, boosts): kernel-resident policies churned while
accounting and enforcement endured (research/0002 Part 1). **The cost:** any capability to a high-MCP
thread is priority authority, so such threads' capabilities must be handed out with care.

- **Run queues.** Per core, 256 intrusive FIFO lists and a 256-bit bitmap: the next thread is a
  count-leading-zeros over four `u64` words — O(1). A preempted thread re-enters at the head of its level;
  one whose timeslice ended, at the tail. The running thread is never queued, and a thread switched to
  directly is never enqueued at all. *(Elphinstone & Heiser, "From L3 to seL4", SOSP 2013.)*
- **Release queue.** Per core, sorted by next refill: O(n) in contexts only the `SchedControl` holder adds.
- **Priority-aware direct switch** (RFC-0004 §4). When *C* wakes *W* by IPC on the same core and *W* has
  released budget: if *C* blocks (`call`, `recv`, `reply_recv`), switch to *W* when its effective priority
  is at least the highest ready; if *C* stays runnable (`send`, `reply`, `notify`), only when *W* is also
  strictly more urgent than *C*. Otherwise *W* is queued. A *W* without released budget goes to the
  release queue; one on another core is enqueued there and sent an IPI (§12).
- **Endpoint queues are priority-ordered, first-come within a level** — seL4 MCS's `tcbAppend`
  (`include/object/tcb.h`, "priority ordered endpoint or notification queue"), which walks back from the
  tail; the FIFO `tcbEPAppend` is compiled only without MCS. The walk is O(n) with interrupts masked, *n*
  bounded by the thread array's fixed capacity (RFC-0005 §5) and counted in the published worst path; a
  `set_priority` on a queued thread repositions it. **Rejected: FIFO** *(classic L4, non-MCS seL4)*: O(1),
  but a process could create threads from its own Pool and queue *N* calls ahead of a more urgent client
  of a shared server — unbounded inversion the server cannot reorder.

## 7. Donation, inheritance and budget expiry (verdicts 7, 8)

**Donation.** When *C* `call`s and the receiver *S* is **passive**, the kernel moves *C*'s current
context to *S*, recorded in the single-use reply object *R* (RFC-0004 §8); `reply` or `reply_recv` moves
it back. *S* runs at `max(prio(S), eff(C))`, **fixed when the donation is made** and not propagated if
*C*'s priority later changes — O(1), one hop at a time. Lending priority is safe because time is lent with
it. *(Ford & Lepreau, migrating threads, 1994; QNX priority inheritance; seL4 MCS passive servers.)*

**An active receiver receives no donation (amendment A1, §13).** It runs on its own context at its own
priority, as in seL4 MCS (`reply_push`, `src/object/reply.c`, donates only to a receiver with no context):
a thread cannot hold two contexts, and an active server has chosen to pay for itself. RFC-0004 §4 and §8
say `call` lends unconditionally, so this is an amendment. **The cost:** an active server gets no
inheritance, and a low-priority active server blocks high-priority clients. Constitution §3's
inheritance therefore holds for passive servers; **a server shared across trust domains must run passive,
or active at a priority no lower than its most urgent client** — the ceiling, now required, not optional.

**Priority inversion, bounded.** Priority-ordered queues serve the most urgent waiter next; a waiter
still waits for the request in service, which runs on its client's context and can be preempted by a
middle-priority thread. `prio(S)` is therefore a **ceiling floor**: set to the highest client priority,
it bounds inversion to one request *(the priority-ceiling bound seL4 MCS relies on)*. **Rejected:**
boosting a busy server to a waiting sender's priority (QNX), where the kernel would guess which thread
serves an endpoint — policy — and walk unbounded chains with interrupts masked.

**Caller death never unfunds a server.** If *C* exits or is killed while *S* serves it, *R* is destroyed
but the context stays with *S* until *S* next blocks in `recv` or `reply_recv` (its `reply` fails
`PeerGone`); the context then detaches, `Unfunded` for no thread, and only its capability holder may
rebind or destroy it. Only a context's holder can pull time from a server, never a client's
`thread_exit` *(seL4 `reply_remove_tcb` likewise leaves the context with the server)*.

The 2026-08-01 amendment, discharged as amended, after seL4 RFC-14 (Mitchell Johnston, "Budget limit
thresholds on endpoints for SC Donation", seL4/rfcs pull request 24, open):

- **Prevention — `min_budget`.** An endpoint carries `min_budget`, fixed at creation (0 means none;
  RFC-0007 (proposed) encodes it). A `call` needs released budget of at least `min_budget + MIN_BUDGET`,
  the margin paying for the `call` and `reply` themselves — **this RFC's rule**; RFC-14's exact comparison
  is to be quoted before acceptance. A context whose `B` can never meet it fails `BudgetRefused`; a
  non-donating `send` to such an endpoint is invalid. With `min_budget` at the server's WCET, expiry
  inside it is a true error.
- **A caller short *now* waits (amendment A2).** The amendment says donation below the threshold is
  "refused up front". Proposed instead: a partial context that cannot pass now has its refills merged
  and deferred, waits `Throttled`, and the call restarts when the head refill suffices — free in an event
  kernel, O(refills) ≤ 8, and the deferral the RFC-14 discussion describes. A **full** context short of
  its slice ends its timeslice: tail of its level, a fresh slice, the call restarts. No donation happens
  below the bar, so the amendment's purpose holds; refusing a transient shortfall would make every client
  spin to retry.
- **Recovery — one layer down.** If a donated context is exhausted with no refill due while passive *S*
  runs on it, the kernel returns it to *S*'s immediate caller *C*, whose wait on *R* completes with
  `BudgetExpired` (observed when the context next refills); *R* is consumed, so a late `reply` fails
  closed. *S* is left `Unfunded`, registers intact, and the expiry counter rises. If `timeout_ep` holds a
  live endpoint, the kernel sends a **timeout fault** carrying consumed time and donating nothing (seL4
  MCS sends timeout faults without donation); the handler, active with time of its own, abandons the
  request (`write_regs`) or lends *S* a context to finish. The message carries **no identity**: badges
  are reserved zero until RFC-0003a, so a handler binds one timeout endpoint per server.
- **Why one layer.** In A → B → C, expiry in C returns time to B, which gets `BudgetExpired` and can reply
  to A with an error; unwinding to A would walk the reply stack — O(n) with interrupts masked.
- **With `min_budget` 0 there is no prevention:** one client can strand a shared server for every client
  until a handler funds it (O-26, A5). Zero is for servers with one client or a supervisor.

**Rejected: the unwind without timeout faults**, where the stranded server is visible only through its
state and counters; a supervisor woken by a message beats one polling. **Deferred: RFC-14's budget
limits** (capping what a server spends of a donated context): the RFC-14 discussion reports 22% extra
fastpath cost; the per-operation figures are to be re-checked before this is revisited (Open question 3).

## 8. The timer and preemption (verdict 9)

One HAL surface: `now() -> Ticks`, `frequency() -> Hz`, `set_deadline(Ticks)`, `cancel()`, and a
`timer_fired()` up-call. The kernel is **tickless**: each exit programs one per-core deadline — the
earliest of a partial context's exhaustion, the timeslice boundary if an equal-priority thread is ready,
and the next refill due. A lone *full* context arms nothing; a lone partial one still arms its exhaustion.

- **AArch64: the EL1 virtual timer** — `CNTV_CVAL_EL0`, `CNTV_CTL_EL0` (`ENABLE`, `IMASK`, `ISTATUS`),
  `CNTVCT_EL0`, and `CNTFRQ_EL0` always read, never assumed. PPI INTID 27, the Arm BSA assignment QEMU's
  `virt` uses (`include/hw/arm/bsa.h`). Preferred to the physical timer, whose EL1 access an EL2 beneath
  us can trap (`CNTHCTL_EL2`); any EL2-to-EL1 descent must grant EL1 timer access and zero `CNTVOFF_EL2`.
  *(Arm ARM DDI 0487, "The Generic Timer".)*
- **x86_64: the local APIC timer**, x2APIC where `CPUID.01H:ECX[21]` reports it, enabled by
  `IA32_APIC_BASE.EXTD` (bit 10). TSC-deadline mode (LVT mode `10b` in bits 18:17; `IA32_TSC_DEADLINE`,
  MSR `0x6E0`) only when `CPUID.01H:ECX[24]` **and** invariant TSC (`CPUID.80000007H:EDX[8]`) are reported;
  otherwise one-shot through the initial-count register, calibrated at boot, a past or zero deadline
  clamped to a count of 1 since 0 stops the timer. QEMU's TCG lacks TSC-deadline (`target/i386/cpu.c`), so
  one-shot is what the emulator runs, not dead code. The console prints the mode. *(Intel SDM Vol. 3A.)*
- **Kernel-reserved; this RFC owns these bits.** `SCTLR_EL1.nTWI` (bit 16) `= 0`: EL0 `wfi` traps (EC
  `0x01`) and completes as `yield`, advancing the saved PC by 4, since `ELR_EL1` holds the `wfi` itself.
  `nTWE` (bit 18) `= 1`: EL0 `wfe` does not trap, so spin-lock backoff costs no kernel entry. `UMA` (bit 9)
  `= 0`; `SA` (bit 3) `= 1`. `CNTKCTL_EL1` and `PMUSERENR_EL0` are written 0 at boot — no EL0 counter,
  timer or PMU — because their reset values are not architecturally fixed. `hlt` at CPL 3 faults; `CR4.TSD`
  is set. RFC-0005 (proposed) writes `SCTLR_EL1` whole and must carry these values; its §13 step 5 today
  says `nTWE = 0` and needs correcting.
- **Userspace timers (RFC-0004 §9.3), named now, built later.** RFC-0004 §8 calls the watchdog
  load-bearing, and the timer is kernel-reserved, so the kernel must provide the source: a **deadline
  object**, bound to a notification, armed with an absolute time in nanoseconds, kept in the per-core
  release queue beside refills, charged to a Pool, and signalled on expiry *(Zircon's timer object)*. One
  armed deadline per object; the sorted insert is bounded by the deadline array's capacity. **Rejected:**
  minting a second hardware timer — the EL1 physical timer (PPI 30) exists per core on AArch64, but x86_64
  has no second per-core timer without the HPET, so the source would differ per Tier-1 target.

## 9. The context switch — what is saved, where

In an event kernel a context switch is *which frame the exit path restores*; assembly is confined to the
entry and exit stubs in `kernel/src/arch/**`. **The user frame lives in the TCB.** On AArch64, while a
thread runs at EL0, `SP_EL1` points at the end of its TCB frame, so the entry stub's stores land there
before it loads the per-core kernel stack from the block `TPIDR_EL1` addresses; `SCTLR_EL1.SA` is on and
the frame 16-byte aligned, and the current-EL `SP_ELx` synchronous vector reports a frame overwrite if
`SP` lies inside a TCB. On x86_64 an interrupt from ring 3 loads `RSP` from `TSS.RSP0`, pointing at the
TCB frame; **`syscall` does not switch `RSP`**, so its stub runs `swapgs`, parks the user `rsp` in per-CPU
scratch, loads the TCB frame pointer, saves into the frame, and only then loads the kernel stack. RFC-0007
(proposed) owns both entry sequences; this RFC owns the layout and the current-EL IRQ stub (§3). The
address-space switch (`TTBR0_EL1` with ASID; `CR3` with PCID) is RFC-0005 (proposed)'s, made on exit when
the process differs — where a time-protection flush would go (Ge et al., EuroSys 2019).

| What | AArch64 | x86_64 |
|------|---------|--------|
| Every entry, into the TCB | `x0`–`x30`, `SP_EL0`, `ELR_EL1`, `SPSR_EL1`: 34 words | `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`, an error-or-vector slot, 15 GPRs: 21 words |
| Thread-local bases, per switch: 2 more words | `TPIDR_EL0` (EL0-writable) saved and restored; `TPIDRRO_EL0` restored | `FS_BASE` and user GS base restored; `CR4.FSGSBASE` off, so only the kernel sets them |
| Also per switch | `CPACR_EL1.FPEN` (§10) | `CR0.TS` (§10); `TSS.RSP0` |
| Lazily (§10) | `V0`–`V31`, `FPCR`, `FPSR`: 520 bytes, 16-byte aligned | XSAVE area, `XCR0` = x87, SSE, AVX: 832 bytes, 64-byte aligned |
| Never | kernel registers, debug and PMU state | the same |
| **TCB size** | 36-word frame (288 B) + FP area padded to 528 B + ~180 B of fields: **1 KiB** | 23-word frame (184 B, padded to 192) + 832 B XSAVE + ~180 B: **1.25 KiB** |

- **O-5 on frames.** `write_regs` never lets userspace choose its privilege: the kernel builds `SPSR_EL1`
  (EL0t, DAIF clear, only NZCV from the caller); on x86_64 `CS`/`SS` are constants, `RFLAGS` takes only
  status flags (IOPL 0, IF set), and a non-canonical `RIP` or base is refused. It writes `TPIDR_EL0` and
  `FS_BASE` only: `TPIDRRO_EL0` and the user GS base are kernel-owned IPC-buffer discovery (RFC-0007 §5).
- NMI, `#DB`, `#DF` and `#MC` run on IST stacks, as RFC-0007 (proposed) asks: they can arrive in the
  `syscall` window before `RSP` is switched. AArch64 has no such window.

## 10. FP/SIMD — owned by userspace, enabled per thread, saved lazily (verdict 10)

- **Enabled per thread** by `set_fp`, off by default. `CLAUDE.md` says "per process"; per thread is finer
  so an FP-off server thread in an FP-using process never forces a save — the § Architectures wording
  changes with this RFC. A thread with FP off that touches FP takes an ordinary fault.
- **Loaded on first use, saved lazily.** Each core records an FP *owner* as an `(index, generation)` pair;
  every other thread runs with access trapped — `CPACR_EL1.FPEN = 0b00` (EC `0x07`, which the reporter
  already names) or `CR0.TS = 1` (`#NM`, vector 7). `FPEN = 0b00` traps EL1 too and `XSAVE` raises `#NM`
  while `TS` is set, so the trap handler runs: `FPEN = 0b01` (EL1 permitted, EL0 still trapped) and `isb`,
  or `clts`; save the owner's registers if the owner is live; load the new owner's, or zeros on first use;
  `FPEN = 0b11`; `isb`; return. A `call` to an FP-off server saves nothing.
- **The owner is cleared** — its registers unsaved — when the owner thread is destroyed or `set_fp(off)`,
  so a thread reusing its slot finds a stale generation and loads zeros, never a dead thread's registers.
- **The kernel's only FP instructions** are those save and restore stubs in `kernel/src/arch/**`,
  executed only with access granted; the soft-float target is the guarantee for generated code. No `FPEN`
  value traps EL1 while allowing EL0, so while an owner runs a stray kernel FP instruction would not trap.
- **Extent.** x86_64 sets `CR4.OSFXSR`, `CR4.OSXSAVE` and `XCR0 = 0b111` — the 832-byte standard-format
  area (512 legacy + 64 header + 256 AVX). AVX-512, AMX and SVE (`CPACR_EL1.ZEN`) stay disabled and fault.
- **The known cost — LazyFP** (CVE-2018-3665; Stecklina & Prescher, 2018): on affected Intel cores a
  non-owner can read the owner's registers speculatively before `#NM` resolves. Hardware side channels are
  out of scope (threat model §7); the cure is Open question 2, costed in §16.

## 11. Interrupts — the controller, and device IRQs as notifications (verdict 11)

The kernel owns the controller and nothing behind it. *(seL4 `IRQControl`/`IRQHandler`.)*

- **Objects.** One `IrqControl` (the root task's) mints at most one `IrqHandler` per device line.
  Kernel-reserved lines — the timer PPI, the IPI SGIs or vector, spurious INTID 1023 — are never minted.
  `bind(irq, notification, bits)` invokes the `IrqHandler` (`WRITE`) and presents the notification
  (`WRITE`); `ack(irq)` needs `WRITE`. **Phase 1 routes every line to the boot core**; the SMP increment
  decides routing as an `IrqControl` concern, never a `SchedControl` method. Shared PCI INTx lines on
  `q35` are **unsupported** until MSI — a Phase-2 cost on the second Tier-1 target.
- **Delivery.** The kernel acknowledges, **masks the line**, signals end of interrupt, ORs `bits` into the
  notification and wakes a waiter through §6's dispatcher; the driver's `ack` unmasks. A level-triggered
  line cannot storm, but can fire once per `ack` (§16). A destroyed notification leaves the line masked
  (fail closed); the `IrqHandler` outlives a crashed driver, and a restarted one re-binds (O-17).
- **AArch64: GICv3 first.** Distributor: `GICD_CTLR` with `ARE` and `EnableGrp1` (`virt` without
  `secure=on` has one security state, `DS = 1`); SPIs through `GICD_ISENABLER<n>`/`ICENABLER<n>`,
  `GICD_IPRIORITYR<n>` and `GICD_IROUTER<n>`. Redistributor: woken by `GICR_WAKER`; SGIs and PPIs through
  `GICR_ISENABLER0` and `GICR_IPRIORITYR<n>`. Every enabled line is put in Group 1 explicitly (Group 0 would
  arrive as FIQ) at a priority numerically below the `ICC_PMR_EL1` mask. CPU interface: `ICC_SRE_EL1.SRE`,
  `ICC_IGRPEN1_EL1`, acknowledge `ICC_IAR1_EL1`, end `ICC_EOIR1_EL1`. On `virt`: distributor
  `0x0800_0000`, redistributors from `0x080A_0000` (QEMU `hw/arm/virt.c`), reached through RFC-0005
  (proposed)'s MMIO window. *(Arm IHI 0069.)* **Cost:** `virt` picks GICv2 at eight CPUs or fewer
  (`finalize_gic_version_do`), so xtask gains `gic-version=3`; the Raspberry Pi 4 in Constitution §6's
  bring-up order has a GIC-400 (GICv2) — a **known** second driver.
- **x86_64: LAPIC and I/O APIC.** The 8259s are masked and never driven; I/O APIC entries carry the mask
  bit; end of interrupt goes to the LAPIC (x2APIC EOI MSR `0x80B`). **MSI/MSI-X wait for the driver RFC:**
  without IOMMU interrupt remapping a device can aim any vector at any core — O-18's precondition again.
- **Latency** is §3's worst path between preemption points. RFC-0007 (proposed)'s console write must take
  one per FIFO-full poll, restartable by byte count; until it does, it holds the kernel ~44 ms per 504
  bytes on real hardware (none on QEMU, which has no baud delay).

## 12. Multi-core shape, placement and exclusive cores (verdict 11)

Phase 1 runs `-smp 1`; the shape is fixed now so multicore is not bolted on.

- **Everything scheduling-related is per core.** A thread runs on the core of the context it holds, so
  donation can move a server thread between cores. **The kernel never load-balances.**
- **Cross-core wake-ups** (RFC-0004 §9.2): enqueue *W* on its core and send an IPI — an SGI through
  `ICC_SGI1R_EL1`; a fixed vector through the LAPIC ICR. Direct switch is same-core only.
- **One big kernel lock when SMP lands**, taken on every entry, the IPC fast path included *(Peters,
  Danis, Elphinstone & Heiser, "For a Microkernel, a Big Lock Is Fine", APSys 2015)*. This **rejects the
  premise** of RFC-0003 §14.2, which asks for table sharing without a global lock on the fast path; within
  one entry the resolve→check→act window holds under the lock (§3 covers restarts). A dated note against
  RFC-0003 records it (§13).
- **A core waiting for the lock services IPIs while it spins** *(as seL4's CLH lock does)*, because two
  operations need a synchronous answer from another core: RFC-0005 (proposed)'s x86_64 TLB shootdown (it
  has no broadcast invalidate) and **FP state live on another core** — when a thread whose registers are
  owned lazily on core X next needs them on core Y, Y asks X by IPI to save them. Cost: a cross-core FP
  handoff is an IPI round trip.
- **Cores are parked until funded (default-deny).** On QEMU `virt` with `-kernel`, the DTB advertises PSCI
  and secondaries start powered off (checked on QEMU 8.2.2: CPU 1 never leaves the entry address), so
  `boot.s`'s `wfe` park loop only matters on spin-table firmware. The first `sc_configure` onto a core
  starts it — PSCI `CPU_ON`, conduit from the DTB's `/psci` node; INIT-SIPI-SIPI on x86_64. Deferred to
  increment 14; `boot.s`'s comment changes with it.
- **SMT placement invariant — one sentence now** (research/0002 Part 6, after L1TF and MDS): a context
  will carry a placement class set only by the `SchedControl` holder; SMT siblings never run different
  classes and donation never crosses one. The field is reserved zero in `sc_configure` until increment 14,
  so no thread can label itself; SMT-off is "never fund the sibling".
- **Exclusive assignment is a degenerate case, not a mode** (research/0002 Part 6, after Jailhouse): one
  thread, a full context, nothing else on the core. The bitmap has one bit, the timer is never armed, no
  IPI arrives unless the `SchedControl` holder acts, and lines are routed elsewhere; counters advance only
  at the next kernel entry. A recipient checks the promise with `ctl_info`: one context, reference count 1.

## 13. Amendments this RFC proposes to accepted documents (verdict 12)

Each, if accepted, is logged in `docs/CHANGELOG.md` under Changed and appended to the target as a dated
amendment — never a silent edit.

| # | Target | Change | Why |
|---|--------|--------|-----|
| A1 | RFC-0004 §4, §8 | `call` donates only to a **passive** receiver; an active one runs on its own context | a thread holds one context; seL4 MCS `reply_push`; cost in §7 |
| A2 | RFC-0004 amendment 2026-08-01 | a caller short *now* waits `Throttled`; only a context that can *never* pass is refused | refusal makes clients spin; no donation happens below the bar either way |
| A3 | RFC-0004 amendment 2026-08-01; research/0002 Part 6 | seL4 timeout faults ship in MCS (`seL4_Fault_Timeout`); what is unmerged is RFC-14's threshold | a factual correction |
| A4 | RFC-0003 §14.2 | the big lock is taken on the fast path by design; the question is answered by rejecting its premise | §12; Peters et al. 2015 |

## 14. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4 MCS** (Lyons et al. 2018) | contexts as objects; passive servers; `SchedControl`; MCP with any-capability authority; sporadic refills; `MIN_BUDGET`; timeout faults; handlers held in the TCB; priority-ordered endpoint queues; no donation to active receivers | the WCET scale factor; ceiling left wholly to convention |
| **seL4 RFC-14** (Johnston) | the endpoint threshold, deferral, one-layer return | budget limits, for now |
| **Heiser & Elphinstone 2016; Fluke 1999** | the event kernel | per-thread kernel stacks |
| **Elphinstone & Heiser 2013** | the running thread unqueued; priority-aware direct switch | lazy scheduling; L4's unconditional switch |
| **QNX Neutrino** | priority inheritance across the message; partitions built on budgets | inheritance to busy servers |
| **Ford & Lepreau 1994** | the migrating-thread picture of `call` | Mach |
| **Sprunt, Sha & Lehoczky 1989** | the sporadic server | an unbounded replenishment list |
| **Zircon** | the timer object, for deadline objects | per-thread timeslices |
| **Jailhouse** | refusing to share: parked cores, exclusive assignment | static-only partitioning |
| **Peters et al. 2015; Linux** | one big lock; the idle wake-up rule; PSI-style stall counters | fine-grained locking before measurement; dynamic priorities |

## 15. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-7 no unprivileged exhaustion | time only through contexts; `MIN_BUDGET`; eight refills; O(1) expiry return; bounded queue walks; no wait holding a lock; lines masked until `ack` | **discharged for CPU time and scheduler structures, except interrupt-entry time**, billed to the running context and bounded per `ack`; memory is RFC-0005's half |
| O-4 no ambient authority | no thread runs without a context; priority through MCP, born 0; time through `SchedControl`; interrupts through `IrqControl`; only `yield` and `thread_exit` authority-free | **discharged for time** |
| O-5 argument validation | `write_regs` sanitises `SPSR_EL1`/`RFLAGS`, refuses non-canonical `RIP` and bases, cannot touch discovery registers; restarts re-resolve | **discharged for this surface** |
| O-9 no ambient side channel | no EL0 counter, timer or PMU; FP owner cleared on reuse; parked cores. **Residual:** untrapped `wfe` lets any thread observe another's `sev` — one bit of wake-up, no data | **contributes** — fine-grained timing channels stay out of scope |
| O-17 contained drivers | lines as notifications, masked until `ack`; `IrqHandler` outlives a crashed driver; fault endpoints the thread cannot close; stall counters | **enables**, weakened where a child holds `WRITE` on its own threads (§4) |
| O-18 confined DMA | MSI deferred until interrupt remapping is designed | **named, not discharged** |
| RFC-0004 amendment, 2026-08-01 | (1) `min_budget` at `call`, with deferral (A2); (2) `BudgetExpired` on the reply object, plus timeout faults (§7) | **both discharged as amended by A1–A2**; caller death cannot bypass (1) |

## 16. Graves checked (§3)

- **Policy in the kernel.** No admission, balancing or budget choice. The constants the kernel does
  choose: 256 levels; eight refills; `MIN_BUDGET`'s factor 2; the XCR0 extent; the thread and deadline
  array capacities (RFC-0005); and §18's interim constants until measured.
- **The catch-all right.** Priority authority is a relation (MCP); `SchedControl` has one mutating method,
  selling time on one core, and a read of its own state; topology is data and routing belongs to
  `IrqControl`; interrupts are per line. The set is closed by listing it (§4).
- **Bolted-on multicore.** Per-core state, IPIs, the lock, its IPI-servicing spin, FP handoff, shootdowns
  and core bring-up are stated before any SMP code.
- **Drivers in the kernel; compiled-in unused device paths.** The kernel masks and signals, never services
  a device; one controller and one timer per target; unclaimed lines never enabled.
- **Baroque hierarchies; multi-copy IPC.** Contexts are flat; donation moves a reference to time.

## 17. Costs — what this makes harder

- **Every server needs someone to think about time.** A passive server cannot run until called; an active
  one needs a configured context and gets no inheritance (A1). "Spawn a thread and it runs" is gone.
- **The one-hop unwind leaves a stranded server to userspace**, and `min_budget` 0 lets one client strand
  it for all. A supervisor is required before any passive server on partial contexts is trusted.
- **Caps, not minima:** a partial context idles its core when spent, even with the core otherwise idle.
- **Interrupt time is billed to whoever runs:** a driver `ack`ing a level line in a loop charges other
  processes' budgets, one interrupt per `ack` — seL4's choice; charging an `IrqHandler`'s bound context
  becomes possible with passive drivers (Open question 4).
- **Priority-ordered queues** cost an O(n) walk with interrupts masked, *n* up to the thread capacity.
- **No userspace timer yet:** RFC-0004 §8's load-bearing watchdog has no time source until deadline
  objects (increment 15) — an unmet RFC-0004 dependency, named.
- **Exclusivity is checkable, not revocable:** `ctl_info` shows a grantor kept no copy, but a grantor who
  did can still configure onto the core.
- **MCS is seL4's least-verified part**, and sporadic refills are subtle — hence property tests.
- **About 1 KiB per TCB on AArch64 and 1.25 KiB on x86_64**, FP space included for threads that never use it.
- **Lazy FP's cure, costed.** Eager restore on a switch into an FP-enabled thread of another process moves
  520 bytes (16 `ldp q` pairs) or an 832-byte `XRSTOR` — 9 or 13 cache lines — and nothing on IPC to
  FP-off servers; its cycles are a measurement owed, beside a `call` round trip of roughly 380–630 cycles
  (twice research/0001's one-way 188–316).

## 18. Open questions

1. **EL0 counter access.** Proposed: a property of the *context*, set by the `SchedControl` holder in
   `sc_configure` and switched with it (`CNTKCTL_EL1.EL0VCTEN`; `CR4.TSD`), default deny — not a thread
   flag, which a process could set on itself. Denial costs a kernel entry per timestamp.
2. **LazyFP on x86_64.** Eager restore for cross-process switches into FP threads (costed in §17)?
3. **RFC-14 budget limits.** Worth the fastpath cost to stop a passive server overspending?
4. **Passive drivers** need a context bound to a notification (MCS); with them, interrupt entry can be
   charged to the driver.
5. **Numbers from measurement:** `MIN_BUDGET`, kernel stack size, the worst kernel path; x86_64's clock
   when the TSC is not invariant.
6. **Interrupt routing on SMP** (§11): `IrqControl` minting per-(line, core) handlers, or a separate
   per-core interrupt-target capability — decided with increment 14.

## 19. Implementation increments

Pure logic lands in a new host-tested `no_std` workspace crate, `scheduler/`, generic over a thread index
as `capability/` is over `ObjectRef` (the `CLAUDE.md` § Layout edit lands with increment 1); hardware
behaviour is proved by counted QEMU boot-test strings. New `unsafe` stays in `kernel/src/arch/**`. The
x86_64 HAL gains stubs that compile, `-> !` ones halting, so its entry still calls straight through into
the kernel proper and the second build compiles the scheduler. **D** marks what the Phase-1 demo needs.

| # | PR | Proves; how tested | Lands after | D |
|---|----|--------------------|-------------|---|
| 1 | `scheduler` I: run queues, the ten states, MCP, priority-ordered endpoint queue | exhaustive host transition table against §4's "Left by" column; "a new thread cannot serve as `auth` above 0"; CI builds it for both targets | — | D |
| 2 | `scheduler` II: full contexts, nanosecond conversion and rounding, billing, stall counters, the interim constants | host tests against a shadow model | 1 | D |
| 3 | GICv3 and the virtual timer, kernel-only; `CNTKCTL_EL1`, `PMUSERENR_EL0` zeroed; idle and its abandon-idle IRQ stub | xtask gains `gic-version=3`; boot-test `timer: tick 3` | 0005#9 (GIC in the MMIO window) | D |
| 4 | Event-kernel dispatch at EL0, MMU off: TCB frames, dispatch and RFC-0006's frame layout under RFC-0007#3's trap stub; `SCTLR_EL1` bits of §8 set | two EL0 self-test threads alternate under the timer: `sched: A B A B`; an EL0 `wfi` completes: `sched: wfi yields` | 3, 0007#3 | D |
| 5 | Threads in address spaces; the boot stack moved into the guarded window, `aarch64.ld`'s comment with it | a spinning EL0 thread is preempted and resumes intact: `user: still here` | 4, 0005#10 | D |
| 6a | Thread, SchedContext and `SchedControl` as slots in RFC-0005's object store, state machine wired, links `(index, generation)` | host tests in `scheduler/`: destroy a lent context, reuse its slot, `reply` — nothing moves | 2, 0005#7 | D |
| 6b | The methods behind RFC-0007's dispatcher: `sc_configure`, `bind_sc`, `resume`, `set_priority`, `write_regs`, `thread_exit` | the root configures two EL0 threads at equal priority: `[a] 1 [b] 1 [a] 2` | 5, 6a, 0007#6 | D |
| 7a | Direct-switch decision as a pure function | host tests over every §6 case | 1 | D |
| 7b | Endpoint queues and IPC blocking states, wired | host tests: queue order, `suspend` from each blocked state | 6b, 7a | D |
| 7c | Integration with RFC-0007#10 | RFC-0007#10's three `Kaya!` lines, then `[client] preempted` from a spin after the reply | 7b, 0007#10 | D |
| 8 | Passive servers, donation, inheritance, the caller-death rule | host test: a client exits mid-request, the server finishes and serves the next; boot-test `passive: served` | 7c | stretch |
| 9 | `scheduler` III: partial contexts, refills, merge, release queue | property test: no window of length `T` sees more than `B` consumed | 2 | |
| 10 | `min_budget`, deferral, expiry return, fault and timeout endpoints, `Unfunded` | boot-tests `BudgetRefused`, then `BudgetExpired` and `timeout fault` for an overrunning server | 8, 9, 0007#11 | |
| 11 | Lazy FP/SIMD | two FP threads load markers into `v0`, each finds its own: `fp: isolated`; host test: destroy the owner, reuse its slot, first use loads zeros | 6b | |
| 12 | `IrqControl` / `IrqHandler`; xtask gains serial input injection | PL011 receive (SPI 1, INTID 33) reaches an EL0 driver: `irq: notified` | 7c | |
| 13 | x86_64 scheduling HAL (IDT, TSS, IST, LAPIC, `#NM`, `XSAVE`) | 3–5 and 11 again | its boot RFC, 0007#12 | |
| 14 | SMP: PSCI `CPU_ON` and INIT-SIPI, the big lock with IPI-servicing spin, FP handoff, shootdowns, placement classes, routing | `-smp 2` boot-tests | 13 | |
| 15 | Deadline objects (userspace timers, RFC-0004 §9.3) | host tests on the shared release queue; boot-test `deadline: fired` | 9 | |

**What the demo takes as interim, honestly.** It needs 1–7c and runs full contexts only.

- **Its server is active**, the root doubling as server (RFC-0007 §18), so the demo exercises **A1's
  amended semantics, not RFC-0004 §4's**: no donation happens. Increment 8 is one more PR, safe before 9
  and 10 because a lent full context refills at once; expiry inside a server needs a partial context.
- **Provisional constants**, in `scheduler/src/config.rs` until measured: `MIN_BUDGET_INTERIM` = 100 µs;
  `ROOT_TIMESLICE` = 5 ms (a full context); root priority and MCP 255, the root lowering itself and the
  client to 100 to share a level; `KERNEL_STACK_SIZE` = 16 KiB. Cost: the budget floor is a guess.
- **No fault handler:** a faulting thread goes `Inactive` and the reporter prints it (RFC-0007 §7).
- **The console write is non-preemptible** until RFC-0007 adds its preemption points — free on QEMU,
  about 44 ms on hardware. **No userspace timer**, so RFC-0004's watchdog cannot yet be built.
- **The x86_64 stubs** compile and halt, removed by 13. Userspace is soft-float, so 11 is not needed.

## 20. What this unblocks

RFC-0004's implementation gets threads to rendezvous between. The driver framework receives interrupts as
notifications (O-17). The broker gains levers to sell time without the kernel knowing why: `SchedControl`,
MCP, `min_budget` and stall counters. Next on paper: the x86_64 boot RFC (increment 13), an SMP amendment
(14), and the dated entries of §13.
