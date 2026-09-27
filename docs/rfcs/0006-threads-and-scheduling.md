<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0006 — Threads and scheduling

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 ("IPC is the product", priority inheritance, bolted-on multicore); the HAL (`kernel/src/arch/**`) and `aarch64.ld`'s stack plan; RFC-0003 §14.2; RFC-0004 §4, §8, §9.2 and its 2026-08-01 amendment; every userspace driver |
| Depends on | RFC-0003 (capability table); RFC-0004 (IPC) and its 2026-08-01 amendment; RFC-0005 (proposed) for object memory, kernel stacks and address-space switching; RFC-0007 (proposed) for the trap path, the syscall encoding, the Process object and the root task |
| Discharges | O-7 for CPU time and the scheduler's own structures; O-4 for time; both commitments of RFC-0004's 2026-08-01 amendment; answers RFC-0003 §14.2 and RFC-0004 §9.2 in shape; contributes to O-5, O-9, O-17 |

> **Proposed verdicts.**
>
> - **V1 — Event kernel (the load-bearing choice).** One kernel stack per core; the kernel is non-preemptible; every wait is a thread state.
> - **V2 — Thread.** An RFC-0003 object with a fixed-size TCB holding its user frame; nine states; `yield` and `exit` are authority-free.
> - **V3 — Time is a capability.** A `SchedContext` (budget, period, ≤ 8 refills) is an object, configured only through a per-core `Core`.
> - **V4 — Priorities and dispatch.** 256 fixed levels; an MCP guard, no scheduling right; O(1) bitmap run queues; FIFO endpoint queues.
> - **V5 — Donation and the amendment.** `call` lends time to a passive server at `max(own, caller's)` priority; an RFC-14 threshold; one-hop `BudgetExpired`.
> - **V6 — Timer.** Tickless one-shot deadlines: the AArch64 EL1 virtual timer; the x86_64 LAPIC, TSC-deadline only with an invariant TSC.
> - **V7 — FP/SIMD.** Off by default, enabled per thread, loaded on first use, saved lazily by owner tracking; the kernel never touches it.
> - **V8 — Interrupts.** GICv3 first; LAPIC and I/O APIC; a device line is an `IrqHandler` that signals a notification, masked until `ack`.
> - **V9 — Multi-core shape.** Per-core state; a big lock when SMP lands; cores parked until funded; SMT placement classes; exclusivity as a degenerate case.

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
availability (A5) lives. seL4 MCS made time a capability, and this RFC adopts that shape.

## 3. The execution model — one kernel stack per core (V1)

The load-bearing choice, because it decides what a context switch *is*.

- **Option A — process kernel** (a kernel stack per thread: Linux, Zircon, original L4). **Rejected:** a
  guarded stack per thread in a kernel with no heap, and blocking in the kernel is how "hold a lock across
  a wait a process controls" (O-7) becomes possible.
- **Option B — event kernel** (one stack per core: seL4, OKL4, NOVA; Fluke's "interrupt model").
  **Verdict sought.** The kernel is entered by exception, interrupt or syscall, runs to completion with
  interrupts masked, and leaves by restoring *some* thread's user frame. A syscall that must wait records
  a state and the object waited on; on wake, the result is written into the waiter's frame, or the call
  restarts by rewinding the saved PC one instruction (4 bytes for `svc #0`, 2 for `syscall`). *(Heiser &
  Elphinstone, "L4 Microkernels: The Lessons from 20 Years of Research and Deployment", TOCS 2016; Ford
  et al., "Interface and Execution Models in the Fluke Kernel", OSDI 1999.)*

**The cost:** every kernel path must be bounded. The long ones — object destruction, RFC-0005
(proposed)'s `reissue` and address-space teardown — take **preemption points**: on a pending interrupt
(`ISR_EL1.I`; the LAPIC IRR) they leave the operation restartable and exit. The worst path is measured and
published: it is the interrupt latency. This **reverses `aarch64.ld`'s plan** that the boot stack "will be
replaced by per-thread stacks": it becomes core 0's kernel stack, in RFC-0005 (proposed)'s guarded window.

## 4. The Thread object (V2)

A **Thread** is an RFC-0003 object belonging to one Process, which RFC-0007 (proposed) provides. Its TCB
is a slot in RFC-0005 (proposed)'s typed array for threads, charged to the Pool presented at creation;
queue links are slot **indices**, not pointers.

| Field | Meaning |
|-------|---------|
| `frame` | the full user register file, written at every kernel entry (§9) |
| `state`, `blocked_on`, links | a state below; the endpoint, reply object or notification waited on; intrusive queue links |
| `priority`, `mcp` | base priority and maximum controlled priority (§6) |
| `home_sc`, `sc`, `effective` | the bound context, if any; the context held now, own or donated; the priority run at now (§7) |
| `bound_notification`, `ipc_buffer` | the notification a `recv` may also wake on (RFC-0004 §10); the IPC-buffer page bound by capability (RFC-0005, RFC-0007, proposed) |
| `fault_ep`, `timeout_ep` | **handles in the thread's own process table**, resolved when raised, so a resolved capability lives only in a table (RFC-0003) and a revoked handler fails closed *(seL4 master; MCS caches them in the TCB)* |
| `fp_enabled`, `fp_state`, `class` | the FP/SIMD flag and save area (§10); the SMT placement class (§12) |

| State | Meaning | Left by |
|-------|---------|---------|
| `Inactive` | created or suspended; in no queue | `resume` |
| `Ready` | runnable with released budget; in its core's run queue | dispatch |
| `Running` | the current thread on its core; in no queue | preemption, block, `yield`, fault, `exit` |
| `Throttled` | runnable, but no released refill; in the release queue | refill release |
| `BlockedOnSend` | queued on an endpoint to `send` or `call` | rendezvous, endpoint destruction |
| `BlockedOnRecv` | waiting on an endpoint (and any bound notification) | rendezvous, signal |
| `BlockedOnReply` | waiting on a reply object after `call` or a fault message | reply, error completion (§7) |
| `BlockedOnNotification` | waiting on a notification alone | signal |
| `Exited` | finished; in no queue; the object lingers until destroyed | never |

Faults reuse IPC: the kernel `call`s the fault endpoint on the thread's behalf and it waits in
`BlockedOnReply`; RFC-0007 (proposed) defines the message. Destroying a thread bumps its generation
(RFC-0003 §7), unlinks it in O(1) from whichever doubly linked queue holds it, destroys any reply object
it waits on (RFC-0004 §8) and returns any donated context as budget expiry does (§7).

| Operation | Authority | Effect |
|-----------|-----------|--------|
| `set_priority(t, auth, p)` / `set_mcp(t, auth, m)` | `t`: `WRITE`; `auth`: `READ`; value `≤ mcp(auth)` | §6 |
| `bind_sc(t, sc)` / `unbind_sc(t)` | `t`, `sc`: `WRITE` | active or passive; class checked (§12) |
| `resume(t)` / `suspend(t)` | `t`: `WRITE` | `Inactive` ↔ runnable; a first `resume` closes the Process's grant window (RFC-0007, proposed) |
| `read_regs(t)` / `write_regs(t, …)` | `READ` / `WRITE` | frame and TLS bases, sanitised (§9) |
| `set_fp(t, on)` / `set_class(t, c)` | `t`: `WRITE` | §10 / §12 |
| `sc_configure(core, sc, B, T, refills, class)` / `sc_stats(sc)` | `core`, `sc`: `WRITE` / `sc`: `READ` | §5 |
| `yield()` / `exit()` | none — two of RFC-0003 §9's three authority-free exceptions | tail of own priority / `Exited` |

No right joins RFC-0003's set; RFC-0007 (proposed) owns the encoding.

## 5. Scheduling contexts and `Core` — time as a capability (V3)

**Options.** A timeslice on the thread (classic L4, Zircon) makes creating a thread *acquiring CPU*, with
nothing to grant or withhold — **rejected** on O-4. Partition budgets (QNX adaptive partitioning) are
**subsumed**: a partition is what a `Core` holder builds from several contexts. **Verdict sought:**
contexts as objects *(seL4 MCS: Lyons, McLeod, Almatary & Heiser, "Scheduling-context capabilities",
EuroSys 2018)*.

A **SchedContext** holds budget `B`, period `T` (`B ≤ T`), at most eight refills, its core, its placement
class, its home thread and its holder. A thread with a bound context is **active**; one without is
**passive** and runs only on donated time (§7). Its *memory* is charged to a Pool; its *time* comes from
a `Core`. One accounting domain for both (RFC-0005 (proposed)'s open question) is left to the resource RFC.

- **`Core`, one per core, minted to the root task** (RFC-0007 (proposed)). `sc_configure` through *c*'s
  `Core` binds a context to *c*: the authority to sell time on *c* and nothing else. `READ` yields
  topology — package and SMT siblings, via RFC-0005 (proposed)'s discovery. No syscall names a core by
  number. *(seL4 `SchedControl`; research/0002's "core-set capability".)* `B` below **`MIN_BUDGET`** —
  twice the measured worst-case kernel path, seL4 MCS's rule (`include/kernel/sporadic.h`) — is refused.
- **Full contexts** (`B = T`) are never throttled and round-robin with timeslice `T`. **Partial contexts**
  (`B < T`) are sporadic servers: within any window of length `T` the holder consumes at most `B`.
  Refills fall due a period after consumption; when all eight are in use they merge, forfeiting
  bandwidth rather than exceeding it. *(Sprunt, Sha & Lehoczky 1989.)*
- **What a budget means** (research/0002: minima, not caps; burst and redistribution stated). The kernel
  enforces the **cap**; dispatch is **work-conserving**; the **minimum** follows from admission, the `Core`
  holder's policy. Unused budget never carries past its window.
- **Accounting window and billing granularity**, stated as design (the QNX caveat, ECRTS '21, research/0002):
  the window is each context's own sliding period; billing is exact, from the counter (`CNTVCT_EL0`; the
  TSC) read at every entry and exit — 16 ns on QEMU's `cortex-a72`, which keeps the pre-Armv8.6 62.5 MHz
  default (flag set in QEMU `target/arm/tcg/cpu64.c`, applied in `target/arm/cpu.c`). Kernel time,
  interrupts included, is billed to the context running on entry. Budgets round down, periods up.
- **Stall counters from the first version** (research/0002 Part 6, after PSI and oomd): per context, time
  consumed, time and count throttled, time ready but waiting, and budget expiries, read-only via
  `sc_stats` — the O-17 supervisor's wedged-server signal. The kernel measures; userspace acts.

## 6. Priorities and dispatch (V4)

**256 levels**, 255 most urgent, meaningful within a core. `set_priority(t, auth, p)` needs `WRITE` on
`t`, `READ` on `auth` and `p ≤ mcp(auth)`; MCP is set the same way, so authority over urgency only
narrows as it is delegated — O-2's shape. The root task's thread starts with MCP 255 and a full context
on core 0, the one bootstrap act; the kernel assigns no other priority. *(seL4 `TCB_SetPriority`.)*

**Rejected: a `SCHEDULE` or `PRIORITY` right** — the catch-all right in embryo, one bit whose meaning
grows with every scheduling operation (the CAP_SYS_ADMIN grave). The MCP is a monotone ceiling, a relation
between two capabilities, and needs no bit. **Rejected: dynamic priorities** (CFS, BSD decay, boosts):
kernel-resident policies churned in every kernel that carried them while accounting and enforcement
endured (research/0002 Part 1, "Mechanism/policy split").

- **Run queues.** Per core, 256 intrusive FIFO lists and a 256-bit bitmap: the next thread is a
  count-leading-zeros over four `u64` words — O(1). A preempted thread re-enters at the head of its level;
  one whose timeslice ended, at the tail. The running thread is never queued, and a thread switched to
  directly is never enqueued at all. *(Elphinstone & Heiser, "From L3 to seL4", SOSP 2013.)*
- **Release queue.** Per core, sorted by next refill: O(n) in contexts only the `Core` holder can add.
- **Idle is a kernel loop, not a thread**, billing nobody, and the lost wake-up is designed out. AArch64
  runs `wfi` with `PSTATE.I` set — a pending interrupt still wakes it — then unmasks briefly; Linux warns
  masking at `ICC_PMR_EL1` would *not* wake the core (`arch/arm64/kernel/idle.c`), so this kernel masks
  with DAIF only. x86_64 uses `sti; hlt`, whose STI shadow issues the `hlt` before delivery.
- **Priority-aware direct switch** (RFC-0004 §4). When *C* wakes *W* by IPC on the same core and *W* has
  released budget: if *C* blocks (`call`, `recv`, `reply_recv`), switch to *W* when its effective priority
  is at least the highest ready; if *C* stays runnable (`send`, `reply`, `notify`), only when *W* is also
  strictly more urgent than *C*. Otherwise *W* is queued. A *W* without released budget goes to the
  release queue; one on another core is enqueued there and sent an IPI (§12).
- **Endpoint queues are FIFO** — a refinement of RFC-0004, which fixed no order: O(1) insertion keeps the
  hottest path bounded with interrupts masked. *(seL4's current MCS `tcbEPAppend` is FIFO.)* **Rejected:
  priority order** *(QNX)* — an O(n) walk over a thread count any process can grow from its own Pool.

## 7. Donation, inheritance and budget expiry (V5)

**Donation.** When *C* `call`s and the receiver *S* is passive, the kernel moves *C*'s current context to
*S*, recorded in the single-use reply object *R* (RFC-0004 §8); `reply` or `reply_recv` moves it back.
*S* runs at `max(prio(S), eff(C))`: the constitution's priority inheritance along the call chain, with
*S*'s own priority a **ceiling floor** that policy can raise to the highest client priority for the
priority-ceiling bound seL4 MCS relies on. Lending priority is safe because time is lent with it. A `call`
to an **active** receiver donates nothing. Chains compose one hop at a time. *(Ford & Lepreau, migrating
threads, 1994; QNX priority inheritance; seL4 MCS passive servers.)*

**Rejected:** a passive server at its own priority only (seL4 MCS), leaving inversion to a global
convention; and boosting a busy server to a waiting sender's priority (QNX), where the kernel would guess
which thread serves an endpoint — policy — and walk unbounded chains with interrupts masked.

**The 2026-08-01 amendment, discharged**, following seL4 RFC-14 (Mitchell Johnston, "Budget limit
thresholds on endpoints for SC Donation", proposed 2023-08-15; seL4/rfcs pull request 24, opened
2024-06-17, still open; read in full for this RFC):

- **Prevention — the threshold.** An endpoint carries a `threshold`, fixed at creation (0 means none;
  RFC-0007 (proposed) encodes it, as `min_budget`). A `call` needs released budget of at least `threshold`
  plus twice the kernel's worst-case path, paying for the `call` and `reply` themselves — RFC-14's rule.
  A context whose `B` can never meet that fails with `BudgetRefused`; a non-donating `send` to a
  thresholded endpoint is invalid. With `threshold` at the server's WCET, expiry inside it is a true error.
- **A flagged deviation.** The amendment says donation below the threshold is "refused up front". RFC-14,
  which it cites, refuses only a context that can *never* pass; one that cannot pass *now* has its refills
  merged and deferred, waits `Throttled`, and the call restarts when the head refill suffices — free in an
  event kernel, O(refills) ≤ 8. Proposed: RFC-14's behaviour. No donation happens below the threshold, so
  the amendment's purpose holds; refusing a transient shortfall would make every client spin to retry. If
  accepted, it is logged against RFC-0004 as a dated amendment.
- **Recovery — one layer down.** If a donated context is exhausted with no refill due while passive *S*
  runs on it, the kernel returns it to *S*'s immediate caller *C*, whose wait on *R* completes with
  `BudgetExpired` (observed when the context next refills); *R* is consumed, so a late `reply` fails
  closed. *S* is left runnable but unfunded, registers intact, and the context's expiry counter rises. If
  *S*'s `timeout_ep` resolves, the kernel raises a **timeout fault** carrying *S*'s identity and consumed
  time; the handler, active with time of its own, abandons the request (`write_regs`) or lends *S* a
  context to finish. With no handler, *S* stays stranded, contained to its own clients. *(seL4 MCS:
  `tcbTimeoutHandler`, `seL4_Fault_Timeout`.)*
- **Why one layer.** In A → B → C, expiry in C returns time to B, which gets `BudgetExpired` and can reply
  to A with an error; unwinding to A would walk the reply stack — the O(n) walk RFC-14 also rejects. This
  kernel-initiated completion is the one the amendment left to this RFC: a status, no new object.
- **A reconciliation.** The amendment says seL4's timeout exceptions "have remained future work"; they
  ship in MCS, and what is unmerged is RFC-14's threshold. The phrase wants a dated correction only.

**Rejected: the unwind without timeout faults**, where the stranded server is visible only through its
state and counters. Timeout faults are no second protocol — they travel RFC-0007 (proposed)'s fault path
— and a supervisor woken by a message beats one polling every passive server; the no-handler case *is*
that unwind. **Deferred: RFC-14's budget limits** (capping what a server spends of a donated context):
RFC-14 measured 22% extra fastpath cost on `call` and 21% on `reply_recv`, against 13% for thresholds.

## 8. The timer and preemption (V6)

One HAL surface: `now() -> Ticks`, `frequency() -> Hz`, `set_deadline(Ticks)`, `cancel()`, and a
`timer_fired()` up-call. The kernel is **tickless**: each exit programs one per-core deadline — the
earliest of a partial context's exhaustion, the timeslice boundary if an equal-priority thread is ready,
and the next refill due. With no competitor (§12's exclusive core) the timer is cancelled.

- **AArch64: the EL1 virtual timer** — `CNTV_CVAL_EL0`, `CNTV_CTL_EL0` (`ENABLE`, `IMASK`, `ISTATUS`),
  `CNTVCT_EL0`, and `CNTFRQ_EL0` always read, never assumed. PPI INTID 27, the Arm BSA assignment QEMU's
  `virt` uses (`include/hw/arm/bsa.h`). Preferred to the physical timer, whose EL1 access an EL2 beneath
  us can trap (`CNTHCTL_EL2`); the EL2-to-EL1 descent `boot.s` owes must grant EL1 timer access and zero
  `CNTVOFF_EL2`. *(Arm ARM DDI 0487, "The Generic Timer".)*
- **x86_64: the local APIC timer**, x2APIC where offered (`IA32_APIC_BASE` bit 10). TSC-deadline mode (LVT
  mode `10b` in bits 18:17; `IA32_TSC_DEADLINE`, MSR `0x6E0`) only when `CPUID.01H:ECX[24]` **and**
  invariant TSC (`CPUID.80000007H:EDX[8]`) are reported; otherwise one-shot through the initial-count
  register, calibrated at boot, behind the same `set_deadline`. QEMU's TCG lacks TSC-deadline
  (`target/i386/cpu.c` lists it as missing), so one-shot is what the emulator runs, not dead code. The
  console prints the mode chosen. *(Intel SDM Vol. 3A, the APIC chapter.)*
- **Kernel-reserved.** The timer line is never minted (§11). EL0 `wfi` traps (`SCTLR_EL1.nTWI = 0`) and
  completes as `yield`; `hlt` at CPL 3 faults. EL0 `wfe` does **not** trap (`nTWE = 1`): spin-lock backoff
  uses it, trapping would make every spin a kernel entry, and the armed timer's interrupt wakes it anyway.
  RFC-0005 (proposed) writes `SCTLR_EL1` and must carry these two bits.

## 9. The context switch — what is saved, where

In an event kernel a context switch is *which frame the exit path restores*; assembly is confined to the
entry and exit stubs in `kernel/src/arch/**`. **The user frame lives in the TCB.** On AArch64, while a
thread runs at EL0, `SP_EL1` points at the end of its TCB frame, so the entry stub's stores land there
before it loads the per-core kernel stack from the block `TPIDR_EL1` addresses. On x86_64 an interrupt or
exception from ring 3 loads `RSP` from `TSS.RSP0`, pointing at the TCB frame — but **`syscall` does not
switch `RSP`**: its stub must `swapgs`, park the user `RSP` and load the frame pointer itself. RFC-0007
(proposed) owns both entry sequences; this RFC owns the layout. The address-space switch (`TTBR0_EL1` with
ASID; `CR3` with PCID) is RFC-0005 (proposed)'s, made on exit when the process differs — the single point
where a time-protection flush would go (Ge et al., EuroSys 2019; research/0002 Part 6).

| What | AArch64 | x86_64 |
|------|---------|--------|
| Every entry, into the TCB | `x0`–`x30`, `SP_EL0`, `ELR_EL1`, `SPSR_EL1`: 34 words | `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`, an error-or-vector slot, 15 GPRs: 21 words |
| Thread-local bases, per switch | `TPIDR_EL0` (EL0-writable) saved and restored; `TPIDRRO_EL0` restored | `FS_BASE` and user GS base restored; `CR4.FSGSBASE` off, so only the kernel sets them |
| Also per switch | `CPACR_EL1.FPEN` (§10) | `CR0.TS` (§10); `TSS.RSP0` |
| Lazily (§10) | `V0`–`V31`, `FPCR`, `FPSR`: 520 bytes, 16-byte aligned | XSAVE area, `XCR0` = x87, SSE, AVX: 832 bytes, 64-byte aligned |
| Never | kernel registers, kernel FP (soft-float), debug and PMU state | the same |
| **TCB size** | 36-word frame (288 B) + FP area padded to 528 B + ~150 B of fields: **1 KiB** | 23-word frame (184 B, padded to 192) + 832 B XSAVE + ~150 B: **1.25 KiB** |

- **O-5 on frames.** `write_regs` never lets userspace choose its privilege: the kernel builds `SPSR_EL1`
  (EL0t, DAIF clear, only NZCV from the caller); on x86_64 `CS`/`SS` are constants, `RFLAGS` takes only
  status flags (IOPL 0, IF set), and a non-canonical `RIP` or base is refused. `SCTLR_EL1.UMA = 0`.
- NMI, `#DB`, `#DF` and `#MC` run on IST stacks, as RFC-0007 (proposed) asks: they can arrive in the
  `syscall` window before `RSP` is switched. AArch64 has no such window.

## 10. FP/SIMD — owned by userspace, enabled per thread, saved lazily (V7)

- **Enabled per thread** by `set_fp`, off by default; the kernel is soft-float, so this state is purely
  user state. A thread with FP off that touches FP takes an ordinary fault, so servers never force a save.
- **Loaded on first use, saved lazily.** Each core records an FP *owner*; every other thread runs with
  access trapped — `CPACR_EL1.FPEN = 0b00` (EC `0x07`, which the reporter already names) or `CR0.TS = 1`
  (`#NM`, vector 7). On that trap, for an FP-enabled thread, the kernel saves the owner's registers,
  loads the new owner's (zeroed on first use), grants access (`FPEN = 0b11`; `clts`) and retries. A `call`
  to an FP-off server saves nothing. State is saved eagerly before a context moves core.
- **AArch64's asymmetry, stated.** No `FPEN` value traps EL1 while allowing EL0 (`0b01` is the reverse),
  so while an owner runs a stray kernel FP instruction would not trap; the soft-float target is the guarantee.
- **Extent.** x86_64 sets `CR4.OSFXSR`, `CR4.OSXSAVE` and `XCR0 = 0b111` — the 832-byte standard-format
  area (512 legacy + 64 header + 256 AVX). AVX-512, AMX and SVE (`CPACR_EL1.ZEN`) stay disabled and fault.
- **The known cost — LazyFP** (CVE-2018-3665; Stecklina & Prescher, 2018): on affected Intel cores a
  non-owner can read the owner's registers speculatively before `#NM` resolves. Hardware side channels are
  out of scope (threat model §7); the cure is Open question 2, costed in §16.

## 11. Interrupts — the controller, and device IRQs as notifications (V8)

The kernel owns the controller and nothing behind it. *(seL4 `IRQControl`/`IRQHandler`; Redox; QNX.)*

- **Objects.** One `IrqControl` (the root task's) mints at most one `IrqHandler` per device line.
  Kernel-reserved lines — the timer PPI, the IPI SGIs or vector, spurious INTID 1023 — are never minted.
  `bind(irq, notification, bits)` needs `WRITE` on both; `ack(irq)` needs `WRITE`. Routing a line to core
  *c* needs the `IrqHandler` and *c*'s `Core`.
- **Delivery.** The kernel acknowledges, **masks the line**, signals end of interrupt, ORs `bits` into the
  notification and wakes a waiter through §6's dispatcher; the driver's `ack` unmasks. A level-triggered
  line cannot storm. A destroyed notification leaves the line masked (fail closed); the `IrqHandler`
  outlives a crashed driver, and a restarted one re-binds (O-17).
- **AArch64: GICv3 first.** System-register CPU interface (`ICC_SRE_EL1.SRE`, `ICC_IGRPEN1_EL1`; acknowledge
  `ICC_IAR1_EL1`; end `ICC_EOIR1_EL1`; `ICC_PMR_EL1` open); SPIs through `GICD_ISENABLER<n>`/`ICENABLER<n>`
  and `GICD_IROUTER<n>`; SGIs and PPIs through the redistributor's `GICR_ISENABLER0`, woken by
  `GICR_WAKER`. Each line the kernel enables is put in Group 1 explicitly, since Group 0 would arrive as
  FIQ. On `virt`: distributor `0x0800_0000`, redistributors from `0x080A_0000` (QEMU `hw/arm/virt.c`).
  Chosen for affinity routing past eight cores and an interface needing no MMIO. *(Arm IHI 0069.)* **Cost:**
  `virt` picks GICv2 at eight CPUs or fewer (`finalize_gic_version_do`), so xtask gains `gic-version=3`;
  the Raspberry Pi 4 in Constitution §6's bring-up order has a GIC-400 (GICv2) — a **known** second driver.
- **x86_64: LAPIC and I/O APIC.** The 8259s are masked and never driven; I/O APIC entries carry the mask
  bit; end of interrupt goes to the LAPIC (x2APIC EOI MSR `0x80B`). **MSI/MSI-X wait for the driver RFC:**
  without IOMMU interrupt remapping a device can aim any vector at any core — O-18's precondition again.
- **Latency.** Interrupts are taken at EL0, in idle or at a preemption point, so latency is §3's worst
  path — which includes RFC-0007 (proposed)'s kernel console write until a userspace UART driver exists.

## 12. Multi-core shape, placement and exclusive cores (V9)

Phase 1 runs `-smp 1`; the shape is fixed now so multicore is not bolted on.

- **Everything scheduling-related is per core.** A thread runs on the core of the context it holds; moving
  a context is `sc_configure` through another `Core`. **The kernel never load-balances.**
- **Cross-core wake-ups** (RFC-0004 §9.2): enqueue *W* on its core and send an IPI — an SGI through
  `ICC_SGI1R_EL1`; a fixed vector through the LAPIC ICR. Direct switch is same-core only.
- **One big kernel lock when SMP lands**, taken on entry *(Peters, Danis, Elphinstone & Heiser, "For a
  Microkernel, a Big Lock Is Fine", APSys 2015)*. **This answers RFC-0003 §14.2:** a table is touched only
  under the lock, so the resolve→check→act window its 2026-07-30 amendment makes a correctness dependency
  holds across the whole entry, with no per-table lock on the fast path.
- **Cores are parked until funded (default-deny).** Secondary cores stay in `boot.s`'s `wfe` loop until a
  context is configured onto them through their `Core`; a core nobody funds never runs anything.
- **SMT placement invariant** (research/0002 Part 6, after L1TF and MDS). Contexts and threads carry a
  placement class set by policy. `sc_configure` refuses a class that differs from any context on the
  funded SMT sibling; `bind_sc` and **donation** refuse a thread of another class — the class travels with
  the context, so a passive server serving two classes needs a thread per class. O(1) at bind and call;
  no gang scheduling. SMT-off is "never fund the sibling"; one class is the vacuous default.
- **Exclusive assignment is a degenerate case, not a mode** (research/0002 Part 6, after Jailhouse): one
  thread, a full context, nothing else on the core. The bitmap has one bit, the timer is never armed, no
  IPI arrives, and device lines are routed elsewhere. No flag: only a `Core` holder can bind another
  context, and one promising exclusivity to a third party transfers the capability, keeping no copy.

## 13. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4 MCS** (Lyons et al. 2018) | contexts as objects; passive servers; `SchedControl` as `Core`; MCP; sporadic refills; `MIN_BUDGET`; timeout faults; `IRQHandler` | own-priority-only passive servers; handlers cached in the TCB |
| **seL4 RFC-14** (Johnston) | the endpoint threshold, its 2 × WCET rule, deferral, one-layer return | budget limits, for now |
| **Heiser & Elphinstone 2016; Fluke 1999** | the event kernel | per-thread kernel stacks |
| **Elphinstone & Heiser 2013** | the running thread unqueued; priority-aware direct switch | lazy scheduling; L4's unconditional switch |
| **QNX Neutrino** | priority inheritance across the message; partitions built on budgets | full inheritance to busy servers; priority-ordered receive |
| **Ford & Lepreau 1994** | the migrating-thread picture of `call` | Mach |
| **Sprunt, Sha & Lehoczky 1989** | the sporadic server | an unbounded replenishment list |
| **Jailhouse** | refusing to share: parked cores, exclusive assignment | static-only partitioning |
| **Peters et al. 2015; Linux** | one big lock; the idle wake-up rule; PSI-style stall counters | fine-grained locking before measurement; dynamic priorities |

## 14. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-7 no unprivileged exhaustion | time only through contexts; `MIN_BUDGET`; eight refills; O(1) expiry return; FIFO endpoint queues; no wait holding a lock; lines masked until `ack` | **discharged for CPU time and scheduler structures**; kernel memory is RFC-0005's half |
| O-4 no ambient authority | no thread runs without a context; priority through MCP; time through `Core`; interrupts through `IrqControl`; only `yield` and `exit` authority-free | **discharged for time** |
| O-5 argument validation | `write_regs` sanitises `SPSR_EL1`/`RFLAGS`, refuses non-canonical `RIP` and bases; every operation takes handles | **discharged for this surface** |
| O-9 no ambient side channel | no EL0 timer or PMU; SMT placement classes; parked cores | **contributes** — fine-grained timing channels stay out of scope |
| O-17 contained drivers | lines as notifications, masked until `ack`; `IrqHandler` outlives a crashed driver; stall counters | **enables** |
| O-18 confined DMA | MSI deferred until interrupt remapping is designed | **named, not discharged** |
| RFC-0004 amendment, 2026-08-01 | (1) threshold at `call`, deferral flagged; (2) `BudgetExpired` on the reply object, plus timeout faults (§7) | **both commitments discharged** |

## 15. Graves checked (§3)

- **Policy in the kernel.** No admission, balancing or budget choice; the one number chosen is 256.
- **The catch-all right.** Priority authority is a relation (MCP); time is per core; interrupts per line.
- **Bolted-on multicore.** Per-core state, IPIs, the lock and placement are stated before any SMP code.
- **Drivers in the kernel; compiled-in unused device paths.** The kernel masks and signals, never services
  a device; one controller and one timer per target; unclaimed lines never enabled.
- **Baroque hierarchies; multi-copy IPC.** Contexts are flat; donation moves a reference to time.

## 16. Costs — what this makes harder

- **Every server needs someone to think about time.** A passive server cannot run until called; an active
  one needs a configured context. "Spawn a thread and it runs" is gone — O-4's price, in time.
- **The one-hop unwind leaves a stranded server to userspace.** The kernel makes the failure visible and
  restarts nothing: a supervisor is required before any passive server on partial contexts is trusted.
- **MCS is seL4's least-verified part**, and sporadic refills are subtle — hence property tests.
- **About 1 KiB per TCB on AArch64 and 1.25 KiB on x86_64**, FP space included for threads that never use it.
- **FIFO endpoint queues** let a high-priority client wait behind earlier low-priority ones.
- **Lazy FP's cure, costed.** Eager restore on a switch into an FP-enabled thread of another process moves
  520 bytes (16 `ldp q` pairs) or an 832-byte `XRSTOR` — 9 or 13 cache lines — and nothing on IPC to
  FP-off servers. Its cycles are a measurement owed, beside a `call` round trip of roughly 380–630 cycles
  (twice research/0001's one-way 188–316).

## 17. Open questions

1. **EL0 counter access.** Proposed default: a per-thread flag set with `WRITE` on the Thread and switched
   with it (`CNTKCTL_EL1.EL0VCTEN`; `CR4.TSD`, written only when it differs). Denial costs a web server a
   kernel entry per timestamp; threat model §7 puts fine-grained timing out of scope. EL0 timers stay off.
2. **LazyFP on x86_64.** Eager restore for cross-process switches into FP threads (costed in §16)?
3. **RFC-14 budget limits.** Worth 21–22% on the fastpath to stop a passive server overspending?
4. **Passive drivers** need a context bound to a notification (MCS); **userspace timers** (RFC-0004 §9.3)
   need kernel deadline objects off the release queue, or a time server. Neither is Phase 1.
5. **Numbers from measurement:** `MIN_BUDGET`, kernel stack size, the worst kernel path; x86_64's clock
   when the TSC is not invariant.

## 18. Implementation increments

Pure logic lands in a new host-tested `no_std` workspace crate, `scheduler/`, generic over a thread index
as `capability/` is over `ObjectRef`; hardware behaviour is proved by QEMU boot-test strings. New `unsafe`
stays in `kernel/src/arch/**`. The x86_64 HAL gains each signature as a documented no-op that returns —
its entry still calls straight through into the kernel proper (CLAUDE.md § Architectures), so the second
build compiles the scheduler. **[demo]** marks what the Phase-1 demo needs.

1. **`scheduler` I — run queues, states, MCP. [demo]** O(1) pick, head/tail re-entry, legal transitions,
   the MCP relation; host tests and an exhaustive transition table. Built `no_std` for both targets in CI.
2. **`scheduler` II — full contexts and billing. [demo]** Round-robin budgets, rounding, stall counters,
   `MIN_BUDGET`; host tests against a shadow model.
3. **GICv3 and the virtual timer, kernel-only. [demo]** xtask's machine line gains `gic-version=3`; the
   kernel's own handler prints `timer: tick` twice — the GIC, the PPI and the idle loop, proved.
4. **Event-kernel dispatch. [demo]** Trap frame in the TCB; two kernel self-test threads (EL1t, on
   `SP_EL0`, behind a `sched-selftest` feature as `provoke-exception` is) alternate under the timer:
   `sched: A B A B`. The boot stack becomes core 0's kernel stack; `aarch64.ld`'s comment changes with it.
5. **EL0 entry and exit** (on RFC-0005's user mappings, RFC-0007's trap stub, both proposed). **[demo]** A
   spinning EL0 thread is preempted anywhere and resumes with its registers intact: `user: still here`.
6. **Thread, SchedContext and `Core` objects; `sc_configure`, `bind_sc`, `resume`, `set_priority`, `exit`.
   [demo]** The root task configures the demo processes; two EL0 threads at equal priority interleave.
7. **IPC blocking states, FIFO endpoint queues, direct switch** (with RFC-0004's endpoint PR). **[demo]**
   Host tests for the switch; the client `call`s an **active** server and RFC-0007's `Kaya!` strings appear.
8. **Passive servers, donation, inheritance.** A passive server runs only when called, at its client's
   priority; a demo stretch goal.
9. **`scheduler` III — partial contexts.** Sporadic refills, merge, release queue. Property test: no
   window of length `T` sees more than `B` consumed.
10. **Threshold, deferral, expiry return, timeout faults.** Host tests; boot-test `BudgetRefused`,
    then `BudgetExpired` and a timeout fault for an overrunning server. Required before partial-context
    passive servers are trusted.
11. **Lazy FP/SIMD.** Two FP-enabled EL0 threads load different markers into `v0`, are preempted in turn
    and each finds its own: `fp: isolated`.
12. **`IrqControl`/`IrqHandler`.** PL011 receive (SPI 1, INTID 33) reaches an EL0 driver as a notification
    when xtask writes a byte to the serial port: `irq: notified`.
13. **x86_64 scheduling HAL** (IDT, TSS, IST, LAPIC, `#NM`, `XSAVE`), after its boot RFC: 3–5, 11 again.
14. **SMP** — secondary cores from the park loop, the big lock, IPIs, placement classes.

**What the demo takes as interim, honestly.** The demo needs increments 1–7 and runs full contexts only.
Its `call` goes to an **active** server, so RFC-0004 §4's accepted `call` semantics — donation — are not
exercised by it; that is increment 8, one more PR, safe to pull in before 9 and 10 because a lent full
context (`B = T`) refills at once and merely re-queues its holder: expiry inside a server needs a partial
context, which the demo never configures. Its userspace stays soft-float, so 11 is not needed. The x86_64
no-op HAL functions are the other interim, removed by 13. Nothing else is smuggled.

## 19. What this unblocks

RFC-0004's implementation gets threads to rendezvous between. The driver framework receives interrupts as
notifications (O-17). The broker gains levers to sell time without the kernel knowing why: `Core`, MCP,
thresholds and stall counters. Next on paper: the x86_64 boot RFC (increment 13), an SMP amendment (14),
and dated notes against RFC-0004 for §7's deviation and reconciliation.
