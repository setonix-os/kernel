<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Prior-art review — capabilities and IPC

| Field | Value |
|-------|-------|
| Status | Living record; informs RFC-0003 (capabilities) and RFC-0004 (IPC) |
| Date | 2026-07-30 |
| Method | Literature and source review against the systems that already solved — or failed at — these problems |
| Cited by | RFC-0003, RFC-0004, and their amendments |

## Purpose

The constitution's method is "keep the proven, prune the legacy": adopt what the field has shown to
work, and refuse the comfortable old mistake. That is only honest if we actually check our design
against the systems and papers that fought these fights first. This note is that check for the two
core RFCs. It records what the literature validates, what it challenges, and — most usefully — where
our own drafts had quietly reinvented something a modern system already tried and abandoned.

Each finding carries a verdict: **VALIDATED** (the field agrees), **CHALLENGED** (the field warns
against it), or **NUANCED** (right with an important qualification). The actionable challenges become
RFC amendments; this note is the evidence they cite.

## Part 1 — Capability systems (informs RFC-0003)

### What the field validates

- **The flat per-process handle table is a named, 60-year-old design point: the *C-list*
  (capability list) / "partitioned capability".** Userspace holds indices; the kernel holds the
  capabilities. Zircon/Fuchsia, KeyKOS, EROS and Mach ports all sit here; seL4's CNode is a
  *structured* C-list, and Unix file descriptors are a degenerate one. Setonix did not invent a design
  point — it picked the Zircon point and imported seL4's discipline onto it, which is a recognised and
  defensible combination.
  *([C-list](https://en.wikipedia.org/wiki/C-list_(computer_security)); [UNSW cs9242 capabilities, Heiser](https://cgi.cse.unsw.edu.au/~cs9242/02/lectures/03-caps/node9.html); [Cornell CS513](https://www.cs.cornell.edu/courses/cs513/2005fa/L08.html))*
- **Subset-only, monotone rights derivation is universal** — seL4 mint/diminish, Fuchsia
  reduced-rights duplicate, and the attenuation assumed throughout Miller/Yee/Shapiro.
  *([Capability Myths Demolished](https://papers.agoric.com/assets/pdf/papers/capability-myths-demolished.pdf))*
- **Handles-as-indices with kernel-held capabilities is the partitioned-capability unforgeability
  guarantee** — identical to Zircon: userspace holds an opaque number, the kernel resolves it.
  *([Fuchsia handles](https://fuchsia.dev/fuchsia-src/concepts/kernel/handles))*
- **No ambient authority / a capability-indexed syscall ABI (RFC-0003 §9) is our best-supported
  choice.** It is the textbook cure for Hardy's *Confused Deputy* (1988): the deputy is confused
  precisely because designation (a path/PID/name) is separated from authority; a capability re-fuses
  them.
  *([Hardy, The Confused Deputy](https://dl.acm.org/doi/10.1145/54289.871709); [GNU Hurd summary](https://www.gnu.org/software/hurd/confused_deputy.html))*
- **Pushing revocation *policy* to userspace is the canonical object-capability answer, not an
  anti-pattern** — the caretaker (Redell 1974) and its generalisation the membrane, as in KeyKOS,
  EROS, E and Genode's `core`. Miller's "Irrevocability Myth" exists precisely to refute "capabilities
  can't be revoked".
  *([Paradigm Regained, Miller](https://pdos.csail.mit.edu/6.828/2004/readings/miller03paradigm.pdf); [Genode capability-based security](https://genode.org/documentation/genode-foundations/20.05/architecture/Capability-based_security.html))*
- **Generation counters are cheaper than the alternative *because* the capability never leaves the
  kernel table.** CHERI, whose capabilities are copyable into user memory, must instead *sweep* all
  memory for stale capabilities on revocation (CHERIvoke, Cornucopia). Our O(1) generation bump works
  only under the invariant that a resolved capability is never observed or cached outside the kernel
  table — a load-bearing property that must be stated, not assumed.
  *([Cornucopia](https://www.cl.cam.ac.uk/research/security/ctsrd/pdfs/2020oakland-cornucopia.pdf); [Cornucopia Reloaded](https://www.microsoft.com/en-us/research/wp-content/uploads/2024/02/revocation3-paper.pdf))*

### What the field challenges

- **Framing seL4's capability *derivation tree* (CDT) as part of the "baroque hierarchies" grave is a
  category error.** The grave is the CNode *radix storage*; the CDT is the separate price of *precise
  revocation*. By rejecting them together, RFC-0003 discarded the very mechanism its own §7 then needs.
  *([seL4 manual](https://sel4.systems/Info/Docs/seL4-manual-latest.pdf))*
- **The revocation design (RFC-0003 §7-B3) is caught between the two real options and picks neither.**
  This is the central finding, and it is a genuine self-contradiction in the draft:
    - A broker can only revoke authority it **interposed at grant time** (the membrane must be in the
      loop when the capability is handed out).
    - But RFC-0003 §6 lets a capability with `TRANSFER` move **peer-to-peer via IPC, bypassing the
      broker**. The broker was never in the loop, so it cannot revoke what it never mediated.
    - For the kernel to revoke a transferred capability instead, it must enumerate capabilities across
      *every* process's table — a **global mapping database with cross-core two-phase commit**, which
      is exactly the cost Barrelfish paid when its flat capability system later needed transitive
      revocation, and exactly the cost our flat table chose to avoid.
  *([Barrelfish capability management TN-013](https://barrelfish.org/publications/TN-013-CapabilityManagement.pdf); [Hille et al., ATC'19](https://www.usenix.org/system/files/atc19-hille.pdf))*
- **"Badges + a one-level parent link" cannot express multi-level transitive revocation.** In seL4 a
  badge supports *one level* of derived children and revoke still recurses via the CDT. A one-level
  link revokes A→B but not A→B→C. So the honest fork is: regrow a CDT (and own its unbounded,
  preemption-point-forcing revoke — seL4's worst-case operations run to *hundreds of thousands of
  cycles*), or commit to userspace membranes (and admit the kernel badge is then nearly vestigial).
  B3 as written risks the worst of both.
  *([Blackham et al., interrupt response with revocation](https://dl.acm.org/doi/pdf/10.1145/2168836.2168869))*

### Consequences for RFC-0003 (folded in by amendment)

1. **The revocation knot must be cut by a *decision*, not a *deferral*.** RFC-0003a chooses one of:
   (a) all revocable authority flows through broker-interposed forwarders at grant time — and
   `TRANSFER` is constrained or routed so it cannot bypass them, leaving the kernel needing *only*
   generations; or (b) the kernel maintains a global capability index / CDT and pays the
   Barrelfish/seL4 revocation cost. "Shallow bookkeeping" is not a third option.
2. **Per-client eviction is a day-one gap, named as such.** Until 0003a lands, the only revocation is
   destroying the whole object for everyone (generation bump). "This app is compromised, cut *it* off"
   is a core pillar-2 requirement, not an edge case; O-3 is only half-discharged and the RFC should say
   so plainly.
3. **State the load-bearing invariants explicitly:** a resolved capability is never observed or cached
   outside the kernel table (the reason a generation bump suffices where CHERI must sweep); the
   generation counter's width and wraparound behaviour (fail-closed, or 64-bit); and the multi-core
   resolve→check→act path holding a reference against concurrent revocation, which is a *correctness*
   dependency of the generation scheme, not a later addition.
4. **The broker is an undesigned single point of authority** — the KeyKOS/EROS "keeper" problem. Its
   own security, availability and revocation-reachability must be a named obligation of the broker RFC.

## Part 2 — Inter-process communication (informs RFC-0004)

### What the field validates

- **Synchronous unbuffered rendezvous is still the consensus for *fast* microkernels** — seL4, QNX,
  Fiasco.OC. seL4's one-way IPC is 188 cycles (ARM11) to 316 (Cortex-A9), "10–20% above the hardware
  limit". Zircon deliberately went async-channel-first, but on *programmability and robustness at
  scale* grounds (retry loops livelocking), not performance — and it pays with the kernel buffering and
  accounting our synchronous choice rightly avoids.
  *([Heiser & Elphinstone, L4: 20 Years, TOCS 2016, §3](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf); [Fuchsia concepts](https://fuchsia.dev/fuchsia-src/concepts/kernel/concepts))*
- **"No kernel timeout; hold a timer capability" (§9.3) is exactly seL4's resolution.** Original L4 had
  four IPC timeouts; they were removed because "there is no theory, or even good heuristics, for
  choosing timeout values… only the values zero and infinity were used." The replacement is a
  poll/block flag plus userspace access to a timer — our proposal precisely.
  *([TOCS 2016 §3.2.4](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf))*
- **`send`/`recv`/`call`/`reply` maps almost 1:1 onto QNX Neutrino** `MsgSend`/`MsgReceive`/`MsgReply`
  and its SEND→REPLY→READY blocking states — a synchronous message-passing model with decades of
  production use.
  *([QNX synchronous messaging](http://www.qnx.com/developers/docs/7.0.0/com.qnx.doc.neutrino.sys_arch/topic/ipc_Sync_messaging.html))*
- **Single-use reply capabilities (`call`) are seL4's exact model** — a reply object pointing at the
  caller, unrelated to the endpoint, one-shot.
  *([seL4 IPC manual source](https://github.com/seL4/seL4/blob/master/manual/parts/ipc.tex))*
- **Endpoints-as-capabilities, the SEND/RECV split, no global registry, and bounded payload-free
  notifications are all seL4's design** — including the exact rule "the moment a notification wants to
  carry data, the answer is send a message".
  *([Heiser, How to use seL4 IPC](https://microkerneldude.org/2019/03/07/how-to-and-how-not-to-use-sel4-ipc/))*

### What the field challenges

- **RFC-0004 §5 "map the sender's pages into the receiver for the duration" is L4's *long IPC* — the
  mechanism seL4, NOVA and Fiasco.OC all removed.** The TOCS paper describes original long IPC as a
  temporary mapping window into the receiver's address space, copied directly — our "map" option almost
  verbatim. It was removed on minimality grounds, and *especially* for a verification-minded kernel: a
  page fault during the in-kernel copy introduces concurrency that "would make the already-challenging
  verification task even harder". The consensus model is the inverse: **establish shared memory once,
  then send a small message/notification over IPC.**
  *([TOCS 2016 §3.2.2](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf); [Heiser, How to use seL4 IPC](https://microkerneldude.org/2019/03/07/how-to-and-how-not-to-use-sel4-ipc/))*
- **The real synchronous-IPC hazard is scheduling coupling, which RFC-0004 underspecifies.** Timeouts
  (correctly dispatched to userspace) are a red herring; the thing that actually bit L4 and drove
  seL4's MCS redesign is *whose scheduling context/priority the callee runs on*. Direct switch must be
  **priority-aware** (switch directly only when it respects priority, else invoke the scheduler), and
  `call` is properly framed as **time-slice / scheduling-context donation** — the migrating-threads
  insight (Ford & Lepreau 1994), QNX's priority inheritance, and seL4's MCS Reply-carries-scheduling
  all say the same thing. A client-held timer capability cannot fix a server monopolising the caller's
  time. This couples IPC to the scheduler earlier than the RFC implies.
  *([TOCS 2016 §4.3](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf); [seL4 MCS](https://docs.sel4.systems/Tutorials/mcs.html); [Ford & Lepreau, migrating threads](https://www.usenix.org/conference/usenix-winter-1994-technical-conference/evolving-mach-30-migrating-thread-model))*
- **Hard-committing the fast path to physical registers x0–x7 is the rigidity L4 engineered away.**
  seL4 uses *virtual* message registers — a few physical, the rest memory-backed — behind one stable
  ABI, adopted specifically for portability and ABI stability. And the in-register payoff is small and
  shrinking: 10% on ARM11 (four-word message), 4% on Cortex-A9, and on modern out-of-order x86
  "reserving any registers… results in an overall loss of performance". Real IPC cost is dominated by
  the mode/context switch, not by how many words ride in registers.
  *([TOCS 2016 §3.2.1, §3.2.3](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf))*
- **The `~100-cycle` target is the fastpath *software logic* only, not a round-trip.** End-to-end
  one-way IPC is 188–316 cycles; a full `call`+`reply` is roughly double, and modern x86 with
  speculative-execution mitigations climbs into the many hundreds. Keep ~100 cycles as an aspiration
  for the logic, or it reads as naïve.
  *([TOCS 2016 Table I](https://www.cs.fsu.edu/~awang/courses/cop5611_s2026/L4.pdf); [SkyBridge, EuroSys'19](https://ipads.se.sjtu.edu.cn/_media/publications/skybridge-eurosys19.pdf))*
- **Reply-capability pitfalls to state:** the reply capability must be destroyed if the caller dies
  (else a dangling one-shot reply); and a server that withholds or deletes the reply blocks the caller
  forever — the class of hazard catalogued in Shapiro's *Vulnerabilities in Synchronous IPC Designs*
  (2003), and the reason the userspace watchdog (§9.3) is not optional.
  *([Shapiro 2003](https://ieeexplore.ieee.org/document/1199341/))*

### Consequences for RFC-0004 (folded in by revision)

1. **Prune §5's page-mapping "long IPC".** Cap the slow path small (an IPC-buffer-sized bounded copy, a
   few hundred bytes — seL4's `seL4_MsgMaxLength` is 120 words). The large-data story becomes
   shared-memory-by-capability established out of band, with IPC/notification carrying only a small
   descriptor. This also strengthens O-7: no bounded-but-large per-message copy to account for.
2. **Make direct switch priority-aware and frame `call` as scheduling-context donation.** State the
   coupling to the scheduler RFC explicitly; this is the real synchronous-IPC correctness issue.
3. **Adopt virtual message registers** behind a stable ABI rather than hard-committing x0–x7, and
   temper the register-budget question with the finding that the payoff is small.
4. **State the reply-capability lifetime rules** (destroyed on caller death; withheld-reply DoS handled
   by the watchdog) and reframe the cycle target as software-logic, not round-trip.

## Part 3 — The Rust-OS state of the art (informs both RFCs)

### By project

- **seL4** is the closest prior art for *both* our core RFCs, and reading its production detail sharpens
  ours: endpoints are capabilities with send-only/receive-only rights and a `GRANT` right that gates
  capability-carrying messages (our exact model); `Call` blocks for a reply through a **one-time reply
  capability**, not the endpoint; and the **register fast path fires only when *no capability is being
  transferred***. That last point is a design fact we had not stated: a message carrying a capability
  is inherently the slow path. seL4's MCS revision made the reply a **first-class object the receiver
  provides**, fixing the old implicit-reply model's timeout and priority problems. Round-trip fast-path
  IPC is ~720–900 cycles on modern x86.
  *([seL4 IPC tutorial](https://docs.sel4.systems/Tutorials/ipc.html); [seL4 performance](https://sel4.systems/performance.html))*
- **Tock** makes capabilities **unforgeable Rust marker types** — zero-sized, compile-time, zero-cost:
  a function requiring authority takes `_c: &C where C: SomeCapability`, and only code permitted to use
  `unsafe` can instantiate one. This validates "a Rust type can be an unforgeable capability token" —
  but only *within* the kernel, where the compiler sees all parties. Not a per-process transferable
  object reference.
  *([Tock capabilities](https://docs.tockos.org/kernel/capabilities/))*
- **Hubris (Oxide)** is production synchronous IPC and corroborates RFC-0004 feature-for-feature:
  synchronous rendezvous, a **single bounded copy** (256-byte cap), non-counting bounded notifications
  (a 32-bit per-task set, `POST` is bitwise-OR) — our notifications exactly. Two lessons stand out.
  First, its **leases** "extend Rust's borrow checker across task boundaries" but the receiver reads
  leased memory **copied, file-like — deliberately *not* mapped**, so a sender crash cannot fault the
  receiver; this is a pointed argument against zero-copy mapping. Second, Cliff Biffle's rationale for
  synchronous-over-async — async "introduces a queueing problem in the last place you want complexity:
  the kernel" — is our §4 argument, from someone shipping it, with no regrets.
  *([Hubris reference](https://hubris.oxide.computer/reference/); [On Hubris and Humility](https://cliffle.com/blog/on-hubris-and-humility/))*
- **RedLeaf (OSDI '20)** is the closest prior art to our move-transfer claim, and the most instructive.
  Its `RRef<T>` moves owned objects across isolation boundaries with no `Clone` and zero copy — but it
  is a **hybrid**: the reference carries **runtime owner metadata and a borrow count**, **trusted
  proxies** mediate every cross-domain call and update ownership at runtime, and **no mutable borrow
  may cross a boundary** (a crash mid-borrow would corrupt state). Even in a single address space with
  all-Rust co-compiled domains, ownership types alone were not enough. The group's 2023 follow-up is
  decisive: "the borrow checker operates within a single compilation unit and cannot enforce guarantees
  across separate protection domains" — safe cross-domain transfer needs **type restrictions + runtime
  tracking + explicit revocation** on top of ownership.
  *([RedLeaf, OSDI '20](https://mars-research.github.io/doc/2020-osdi-redleaf.pdf); [Rust zero-copy across domains, PLOS '23](https://mars-research.github.io/doc/2023-plos-rust-zerocopy.pdf))*
- **Asterinas** ships the most sophisticated Rust rights model: `aster-rights` encodes each right as a
  **type-level marker** (`Dup, Read, Write, …`), `TRights![Read, Write]` builds a compile-time set, and
  a `#[require(R > Read)]` macro turns "calling a method whose right you lack" into a **compile error**.
  Attenuation is a type transform: `Channel::restrict<R1>(self) -> Channel<R1>`. It runs a **dual model**
  — the type-level set for the static case, a runtime `bitflags Rights` for the dynamic one — which is
  a concrete pattern for our own `DUPLICATE/TRANSFER/READ/WRITE/REVOKE`.
  *([aster-rights source](https://github.com/asterinas/asterinas/blob/main/kernel/libs/aster-rights/src/lib.rs); [Asterinas, USENIX ATC '25](https://www.usenix.org/conference/atc25/presentation/peng-yuke))*
- **Theseus (OSDI '20)** is the "trust the type system entirely" extreme — single address space, single
  privilege level, all safe Rust, no capability table, no message-passing IPC. It recovered 11/13
  injected faults to MINIX 3's 0/13, but its own authors concede "it is impossible to write a complete
  OS in 100% safe Rust", and it cannot host untrusted or non-Rust code. A servers-first microkernel
  cannot adopt its model — but its **state-spill** lens (minimise state a service retains from
  interacting with others, for fault recovery) is a useful design discipline.
  *([Theseus, OSDI '20](https://www.usenix.org/system/files/osdi20-boos.pdf))*
- **Redox** — which we already borrow the *scheme model* from — confirms two things. Its authority is
  the **file-descriptor table** ("an open file descriptor is a capability"), not a typed rights-bearing
  cap table, and it is **presently ambient** (path + namespace) and only now *migrating* toward
  capabilities (namespace-as-fd, CWD-as-fd, `O_RESOLVE_BENEATH`). So **our RFC-0003 §9 no-ambient-authority
  rule is stricter than Redox's today** — we forbid ambient syscalls from syscall #1; Redox is still
  getting there. We borrow Redox's schemes, deliberately not its authority model.
  *([Redox security](https://doc.redox-os.org/book/security.html); [NLnet cap/nsmgr/cwd work](https://www.redox-os.org/news/nlnet-cap-nsmgr-cwd/))*
- **Hermit / Kerla / Maestro** — unikernel and monolithic Rust kernels with no relevant capability
  model; boundary cases only.

### Verdicts on the two claims

- **"Capability transfer is a Rust move, modelled at compile time" — NUANCED; the slogan overreaches.**
  The owned, no-`Clone` `Capability` genuinely makes the borrow checker enforce single-ownership *inside
  kernel code*, and RedLeaf's `RRef` is direct prior art. But the constitution §3 / RFC-0003 §6 phrasing
  "models capability transfer at compile time" is false for the *userspace-observable cross-process*
  transfer: the compiler sees one compilation unit, our processes are separately compiled and
  hardware-isolated, and the transfer is a runtime table operation — our generation counter is itself
  proof we depend on runtime checks. Keep the design; scope the claim to kernel-internal assurance, and
  cite RedLeaf and PLOS '23 pre-emptively.
- **"Synchronous IPC: register fast path, endpoints-as-capabilities, call/reply, bounded notifications"
  — VALIDATED.** seL4's model feature-for-feature, independently corroborated by Hubris in production.
  The refinements to fold in (not objections): reply as a first-class one-time reply object (seL4 MCS),
  and the register fast path defined as **cap-transfer-free** by design.

### The meta-observation

Every project that extracts the most from Rust's type system for capabilities — Tock, Theseus,
Asterinas, RedLeaf — does so by **collapsing the address-space boundary** so the compiler can see all
parties. Setonix's servers-first, hardware-isolated, possibly-non-Rust userspace is precisely the
setting where that leverage is weakest. This is not a weakness in our plan; it is the *reason* the
runtime capability table (handles, generations, table-to-table move) is the correct spine, and the
Rust-type-system story should be positioned as kernel-implementation assurance, never as the capability
model itself.

## Synthesis — keep, prune, adopt

The spine of both RFCs is validated. The corrections are specific and bounded.

**KEEP (validated, well-cited):** the flat C-list capability table; subset-only monotone rights;
handles-as-indices unforgeability; generation-vs-ABA; no ambient authority and the capability-indexed
syscall ABI (our single strongest, best-supported choice); synchronous unbuffered rendezvous;
endpoints-as-capabilities with the send/recv split; bounded payload-free notifications; no kernel
timeout with a userspace timer capability; the owned no-`Clone` `Capability` as kernel hygiene.

**PRUNE (a modern legacy we nearly re-adopted):** RFC-0004 §5's "map the sender's pages into the
receiver for the duration" — this is L4's *long IPC*, removed by seL4/NOVA/Fiasco.OC and actively
hostile to verification. Replace with shared-memory-by-capability set up out of band plus a small
descriptor over IPC; cap the in-message slow path small.

**ADOPT (proven refinements):**

1. Scope the Rust-move claim to kernel-internal assurance (Constitution §3 slogan + RFC-0003 §6),
   citing RedLeaf `RRef` and PLOS '23. *(Constitution §3 is the maintainer's to amend — flagged, not
   taken.)*
2. Make the reply a **first-class one-time reply capability/object** (seL4 MCS), destroyed on caller
   death; state the withheld-reply DoS and its userspace-watchdog answer.
3. Define the register fast path as **cap-transfer-free**: a capability-carrying message is the slow
   path by design (seL4).
4. Use **virtual message registers** behind a stable ABI rather than hard-committing x0–x7 (seL4
   portability lesson); reframe the ~100-cycle figure as software-logic, not round-trip.
5. Make direct switch **priority-aware** and frame `call` as **scheduling-context / time-slice
   donation** (migrating threads → QNX priority inheritance → seL4 MCS); couple to the scheduler RFC.
6. Cite seL4 **badges** by name in RFC-0003 §7 (B3 already reinvents them), and evaluate **Asterinas
   `aster-rights`** — type-level rights with a `require(R ⊇ X)` macro for the kernel's internal
   handling, keeping the runtime bitmask at the userspace boundary (a dual static/dynamic model).

**DECIDE (the one thing research forces, not defers):** the revocation knot. RFC-0003a must choose
between broker-interposed forwarders at grant time (with `TRANSFER` constrained so it cannot bypass
them) and a kernel-maintained global capability index / CDT (with its Barrelfish/seL4 costs). Our
current "badges + shallow record, defer to broker" is contradicted by §6's broker-bypassing transfer
and cannot express multi-level transitive revocation. Until then, per-client eviction is a named
day-one gap: the only revocation is destroying the whole object for everyone.
