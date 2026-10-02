<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0007 — System calls and process bootstrap

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 (capabilities are the only authority; the Language clause) and §4 (a Borrow Ledger reading, §12); `CLAUDE.md` § `unsafe` policy and § Layout; threat model O-6's text; RFC-0003 (a dated amendment: the handle word, a slot-generation bound, `free_slots`, one error variant split); RFC-0004 §8 (a dated amendment: reply objects are unbound, not destroyed); `docs/CHANGELOG.md`; the workspace (`user/`), the comments in `Cargo.toml` and `xtask/Cargo.toml`, `kernel/src/main.rs`'s grep scope; `xtask`, `kernel/build.rs` and `aarch64.ld`; the kernel's trap path |
| Depends on | RFC-0003 (accepted; §9 binds every syscall); RFC-0004 (accepted; §5, §6, §8, §9.4, §10 and the 2026-08-01 amendment); RFC-0005 and RFC-0006 (proposed, drafted alongside); a future x86_64 boot RFC for the UEFI stub |
| Discharges | O-5 (argument validation) for the syscall surface; O-27 (explicit authority at spawn); binds O-1, O-2 and O-4 at B1; keeps O-6 Built with a third designated tree; contributes to O-7, O-9 and O-13; designs O-17's fault path |

> **Proposed verdicts.**
>
> 1. **Uniform invocation:** every kernel-object operation is a `call` on its capability; twelve syscalls in all (§4).
> 2. **Method arguments in registers:** handle words in MR0–MR3 are *presented* — resolved, checked, never moved — unless marked *moved* (§4).
> 3. **Reply objects are reusable:** `reply`, budget expiry and caller death unbind them — a dated RFC-0004 amendment (§4).
> 4. **Registers:** AArch64 `svc #0`, number in `x8`; x86_64 `syscall`, number in `rax`; threads start with zeroed registers but four (§3).
> 5. **Messages:** four physical and 64 virtual message registers, three capabilities, an optional 1 KiB IPC buffer, fetched once at rendezvous (§5).
> 6. **IPC-buffer discovery:** a kernel-set, user-read-only register — `TPIDRRO_EL0`; the user GS base with a self-pointer at `gs:[0]` (§5).
> 7. **Handle word:** 20-bit index, 44-bit slot generation; word `0` is null; recorded as a dated RFC-0003 amendment (§6).
> 8. **Errors in two layers:** numbered transport status in a register, method result in the reply label; faults are not errors (§7).
> 9. **Process object:** table, one AddressSpace for life, threads; born empty; `GRANT` only until the first `resume`; then `WRITE` means `KILL` (§8).
> 10. **One root task,** built once from a load descriptor, every boot capability listed in BootInfo, no "is root" branch anywhere (§9).
> 11. **Boot images:** `arch::boot_modules()` yields descriptors a host-side ELF reader made; the kernel parses no ELF; AArch64 embeds, as an interim (§10).
> 12. **Console as a capability:** one `WRITE` of at most 24 bytes that never waits; deleted when the Phase-2 UART driver lands (§11).
> 13. **`user/abi`** is the microkernel-core row; its `src/arch/**` — trap instructions and entry stubs only — is the third `unsafe` tree (§12).

## 1. The question

**How does userspace ask the kernel to do anything, and how does the first userspace come to exist —
such that every request names a capability, no argument is trusted, and no process is born holding
authority it was not explicitly handed?**

RFC-0003 §9 fixed the rule and RFC-0004 §10 the IPC semantics; both left the encoding here. This RFC
designs it, the trap, the process as a kernel object, and the bootstrap seam RFC-0003 §14.4 named. It does
not design address spaces (RFC-0005), threads (RFC-0006), or any policy on which programs run holding what.

## 2. Which pillar

**Pillar 2 and the doctrine that capabilities are the only authority.** The syscall surface is the whole
of B1; one entry naming a resource without a handle makes RFC-0003's table decorative. Spawn is where
research/0002 found the freshest scar (Part 4: runc CVE-2024-21626, one inherited descriptor became host
traversal), and O-27 exists because of it. Boot images reach userspace read-only (pillar 1).

## 3. Entering the kernel

**AArch64 — `svc #0`** traps from EL0 to `VBAR_EL1 + 0x400` with `ESR_EL1.EC = 0x15`, the immediate in
`ISS[15:0]`, `ELR_EL1` past the `svc`, DAIF masked and `SP_EL1` selected *(Arm ARM DDI 0487)*. The stub
saves `x0`–`x30`, `SP_EL0`, `ELR_EL1` and `SPSR_EL1` into the thread's frame (RFC-0006 (proposed) §9 owns
the layout) and dispatches on EC: `0x15` is a syscall, anything else a *fault* (§7). A non-zero immediate
answers `InvalidArgument`, so a later ABI revision is detectable. `eret` uses an `SPSR_EL1` the kernel
builds: EL0t, DAIF clear, NZCV from the frame, `DIT` and `SSBS` where implemented (the Cortex-A72 has
neither), every other bit zero.

**x86_64 — `syscall` / `sysretq`.** `syscall` needs `IA32_EFER.SCE`, loads `RIP` from `IA32_LSTAR`, leaves
the return `RIP` in `rcx` and `RFLAGS` in `r11`, clears the `IA32_FMASK` bits (here IF, DF, TF and AC) and
does **not** switch stacks *(Intel SDM Vol. 3A, "Fast System Calls in 64-Bit Mode")*. As RFC-0006 §9
requires, the stub runs `swapgs`, parks the user `rsp` in per-core scratch, loads the TCB frame pointer,
saves into the frame, and only then loads the per-core kernel stack. **The exit uses `sysretq` only when
`rcx` is canonical and the frame's `RFLAGS` has TF and RF clear and IOPL 0, else `iretq`:** a
non-canonical `rcx` makes Intel's `sysretq` fault *in ring 0* on the user's stack (CVE-2012-0217), and
`sysretq` sets `RFLAGS` to `r11 AND 3C7FD7H OR 2` (SDM Vol. 2B, SYSRET), which passes TF — Linux takes
`iretq` for TF or RF likewise (`arch/x86/entry/entry_64.S`). RFC-0005's unmapped top user page and
RFC-0006's sanitising `write_regs` already prevent both; the exit checks anyway. **Rejected:** `sysenter`,
absent on AMD in long mode; an `int` gate, a full IDT delivery per call.

| Role | AArch64 | x86_64 |
|------|---------|--------|
| syscall number (in) | `x8` | `rax` |
| invoked handle (in) / status (out) | `x0` | `rdi` in, `rax` out |
| message tag (in/out) | `x1` | `rsi` |
| MR0–MR3 (in/out) | `x2`–`x5` | `rdx`, `r10`, `r8`, `r9` |
| reply-object handle (in) | `x6` | `r12` |
| error detail (out) | `x7` | `r13` |
| clobbered by hardware | — | `rcx`, `r11` |

Other registers are restored from the frame and result registers carry a result or zero, so no kernel
value leaves in a register (O-9). `x8`/`rax` follow Linux; the x86_64 column avoids `rbx` and `rbp`, which
Rust inline assembly cannot name. **No argument is a pointer** (RFC-0005 (proposed) §11 owns the rule): only
the IPC buffer is touched, by frame (§5) — on the Cortex-A72, which lacks PAN, the whole defence.

**Every thread starts** at its entry PC with SP as given (16-byte aligned) and two start values in `a0`,
`a1` (`x0`/`x1`, `rdi`/`rsi`); every other general register zero; EL0t with DAIF clear, or ring 3 with
`RFLAGS` = `0x202`; FP/SIMD trapped until RFC-0006 enables it; `TPIDR_EL0` and `FS_BASE` zero. The entry
symbol is an `abi` assembly stub that calls a Rust `extern "C" fn`, so AAPCS64 and SysV each see their
entry alignment. The trap wrappers are single-instruction `asm!` intrinsics (Open question 4).

## 4. The syscall set

**Options.** **A — one syscall per operation** *(Zircon, Linux)*: every object type grows the table and no
server can stand in for a kernel object. **B — uniform invocation** *(seL4; KeyKOS)*: a handful of IPC
syscalls, a kernel-object operation being a `call` on its capability with the method in the tag's label,
answered as a server would answer. **C — B plus a separate `invoke`**: callers could tell a kernel object
from a server, which removes B's one advantage.

**Verdict sought: B,** for interposition: a Console and a logging server's endpoint are invoked alike, so
Phase 2's UART driver replaces the Console with no client changing. **The claim is scoped:** a server can
stand in for a kernel object whose methods take no handle-word arguments — the Console, and every protocol
a server defines — but not for `map` or `bind_sc`, whose extra handle word means something only in the
caller's table (§16). So `cap_rights` reports rights, never the object's kind.

| # | Syscall | Handles | Beyond the tag and message registers |
|---|---------|---------|--------------------------------------|
| 0, 11 | `yield`, `thread_exit` | — | authority-free (RFC-0003 §9); `thread_exit` leaves the caller `Exited` |
| 1, 2 | `send`, `nb_send` | object | endpoint: send, blocking or failing `WouldBlock`; notification: signal |
| 3 | `call` | object | endpoint: send and await the reply; kernel object: invoke a method, donating nothing |
| 4, 5 | `recv`, `nb_recv` | endpoint or notification; reply object | receive, blocking or polling; a bound notification may wake it |
| 6 | `reply` | reply object | reply once to the bound caller |
| 7 | `reply_recv` | endpoint; reply object | `reply` then `recv` in one entry — the server loop *(seL4 `ReplyRecv`)* |
| 8 | `cap_derive` | object, with `DUPLICATE` | in: MR0 the rights wanted, MR1 a badge (reserved, zero); out: MR0 the new handle word |
| 9, 10 | `cap_close`, `cap_rights` | object | the handle is dead on return; out: MR0 the rights bits |

The `cap_*` calls touch only the caller's table and build `Rights` from the crate's named constants,
refusing undefined bits. **A signal sets bit 0** of a notification until badges exist (RFC-0003a), then the
capability's badge, fixed at derivation *(seL4)*: no sender chooses bits, so it stays RFC-0004 §8's
payload-free doorbell; an `IrqHandler` ORs its bound bits (RFC-0006 §11). **Rejected:** a `debug_putc`
(§11); anything taking a PID, path or address; seL4's depth-addressed pointers, made for CNode trees.

**Reply objects** are created once from a Pool and reused *(seL4 MCS)*: `recv` binds one to a caller, and
`reply`, budget expiry or the caller's death **unbinds** it; `reply` on an unbound one fails `PeerGone`.
RFC-0004 §8 says it "is destroyed if the caller dies", which would let a client that calls and exits in a
loop cost the server a Pool allocation per request — an O-7 drain — so RFC-0004 needs a dated amendment, which
RFC-0006 (proposed) §4 and §7 already follow.

**Kernel-object methods.** A method is a `call` whose label names it and whose MR0–MR3 carry every
argument; one needing more is split. A handle word there is **presented**: resolved in the caller's table,
checked for its row's right, never moved, never needing `TRANSFER` *(seL4's `extraCaps` on an
invocation)*. A handle word marked **moved** leaves the caller's table, which always needs `TRANSFER`
(RFC-0003 §6). `tag.caps` is zero on every method (`InvalidArgument`): `caps[]` belongs to endpoint
messages, whose receiver needs its new handle words delivered to memory. Replies carry new handle words
from MR0; labels count from 1 per kind; the `abi` data half (increment 2) is the table of record.

| Invoked (right) | Label: method | MR0 | MR1 | MR2 | MR3 | Reply |
|-----------------|---------------|-----|-----|-----|-----|-------|
| Pool (`WRITE`) | 1 `region_create` | pages | alignment, pages | max extents | rights | Region |
| Pool (`WRITE`) | 2 `as_create` · 3 `process_create` | rights · AddressSpace, presented `WRITE`, bound for life | — | — | — | AddressSpace · Process |
| Pool (`WRITE`) | 4 `thread_create` | Process, presented `WRITE` | its AddressSpace, presented `WRITE` | IPC-buffer address, or 0 for none | — | Thread |
| Pool (`WRITE`) | 5 `sc_create` · 6 `endpoint_create` · 7 `notification_create` · 8 `reply_create` | — · `min_budget` in ns (RFC-0006) · — · — | — | — | — | the new object |
| AddressSpace (`WRITE`) | 1 `map` · 2 `unmap` · 3 `protect` (RFC-0005 §8) | Region, presented · vaddr · vaddr | the space's Pool, presented `WRITE` · — · perms | vaddr | first page (bits 0–31), pages (32–41), perms (48–49) | — |
| Region (`WRITE`) | 1 `region_copy` (RFC-0005 §8) | source Region, presented `READ` | source page | destination page | pages, at most 16 | — |
| Process (`WRITE` / `READ`) | 1 `GRANT` · 2 `KILL` · 3 `INFO` (§8) | handle words, **moved**, count in `tag.length` | | | | `GRANT`: words in the child; `INFO`: counts |
| Thread (`WRITE` / `READ`; RFC-0006 §4) | 1 `write_regs` · 2 `read_regs`, a four-word group of RFC-0006's frame in label bits 8–15, group 0 being PC, SP, `a0`, `a1` | `write_regs`: the group's four words in MR0–MR3 | | | | `read_regs`: the group |
| Thread (`WRITE`; RFC-0006 §4) | 3 `resume` · 4 `suspend` · 5 `bind_sc` · 6 `unbind_sc` · 7 `set_priority` · 8 `set_mcp` · 9 `bind_notification` · 10 `unbind_notification` · 11 `set_fault_ep` · 12 `set_timeout_ep` · 13 `set_fp` | 5: SchedContext, presented `WRITE` · 7, 8: `auth`, presented, no right checked · 9: Notification, presented `READ` · 11, 12: endpoint, **moved** · 13: on or off | 7, 8: the value | — | — | — |
| SchedControl (RFC-0006 §5) | 1 `sc_configure` (`WRITE`) · 2 `ctl_info` (`READ`) | SchedContext, presented `WRITE` | budget, ns | period, ns | refills (bits 0–7), class (8–15, zero) | `ctl_info`: counts |
| SchedContext (`READ`) · Console (`WRITE`) | 1 `sc_stats` · 1 `WRITE` (§11) | — · byte count, at most 24 | · bytes | · bytes | · bytes | counters · bytes accepted |

`IrqControl` and `IrqHandler` (RFC-0006 §11) follow the same rule, labelled when built. Endpoints and
notifications have no methods: they are what `send`, `call` and `recv` act on.

## 5. Messages, the register budget and the IPC buffer (RFC-0004 §9.4)

The **tag** is one word: bits 0–6 length in words (0–64); bits 7–8 capabilities (0–3); bit 9 set by the
kernel, on a returned tag only, when a bound notification woke a `recv` (MR0 then holds the bits); bits
16–63 a 48-bit label — user-defined for endpoints, the method for kernel objects, its result on a kernel
reply *(seL4's `seL4_MessageInfo`)*. **Bits 9–15 must be zero in every tag userspace supplies**, so no
sender can forge the notification flag. **Four physical message registers on both architectures:** seL4
uses four, research/0001 found the in-register payoff small and shrinking (10% on ARM11, negative on modern
x86), and a per-architecture count would make a message fast on one Tier-1 target only. Up to **64 words**
spill MR4–MR63 to the buffer — below seL4's 120 on purpose: 512 bytes bounds the non-preemptible copy, and
RFC-0004 §5 sends bulk through shared regions. **Three capabilities**, a 2-bit field with no invalid
encodings. **Rejected:** six physical registers on AArch64; seL4's 120 words.

**The IPC buffer** is 1 KiB and 1 KiB-aligned, so it never straddles a page: `#[repr(C)]` words 0 (its own
user address, written by the kernel), 4–67 (MR0–MR63, the first four never read) and 68–70 (`caps[]`), the
rest untouched. **It is optional and reached by frame.** `thread_create` takes its user address, or 0; the
kernel checks it against the AddressSpace's mapping list (RFC-0005 (proposed) §7) — aligned, inside a
read-write mapping — then binds that page of the backing Region, a counted reference and a writable use for
W^X, and reaches it only through the physmap. The address is never dereferenced. A syscall needing a buffer
(a length over four, or any capability) from a thread with none, or a severed one, fails `NoBuffer`.

**Discovery: a kernel-set, user-read-only register.** The kernel loads the buffer's address into
`TPIDRRO_EL0` (EL0 reads, only EL1 writes) and, on x86_64, into the user GS base with `CR4.FSGSBASE` off,
`abi` reading the self-pointer at `gs:[0]` — the ELF TLS convention of a block whose first word is its own
address *(Drepper, "ELF Handling For Thread-Local Storage")*. RFC-0006 restores both on every switch.
**Rejected:** `TPIDR_EL0` and `FS_BASE`, which a relibc port needs for TLS; seL4's thread-local
`__sel4_ipc_buffer` (libsel4 `functions.h`), which needs TLS first.

**Transfer and single fetch (O-5).** Another thread can rewrite a buffer mid-syscall, so every buffer word
the kernel acts on is read **exactly once** into kernel memory before any check depends on it; no Rust
reference is formed into it, and payload moves through RFC-0005's volatile-copy module, never interpreted
*(research/0002 Part 5, virtio's double fetch)*. A blocked sender's tag and MR0–MR3 wait in its frame;
**the rest is fetched at rendezvous, in one kernel entry** *(seL4)*: `caps[]` read once, every word resolved
and checked for `TRANSFER`, the receiver's free slots counted exactly by the crate's new `free_slots()` (a
retired slot is neither free nor live), and only then any move. A failure moves nothing, completes the
sender with its status and leaves the receiver waiting. **Rejected:** removing the capabilities at entry
into the sender's TCB — in-flight state per thread, and a refused delivery could not restore their slots.

## 6. Handles in one register

RFC-0003's `Handle` — a `u32` index and a 64-bit generation — fits no register. The ABI word is
**`slot_generation << 20 | index`**: 2^20 slots, Linux's default `fs.nr_open` (1,048,576); 2^44 slot
generations, about two hundred days of one close-and-reopen per microsecond on one slot. **Word `0` is
never live**, because `Generation::FIRST` is 1: it is the null handle. Separately, **the width is the
invariant RFC-0003's amendment asked to be stated:** only the slot generation crosses the ABI, bounded at
2^44 − 1; a slot whose next generation would pass it retires through the crate's `Retired` state, never
wrapping. Object generations stay in kernel memory at 64 bits. Rights travel in RFC-0003's bit order —
`DUPLICATE` 0, `TRANSFER` 1, `READ` 2, `WRITE` 3, `REVOKE` 4 — and RFC-0005's `EXECUTE` 5, pinned in `abi`
by a `const` assertion. **Rejected:** two registers per handle (halves the budget); 32/32 (seventy minutes
at the same rate); a 16-bit index (too few for server workloads).

**The crate changes, as one dated RFC-0003 amendment** logged in `docs/CHANGELOG.md`, so the width lives
in RFC-0003: `Handle::to_word`/`from_word`, `MAX_SLOT_GENERATION`, a `const` bound of `N` by 2^20 and
`free_slots()` (§5); later, with destruction (increment 14), a split so a destroyed object reports
`ObjectDestroyed` rather than the holder's own use-after-close as `StaleGeneration`.

## 7. The error model

**Transport status** (`x0`/`rax`) is what the syscall layer found, identical for a kernel object or a
server. Every failure is atomic. The detail register names the culprit: 0 the invoked handle, 1–4 a handle
word in MR0–MR3, 5–7 `caps[0..2]`, 8 the tag, 9 the reply handle, 10 the IPC buffer.

| # | Status | Meaning |
|---|--------|---------|
| 0 | `Ok` | done |
| 1 | `InvalidSyscall` | no such number — a closed error, not a fault: the surface stays total |
| 2 | `InvalidArgument` | a reserved bit, a length over 64, a non-zero `svc` immediate, an undefined rights bit, `tag.caps` on a method, a bad buffer address |
| 3 | `InvalidHandle` | word 0, index out of range, empty slot, stale slot generation: the handle names nothing *of yours* |
| 4 | `Revoked` | the slot is live but its object was destroyed (from increment 14; nothing is destroyed before it) |
| 5 | `WrongType` | live handle, wrong kind of object |
| 6 | `InsufficientRights` | lacking the right the operation needs |
| 7 | `TableFull` | a moved capability found no free slot in the receiving table; nothing moved |
| 8 | `WouldBlock` | `nb_send` or `nb_recv` found no partner |
| 9 | `PeerGone` | the object or party waited on went away, or a `reply` found its object unbound |
| 10 | `BudgetRefused` | a `call` whose context can never meet the endpoint's `min_budget`, or a `send` — which donates nothing — to an endpoint that has one |
| 11 | `BudgetExpired` | the donated budget expired inside the server; the kernel completed the wait on the reply object |
| 12 | `NoBuffer` | the syscall needs an IPC buffer the thread does not have |

`BudgetRefused` follows RFC-0006 (proposed) §7, which refuses only a context that can *never* pass — its
amendment A2 to RFC-0004's "refused up front"; the status stands either way. **Method results** (the reply
label) share one vocabulary for kernel and server, so a stand-in reports errors alike *(seL4's reply
label)*: the numbers above, then 13 `InvalidMethod`, 14 `PoolExhausted`, 15 `Fragmented` (RFC-0005) and 16
`WindowClosed` (§8).

**Faults** — EL0 exceptions that are not syscalls — are not return values: the thread is suspended and a
fault message goes to its `fault_ep` (RFC-0006 §4), so a supervisor can restart a driver (O-17).
**Interim, until increment 11:** the kernel prints one line (thread, `ESR`, `ELR`, `FAR`) and leaves the
thread `Inactive`. **Its cost:** an **ambient output channel** — a thread holding no Console can put two
chosen words on the operator's line by faulting, once per thread it can create, each line about 100 bytes
of kernel UART time. It builds only under a `report-user-faults` feature `xtask` enables for Phase-1
boot-tests and `run-qemu`, and it qualifies the O-4 and O-9 rows (§14).

## 8. The Process object and spawning (O-27)

A **Process** holds one RFC-0003 table, one AddressSpace (RFC-0005) and its threads (RFC-0006) — a real
object *(Zircon)*, not seL4's thread naming a CSpace and VSpace. For first authority, **A — inherit**
*(Unix `fork`/`exec`)* is **rejected** by O-27 by name; **B — one bootstrap handle** *(Zircon's
`zx_process_start`)* makes a parent serve its child a protocol over an unbuffered rendezvous before the
child can act; **C — explicit grants before start** *(seL4's root task writing a child's CSpace)* is the
**verdict sought**. `process_create` **binds its AddressSpace for life** — one already bound is refused, so
two tables never share one memory (O-8) — and the Process is born with an **empty table** and no thread.

| Method | Right | Effect |
|--------|-------|--------|
| `GRANT` | `WRITE`; `TRANSFER` on each moved capability | before the first `resume` only: move up to three capabilities in, all or nothing; the reply carries their handle words *in the child* |
| `KILL` | `WRITE` | stop every thread, drop the table, release the address space; the generation bump makes every capability to the process inert |
| `INFO` | `READ` | live capabilities in the table; threads not `Exited`; whether the window is open |

**The grant window** closes at the first `resume` of any of its threads, which RFC-0006 reports by
flagging the Process; `GRANT` then answers `WindowClosed`, and authority arrives only by IPC, which the
process consents to by receiving. It is **single-use** in research/0002's sense (Part 3, after Vault's
response-wrapping) but lacks **prior-use detection**, which is the broker RFC's.

**What the window closes, exactly: capability injection.** Once started, no holder of the Process
capability adds to its table, and `WRITE` on it means `KILL` alone, so a supervisor can hold kill-and-
inspect authority without the child's power (O-17). It does **not** close **control**: `WRITE` on the
child's AddressSpace maps into it and, beside `WRITE` on the Process, creates threads in it; `WRITE` on a
Thread rewrites its registers (RFC-0006 §4). That is authority over the child's code and data, which its
loader always had — and why thread creation presents the AddressSpace: a Process capability alone cannot
start code in a child.

**The spawn protocol**, encoded and host-tested in increment 9's loader plan: the spawner never maps the
child's writable memory into itself — data arrives by `region_copy` (§10); before `resume` it grants the
child whichever of its AddressSpace, Thread and Region capabilities the child needs, closes the rest, and
keeps only the Process capability. A spawner that derived copies first keeps control; the kernel cannot
tell, and the child cannot yet check (Open question 3).

**Lifetime.** A Process ends by `KILL`, or when no capability names it and none of its threads can run
again. One whose threads have all `Exited` is dead but intact, inspectable through `INFO` until then
*(seL4: objects live until destroyed)*. **Rejected:** ending with the last thread *(Zircon)* — undefined for
a newborn, and it destroys what a supervisor would inspect. The spawner passes handle words in `a0` and
`a1` (the demo's Console and endpoint), or a start Region in `a0` — a runtime convention, not kernel ABI.

## 9. The root task and boot info

The one act the kernel performs unasked: at boot it builds **exactly one** process from the first boot
module and gives it every boot capability *(seL4's root task; research/0002 Part 3, "the bootstrap seam is
a one-time kernel act")* — an AddressSpace, the segments mapped as in §10, a stack, a read-only
**BootInfo** page, a Process and a Thread in §3's entry state with `a0` = BootInfo's address and `a1` = 0.
Its priority, ceiling and full context are RFC-0006 (proposed) §6's one bootstrap act.

**BootInfo**, in `user/abi`, is a header (magic, version, length), one `#[repr(C)] BootCap { kind: u32,
flags: u32, handle: u64, base: u64, size: u64 }` per initial capability, and a string table for module
names, pinned within a page by a host test. Every initial capability is *listed*, none at a well-known
slot, so a reviewer can enumerate the whole grant. Kinds: the root's Process, the Console (§11), a
read-only Region per further module with its descriptor; RFC-0005 adds the AddressSpace, the one root Pool
and the DTB; RFC-0006 the Thread, a `SchedControl` per core and the `IrqControl`. Kinds appear as built.

**The root is not a superuser.** Its capabilities are ordinary, droppable and generation-checked, and **no
kernel code branches on "is this the root task"**. They are **not revocable** until RFC-0003a, though
research/0002 Part 3 asks for revocable init capabilities. **Rejected:** a kernel boot manifest (policy in
the kernel); a `spawn_from_module` syscall (a loader as a kernel service); a `system:masters` superuser.

## 10. Getting the first images into memory

**Verdict sought — the mechanism:** the kernel reaches modules through `arch::boot_modules()`, which yields
fixed-shape **load descriptors** made on the host, and **never parses ELF**. A dependency-free, host-tested
ELF64 reader in `xtask` emits an entry point; text (`R+X`), optional read-only (`R`) and data (`R+W`,
zero-padded to a page, with a `bss` size) segments, page-aligned in the user half; and stack and BootInfo
addresses. It **refuses** a writable-and-executable, overlapping or unaligned `PT_LOAD`, `PT_DYNAMIC`,
`PT_INTERP`, `PT_TLS`, `PT_GNU_RELRO` and an executable `PT_GNU_STACK`, ignoring `PT_NOTE` and a
non-executable `PT_GNU_STACK`; `user/link/<arch>.ld` declares `PHDRS` so lld emits no other. W^X is checked
before the image exists (O-13), and the kernel validates one fixed structure — Nexen's finding that kernel
code validating structures built elsewhere is the bug farm (research/0002 Part 7). **Rejected:** an
in-kernel `elf/` crate, a parser in the TCB for no gain.

**The source — an AArch64 interim: embedded in the kernel image.** QEMU's `-initrd` and `-device loader`
add a second, QEMU-only artefact (`hw/arm/boot.c`, re-read at the pinned 11.0.3, loads `-initrd` only for
Linux images and silently ignores it for a bare ELF); children linked into the root's ELF *(seL4's CPIO
archive)* would tie the root to each image. So `xtask` writes `payload.bin` — a versioned header, then per
module a name, descriptor and segment bytes — named by `SETONIX_PAYLOAD`; `kernel/build.rs` re-runs on that
variable and substitutes an empty bundle when it is unset, so a bare `cargo build` prints `boot modules:
0`. A `global_asm!` `.incbin` under `kernel/src/arch/aarch64/` embeds it; `aarch64.ld` places it in a
page-aligned, read-only, `KEEP`ed `.payload` section *(Hubris's `xtask dist`)*. **Cost:** a user change
relinks the kernel. **On x86_64**, a future x86_64 boot RFC provides the UEFI stub that loads the same
bundle as a file; increment 12 depends on it.

**Loading.** The kernel loads **module 0 only**; the others reach the root as **read-only Regions**
(RFC-0005 §7) with descriptors in BootInfo *(Genode's core hands init boot modules read-only)*. Every
loader, kernel or root, maps text `RX` and read-only data `R` **in place**, copies data pages into a fresh
Region with `region_copy` — RFC-0005 (proposed) §8's page-granular, frame-to-frame copy, the one the kernel
already makes for the root — and takes
`bss` as fresh zeroed pages, so no loader maps a child's writable memory into itself. The programs are soft-float `no_std` crates on `abi` alone with no `unsafe`;
on `x86_64-unknown-none` `xtask` passes `-C relocation-model=static -C code-model=small`.

## 11. The console as a capability

Userspace reaches the kernel's UART only through a **Console** object with one method, `WRITE`: the byte
count in MR0, at most 24 bytes in MR1–MR3, sent verbatim (`abi`'s writer supplies `\r\n`). **It never
waits:** the kernel stores bytes while the PL011's transmit-full flag is clear and returns the count
accepted, and `abi` yields and retries the rest, so any waiting happens at EL0, preemptibly. The hold is at
most 24 flag reads and 24 stores, inside RFC-0006 (proposed)'s measured worst path, so the console sets no
floor under `MIN_BUDGET` and needs no preemption point. The root receives it with `DUPLICATE | TRANSFER |
WRITE` (no input path, so no `READ`) and derives `WRITE`-only copies for chosen children; a program not
handed one cannot print, but for §7's interim. **Rejected:** seL4's `DebugPutChar`, any thread's in a
`CONFIG_PRINTING` build; a console at a well-known handle. **Adopted in spirit:** Zircon's write-only
debuglog. Once a userspace UART driver exists the root hands out an endpoint speaking `WRITE`, no client
changes, and the Console object is **deleted** (increment 13).

## 12. `user/abi` — the kernel's own binding

**Which ledger row.** The libc/runtime row ("port code — relibc pieces") gives a program an environment
and issues no syscalls: relibc sits on `redox_syscall`, versioned with the Redox kernel, and seL4 keeps
libsel4 in its kernel's repository. The binding is the user half of §3's table, with nothing to port.
**Verdict sought: the microkernel-core row, write ourselves**; anything that would read the same on
another kernel moves out. Appending "syscall ABI binding (`user/abi`)" to that row is constitution text,
the maintainer's (Constitution §4, logged in `docs/CHANGELOG.md`); increments 2 and 7 wait on it. `abi`
holds the numbers, `Handle`, `Tag`, `Status`, labels, `BootCap`, the layouts with `const` assertions and
safe wrappers — no allocator, no formatting. The kernel and `xtask` build it without its `syscalls`
feature, the one that compiles `src/arch/**`, so all three share one data half.

**The third designated `unsafe` tree**, approved in principle on 2026-09-27, is `user/abi/src/arch/**`,
holding only the `svc`/`syscall` wrappers, the discovery-register read and the entry stubs. `root_start`
copies the BootInfo page by volatile reads onto its stack and hands `main` a reference to the copy — sound
because the kernel mapped the page read-only before the process's first instruction and nothing else has
run; `child_start` passes `a0` and `a1`. User programs contain no `unsafe`, and no Rust reference is formed
into memory another thread may write. **Sought separately, not for the demo:** volatile IPC-buffer word
accessors in `user/abi/src/buffer/**`, whose invariant — the buffer stays mapped at its bound address while
the thread runs — the process itself upholds; broken, it faults that thread and touches no other process.

**The edits, all in increment 7:** `CLAUDE.md` § `unsafe` policy gains "`user/abi/src/arch/**` — trap
instructions and process entry stubs" and § Layout gains `user/`; threat-model O-6 names three trees; the
`Cargo.toml` comment ("the two trees") and `kernel/src/main.rs`'s grep widen to the repository; and
`xtask/Cargo.toml`'s "No dependencies" becomes "no external dependencies".

## 13. Lineage

Named inline. Mostly **seL4** (root task, invocation as IPC, `extraCaps`, reply objects, transfer at
rendezvous; not CNodes, `DebugPutChar` or 120-word messages) and **Zircon** (the process object, debuglog;
not a syscall per operation), with **Linux**, **Hubris**, **Genode** and **Redox** for smaller pieces.

## 14. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-1 unforgeability | handles reach the kernel only as words resolved through the caller's table; word 0 and out-of-range indices fail closed (§6) | **binds at B1** — RFC-0003's mechanism made live |
| O-2 non-widenability | `cap_derive` is the only duplication and is subset-only; presented handles move nothing; moves never widen (§4, §8) | **binds at B1** |
| O-4 no ambient authority | twelve syscalls, each handle-indexed or authority-free; methods take handles; console by capability (§4, §11) | **discharged at the ABI, except the fault-report interim** (§7) until increment 11; each new method checked at review |
| O-5 argument validation | no pointer arguments; reserved tag bits zero; single fetch at rendezvous; transfers all-or-nothing; kernel-built `SPSR_EL1`; `sysretq` guards (§3, §5, §7) | **discharged for the syscall surface**, with RFC-0005 §11 for user memory |
| O-6 kernel memory safety | a third designated tree, trap instructions and entry stubs only; threat-model O-6 names it (§12) | **stays Built**, with a repository-wide grep |
| O-7 no unprivileged exhaustion | 512-byte copy bound; creation paid by a named Pool; reusable reply objects; fixed tables fail closed; a console that never waits (§4, §5, §11) | **contributes** — RFC-0005 owns the accounting |
| O-9 no ambient side channel | result registers carry results or zero; no grant after start; no ambient console (§3, §8, §11) | **contributes**, except the fault-report interim (§7) |
| O-13 W^X | the build refuses writable-and-executable segments; loaders map per descriptor (§10) | **contributes** — RFC-0005 owns enforcement |
| O-17 contained drivers | fault messages to `fault_ep`; `KILL`-only authority over a started Process; dead processes inspectable (§7, §8) | **designed, deferred** — the interim stops the thread |
| O-27 explicit authority at spawn | empty table at birth; grants only by explicit move before start; root's grant enumerated; a host test and a boot-test count the child's table (§8, §9, §18) | **discharged** in its letter; the spawner's retained control is named (§8), not claimed away |

## 15. Graves checked (§3)

- **Policy in the kernel.** The kernel runs one image and lists what it granted; the rest is the root's.
- **The catch-all right.** No root identity survives `eret`; no right is added. A Process capability reaches
  its table only through `GRANT` before start, then means `KILL`; control of a running child lives in
  separate AddressSpace and Thread capabilities, each droppable.
- **Baroque hierarchies; multi-copy IPC.** Flat handles, no receive window, a flat BootInfo; one copy.
- **Bolted-on multicore.** Entry is per-core from the first stub (`SP_EL1`, `TPIDR_EL1`; `swapgs`).
  RFC-0006 (proposed) §12 provides the big kernel lock taken on entry, answering RFC-0003 §14.2; single
  fetch stays necessary under it, because other threads write the buffer from EL0 without the lock.
- **Drivers pulled in; unused device paths.** The Console alone is reachable from EL0, bounded, its
  deletion scheduled.

## 16. Costs — what this makes harder

- **`call` is overloaded** — per-type method decoding and two error layers, the price of interposition —
  and **interposition is partial:** a forwarder for a method with handle-word arguments must first be handed
  those capabilities, costing the client a `TRANSFER` it may not hold (input to RFC-0003a).
- **Four physical registers and 64 words:** past 32 bytes a message touches memory, a 65–120-word message
  seL4 would copy needs a Region here, and raising either later breaks binaries.
- **Handles are 20/44 for good,** the accepted crate changes by amendment, and `TPIDRRO_EL0` and the user
  GS base belong to the ABI rather than a runtime.
- **A spawner keeps control if it wants to:** the window stops injection, not a loader that kept derived
  copies of the child's AddressSpace or Threads, and the child cannot yet check.
- **The root is the most sensitive process,** as in seL4, and not revocable until RFC-0003a; it must stay
  small. **No ambient printing; late grants need a willing child; every console writer loops** on short
  24-byte writes. **No PAN on the demo CPU;** `user/abi` is a third `unsafe` tree to audit.

## 17. Open questions

1. **Fault messages and death** — their layout (this RFC's), how a supervisor learns a process died, and
   how it supervises threads a child creates itself (RFC-0006 §4); likely a notification bound at creation.
2. **Badges** — `cap_derive`'s badge word is reserved zero until RFC-0003a; notification signals then carry it.
3. **A sovereignty check** — should RFC-0005's AddressSpace query report its live-capability count, as
   RFC-0006's `ctl_info` does for `SchedControl`, so a child can confirm it holds the only one?
4. **The Language clause** — do single-instruction `svc`/`syscall` wrappers and entry stubs in userspace
   fall within "minimal assembly … for boot and context switching", as RFC-0005 §17.7 asks for `TLBI`?
5. **The 64-word budget** — a verdict now, to be revisited with the first measurements.

## 18. Implementation increments

Each is one reviewable PR; **[demo]** marks what the Phase-1 demo needs. Userspace boot-tests expect
prefixed strings, since bare `Kaya!` matches the kernel's greeting; every demo line fits one `WRITE`.

1. **[demo] `capability`: the handle word**, `free_slots`, with RFC-0003's amendment. Host tests: round
   trips; word 0 and index ≥ 2^20 refused; retirement at the bound; `free_slots` exact beside retired slots.
2. **[demo] `user/abi`, data half**, after the ledger verdict, no `unsafe`. Host tests refuse every
   reserved-bit (tag bits 9–15 included) and over-length encoding; CI builds both targets.
3. **[demo] AArch64 trap path, MMU off** — one kernel-held frame (RFC-0006's increment 4 moves frames into
   TCBs), dispatch, `eret`; `yield`, `thread_exit`, `InvalidSyscall`; an EL0 fault stops the self-test, not
   the kernel: `--features syscall-selftest --expect "syscall round trip"`. **Deleted** by RFC-0005's 9.
4. **[demo] `xtask`: repeated `--expect`, matched in order**, host-tested.
5. **[demo] The bundle** — the ELF reader, host-tested on W+X, overlapping, truncated, `PT_TLS`, `PT_DYNAMIC`
   and executable-stack fixtures; `build.rs`, `.incbin`, `.payload`: `--expect "boot modules: 0"`.
6. **[demo] Objects, `cap_*`, the dispatcher, the Console and the Process's kernel half** (create, `INFO`)
   on RFC-0005's store and root Pool, with its 14: an EL0 self-test in a kernel-built process prints
   through a Console handle, `--features syscall-selftest --expect "[el0] Kaya!"`.
7. **[demo] The root reaches EL0** — `user/abi/src/arch/**` and every §12 edit in one PR, once the tree is
   approved; module 0 becomes the root: `--expect "boot modules: 2" --expect "[root] up"`.
8. **[demo] `GRANT`, the window, `INFO`, `KILL`** in a host-tested `process/` crate, then wired: born empty;
   `GRANT` all-or-nothing; `WindowClosed` after the first resume; an AddressSpace bound once.
9. **[demo] The root spawns the client,** in three PRs:
    - **a.** A host-tested `no_std` `user/loader` crate: descriptor in, method calls out, W^X and overlaps
      refused; a host test that the spawner ends holding only the Process capability.
    - **b.** The root runs the plan, granting a `WRITE`-only Console and a send-only endpoint:
      `--expect "[root] up" --expect "[client] up"`.
    - **c.** O-27 evidence: the client probes indices 0–63 at generation 1 with `cap_rights` (exhaustive
      while no slot has been vacated), printing `[client] 2 handles`, and the root prints `INFO`'s count,
      `[root] client: 2 caps`. Evidence, not proof: the proof is increment 8's host test.
10. **[demo] IPC syscalls** over RFC-0004 endpoints, reusable Reply objects and RFC-0006's blocking states:
    `[client] -> Kaya!`, `[server] <- Kaya!`, `[client] <- Kaya!` in order. Host test: clients that exit
    mid-call leave the server's Pool balance unchanged.
11. **Fault messages and death notification** (§17.1): the root prints `[root] child faulted`, and
    `report-user-faults` leaves the boot-tests.
12. **x86_64 trap path and `abi` stubs** (`EFER.SCE`, `LSTAR`/`STAR`/`FMASK`, `swapgs`, the `sysretq`
    guards), after the x86_64 boot RFC's UEFI stub: the demo boot-tests on `q35`.
13. **Delete the Console object** when the Phase-2 UART driver lands.
14. **`Revoked`**: the crate's `ObjectDestroyed` split, with RFC-0005's destruction (its 17).
15. **The root spawns the server** and stops serving, removing that interim.

**Demo order across the three RFCs** — the one merged order, to which RFC-0005 §18 and RFC-0006 §19 defer,
for the maintainer to fix:

1. **Before any EL0 code, MMU off,** as dependencies allow: RFC-0005 1–8, 11 and 12; RFC-0006 1–3, 6a and
   7a; this RFC's 1, 2 and 4, with 5 after 2.
2. **EL0 with the MMU off:** this RFC's 3, then RFC-0006 4.
3. **MMU on:** RFC-0005 9–10, retiring the MMU-off EL0 self-tests and repointing the UART and the GIC.
4. **Address spaces and the first userspace:** RFC-0005 13 → RFC-0006 5 → 6 with RFC-0005 14 → 7 → 8 →
   RFC-0006 6b → 9 → RFC-0006 7b → 10 with RFC-0006 7c.

**The demo's interims, with their costs:**

- **Faults print and stop** (§7): an ambient output channel and ~100 bytes of kernel UART time per fault.
- **The in-kernel Console** (§11); **modules in the AArch64 image** (§10), relinking the kernel per change.
- **One root Pool, no destruction** (RFC-0005): charges unattributed, `KILL` releases nothing, no `Revoked`.
- **Register-only messages** (RFC-0005's IPC-buffer module, its 16, is not demo): no thread has a buffer
  and no capability crosses an endpoint; the demo's grants use `GRANT`, whose words are registers by design.
- **An active server** (RFC-0006's A1), so RFC-0004's donation is not exercised; **the root doubles as
  server**, running client-facing code in the process holding every boot capability, until increment 15.
- **The MMU-off self-test** (increment 3); **RFC-0006's provisional constants** for the root's scheduling.

## 19. What this unblocks

The first userspace, and every Phase-2 paper: the **scheme registry** and **broker** (processes, grants,
`call`), the **driver framework** (faults, interrupt notifications, the console moving out), a **relibc
port** over `user/abi` with both TLS registers its own, and the **x86_64 bring-up**. RFC-0003a gains an ABI
and two inputs: partial interposition (§16) and the sovereignty check (§17.3).
