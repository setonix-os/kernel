<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0007 — System calls and process bootstrap

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 (capabilities are the only authority), pillar 2; `CLAUDE.md` § `unsafe` policy and § Layout; the workspace (`user/`); `xtask` and `kernel/build.rs`; the kernel's trap path; the accepted `capability` crate (the handle word, a slot-generation bound, one error variant split) |
| Depends on | RFC-0003 (accepted; §9 binds every syscall); RFC-0004 (accepted; §5, §6, §8, §9.4, §10 and the 2026-08-01 amendment); RFC-0005 and RFC-0006 (proposed, drafted alongside) |
| Discharges | O-5 (argument validation) for the syscall surface; O-27 (explicit authority at spawn); binds O-1, O-2 and O-4 at B1; contributes to O-7, O-9 and O-13 |

> **Proposed verdicts.**
>
> 1. **Uniform invocation:** every kernel-object operation is a `call` on its capability; twelve syscalls in all (§5).
> 2. **Registers:** AArch64 `svc #0`, number in `x8`; x86_64 `syscall`, number in `rax`; every thread starts with zeroed registers but four (§4).
> 3. **Messages:** four physical and sixty-four virtual message registers, three capabilities; a 1 KiB IPC buffer, every word fetched once (§6).
> 4. **IPC buffer discovery:** a kernel-set, user-read-only register — `TPIDRRO_EL0`; the user GS base with a self-pointer at `gs:[0]` (§6).
> 5. **Handle word:** 20-bit index, 44-bit *slot* generation, `0` is null; the crate retires slots at 2^44 − 1, objects keep 64 bits (§7).
> 6. **Errors in two layers:** transport status in a register, method result in the reply label; `BudgetRefused` beside `BudgetExpired`; faults are not errors (§8).
> 7. **Process object:** table + address space + threads, born empty, grant window shut by the first `resume` of any of its threads (§9).
> 8. **One root task,** built once from a load descriptor, every boot capability listed in BootInfo, no "is root" branch anywhere (§10).
> 9. **Boot images:** `xtask` parses ELF on the host and embeds a bundle of fixed-shape descriptors in the kernel ELF; the kernel never parses ELF (§11).
> 10. **Console as a capability:** a kernel object with one `WRITE` method, scheduled for deletion when the Phase-2 UART driver lands (§12).
> 11. **`user/abi`** is the microkernel-core row, not libc/runtime; its `src/arch/**` is the third designated `unsafe` tree (§13).

## 1. The question

**How does userspace ask the kernel to do anything, and how does the first userspace come to exist —
such that every request names a capability, no argument is trusted, and no process is born holding
authority it was not explicitly handed?**

RFC-0003 §9 fixed the rule and RFC-0004 §10 the IPC semantics; both left the encoding here. This RFC
designs the encoding, the trap, the process as a kernel object, and the bootstrap seam RFC-0003 §14.4
named: minting the first capabilities without an ambient grantor. It does not design address spaces
(RFC-0005), threads or scheduling (RFC-0006), or any policy about which programs run holding what —
that is the root task's, in userspace, and later the broker's.

## 2. Which pillar

**Pillar 2 and the kernel doctrine that capabilities are the only authority.** The syscall surface is the
whole of B1; one entry that names a resource without a handle makes RFC-0003's table decorative. Spawn
is where research/0002 found the freshest scar (Part 4: runc CVE-2024-21626, one inherited descriptor
became host traversal), and O-27 exists because of it. Pillar 1 is served in passing: boot images reach
userspace as read-only regions, immutable from their first instruction.

## 3. What already binds this RFC

- **RFC-0003 §9:** resources only by handle; authority-free are yielding, halting oneself, self-queries.
- **RFC-0004 §5–§10 and its 2026-08-01 amendment:** virtual message registers; one bounded copy;
  cap-carrying messages off the fast path; all-or-nothing transfer; the IPC semantics; a minimum budget
  refused up front at `call`, and an error completion when a budget expires inside a server.
- **`CLAUDE.md`:** `unsafe` only in designated trees; no kernel heap; both targets soft-float.

## 4. Entering the kernel

**AArch64 — `svc #0`.** From EL0, `svc` takes a synchronous exception to `VBAR_EL1 + 0x400` (lower EL,
AArch64 — entry 8 of the table `vectors.s` already installs) with `ESR_EL1.EC = 0x15`, the immediate in
`ESR_EL1.ISS[15:0]`, and `ELR_EL1` already past the `svc`; entry masks D, A, I and F and selects `SP_EL1`
*(Arm ARM DDI 0487)*. The stub saves `x0`–`x30`, `SP_EL0`, `ELR_EL1` and `SPSR_EL1` into the thread's
frame (RFC-0006 (proposed) provides its layout, in the TCB) and dispatches on EC: `0x15` to the syscall
dispatcher, anything else is a *fault* (§8). A non-zero immediate answers `InvalidArgument`, so a later
ABI revision is detectable rather than silently accepted. Return is `eret` with an `SPSR_EL1` the kernel
builds: `M` = EL0t, DAIF clear, NZCV from the frame; where implemented, `DIT` (Armv8.4) and `SSBS`
(Armv8.5) — which EL0 can already set with `msr` — are carried too; every other bit (`SS`, `IL`, `PAN`,
`UAO`, `TCO`, `BTYPE`) is zero. The Cortex-A72 implements neither, so on the demo CPU only NZCV crosses.
*(Linux arm64 and seL4 both enter through `svc #0`.)*

**x86_64 — `syscall` / `sysretq`.** `syscall` requires `IA32_EFER.SCE`; it loads `RIP` from `IA32_LSTAR`,
leaves the return `RIP` in `rcx` and `RFLAGS` in `r11`, clears the `RFLAGS` bits named in `IA32_FMASK`
and takes `CS`/`SS` from `IA32_STAR` — but does **not** switch stacks *(Intel SDM Vol. 3A, "Fast System
Calls in 64-Bit Mode")*. The stub begins with `swapgs`, parks the user `rsp` in per-CPU scratch and loads
the kernel stack; the untrusted `rsp` is never dereferenced. `FMASK` clears IF, DF, TF and AC: interrupts
off, string direction sane, and — where SMAP exists — user access closed. Two hazards are designed in:

- **Non-canonical return.** On Intel parts `sysretq` with a non-canonical `rcx` raises #GP *in ring 0*
  on the user's stack — CVE-2012-0217, which took Xen, FreeBSD, NetBSD and Windows. RFC-0005 (proposed)
  leaves the top user page unmapped and RFC-0006 (proposed)'s `write_regs` refuses such a `RIP`; the exit
  path checks `rcx` anyway and falls back to `iretq`, because the check is one compare.
- **The window before the stack switch.** An NMI or machine check there would run on a user-chosen
  `rsp`; RFC-0006 (proposed) provides IST stacks for the exceptions that can land in it.

`sysretq` also fixes the GDT order — user data at `STAR[63:48] + 8`, user code at `+ 16`. `sysenter` is
rejected: AMD does not implement it in long mode; so is an `int` gate, which costs a full IDT delivery.

| Role | AArch64 | x86_64 |
|------|---------|--------|
| syscall number (in) | `x8` | `rax` |
| invoked handle (in) / status (out) | `x0` | `rdi` in, `rax` out |
| message tag (in/out) | `x1` | `rsi` |
| MR0–MR3 (in/out) | `x2`–`x5` | `rdx`, `r10`, `r8`, `r9` |
| second handle: reply object (in) / badge (out) | `x6` | `r12` |
| error detail (out) | `x7` | `r13` |
| clobbered by hardware | — | `rcx`, `r11` |

Every register outside the result set is restored from the frame; the eight result registers (`x0`–`x7`;
`rax`, `rsi`, `rdx`, `r10`, `r8`, `r9`, `r12`, `r13`) carry a result or zero, so no kernel value ever
leaves in a register (O-9 hygiene). `x8`/`rax` follow Linux, leaving `x0`–`x7`, the AAPCS64 argument
registers, to the message. The x86_64 column avoids `rbx` and `rbp`, which Rust inline assembly cannot
name (LLVM's base and frame pointers), and `rcx`/`r11`, which `syscall` destroys. seL4 has the same
shape in different seats — `x7`; `rdx` *(libsel4 `sel4_arch/syscalls.h`)*.

**Entry state of every thread,** root task included, on both architectures: PC = entry; SP as given;
`a0` and `a1` — `x0`/`x1`, `rdi`/`rsi`, so `_start` is an ordinary two-argument `extern "C"` function —
the two start values; every other general register zero; EL0t with DAIF clear (`SPSR_EL1` = 0), or ring 3
with `RFLAGS` = `0x202` (IF and the always-one bit 1); FP/SIMD trapped (`CPACR_EL1.FPEN` = 0, `CR0.TS`
set) until RFC-0006 (proposed) enables it for the thread; `TPIDR_EL0` and `FS_BASE` zero; the IPC-buffer
register set (§6). A Thread is created in exactly this state; the spawner writes PC, SP, `a0` and `a1`
through RFC-0006 (proposed)'s `write_regs`, and the kernel writes the root's (§10).

**The kernel never touches user memory through an address userspace supplies.** No syscall argument is
a pointer; the only user memory the syscall path touches is the IPC buffer, reached by frame (§6). On the
demo CPU this discipline is the whole defence: the Cortex-A72 is Armv8.0 and lacks PAN (Armv8.1).

## 5. The syscall set

- **A — one syscall per operation** *(Zircon, Linux)*. Legible per call, but the table grows with every
  object type, each entry is a new path to audit, and no server can ever stand in for a kernel object.
- **B — uniform invocation** *(seL4; KeyKOS before it)*. A handful of IPC syscalls; an operation on a
  kernel object is a `call` on its capability with the method in the tag's label, answered by the kernel
  as a server would answer. Kernel objects and servers are invoked identically.
- **C — B plus a separate `invoke` number.** Keeps the small table and loses the point: the caller can
  again tell a kernel object from a server.

**Verdict sought: B.** The payoff is interposition. A Console capability and an endpoint to a logging
server are invoked alike, so Phase 2's userspace UART driver takes over the console protocol without a
client changing, and the broker can hand out a forwarder in place of any kernel object — RFC-0003a option
(a)'s precondition. A `call` on a kernel object runs on the caller's own time: nothing is donated,
nothing blocks, no reply object is involved.

| # | Syscall | Handles | Beyond the tag and message registers |
|---|---------|---------|--------------------------------------|
| 0 | `yield` | — | authority-free (RFC-0003 §9) |
| 1 | `send` | object | endpoint: blocking send; notification: signal (RFC-0004's `notify`) |
| 2 | `nb_send` | object | as `send`, failing `WouldBlock` rather than blocking |
| 3 | `call` | object | endpoint: send, await reply, donating (RFC-0004 §8); kernel object: invoke a method |
| 4 | `recv` | endpoint or notification; reply object | blocking receive; a bound notification may wake it |
| 5 | `nb_recv` | as `recv` | poll |
| 6 | `reply` | reply object | reply once to a caller |
| 7 | `reply_recv` | endpoint; reply object | `reply` then `recv` in one entry — the server loop *(seL4 `ReplyRecv`)* |
| 8 | `cap_derive` | object, with `DUPLICATE` | in: MR0 the rights wanted, MR1 a badge (reserved, zero); out: MR0 the new handle word |
| 9 | `cap_close` | object | nothing; the handle is dead on return |
| 10 | `cap_identify` | object | out: MR0 the object kind (an `abi` enumeration), MR1 the rights bits |
| 11 | `thread_exit` | — | halt the calling thread; authority-free |

`reply_recv` fuses two RFC-0004 operations so a server loop is one kernel entry, which the direct-switch
fast path needs; it adds no semantics. `notify` is `send` on a notification, since RFC-0004 left "one
syscall with a mode, or several" here; a signal never blocks, so `nb_send` signals too. The `cap_*` calls
take a zero tag, return a zero tag, and act only on the caller's own table, gated by the handle they name,
so §9 needs no table capability. A rights word with an undefined bit answers `InvalidArgument`; the
dispatcher assembles `Rights` from the crate's named constants, so the crate still never accepts raw
bits. Transfer in a message is always a move; a sender that keeps a copy runs `cap_derive` first
(RFC-0003 §6). **Creation** is a method on a memory capability — RFC-0005 (proposed) provides the Pool
and its creation methods — and the per-type parameters are encoded here: an endpoint's creation carries
**`min_budget`** in MR1, in RFC-0006 (proposed)'s time unit, 0 for none, fixed for the endpoint's life.
**Rejected:** a `debug_putc` (§12); anything taking a PID, path or address; seL4's depth-addressed
capability pointers, which exist for CNode trees this kernel does not have.

## 6. Messages, the register budget and the IPC buffer (RFC-0004 §9.4)

The **tag** is one word: bits 0–6 length in words (0–64); bits 7–8 capabilities attached (0–3); bit 9
set by the kernel when a `recv` was woken by its bound notification (MR0 then holds the pending bits),
zero on input; bits 10–15 reserved, zero; bits 16–63 a 48-bit label — user-defined for endpoints, the
method number for kernel objects, the method result on a kernel reply. *(seL4's `seL4_MessageInfo`:
label, extra capabilities, length.)*

**Four physical message registers on both architectures.** seL4 uses four on both 64-bit targets;
research/0001 found the in-register payoff small and shrinking (10% on ARM11, negative on modern x86);
and a count that differed by architecture would make one message fast on one Tier-1 target and slow on
the other. Messages up to **64 words** (512 bytes) spill MR4–MR63 to the IPC buffer — below seL4's 120 on
purpose: 512 bytes bounds the non-preemptible copy, and RFC-0004 §5 sends bulk data through shared
regions. **Three capabilities per message**, seL4's 2-bit field, so the field has no invalid encodings.

```rust
/// 1 KiB and 1 KiB-aligned, so it never straddles a page.
#[repr(C, align(1024))]
pub struct IpcBuffer {
    pub self_ptr: u64,      // this buffer's user address, written by the kernel at binding
    _reserved: [u64; 3],
    pub msg: [u64; 64],     // MR0..MR63; msg[0..4] never read, those travel in registers
    pub caps: [u64; 3],     // handle words sent or received, `tag.caps` of them
    _tail: [u64; 57],       // neither read nor written by the kernel
}
```

**The buffer is kernel-owned memory, reached by frame.** A thread's buffer is a 1 KiB-aligned slice of
one page of a Region, bound when the thread is created by the Region capability (`READ | WRITE`), the
offset, and the user address where the spawner mapped it; RFC-0006 (proposed) records the binding and
RFC-0005 (proposed) provides the Region and the kernel's physmap. The kernel reads and writes the buffer
only through its own mapping of that frame. The user address is used for two things — word 0 and the
discovery register — and is never dereferenced. Received capabilities land in fresh slots of the
receiver's table (RFC-0004 §6), so there is no seL4-style receive window.

**Discovery: a kernel-set, user-read-only register.** The kernel loads the buffer's user address into
`TPIDRRO_EL0` (readable at EL0, writable only at EL1) and into the user GS base on x86_64, where
`CR4.FSGSBASE` stays off (RFC-0006 (proposed)) and `abi` reads the self-pointer at `gs:[0]` — the x86_64
ELF TLS convention of a thread block whose first word holds its own address *(Drepper, "ELF Handling For
Thread-Local Storage")*. RFC-0006 (proposed) already restores both registers on every switch and leaves
their use here, so `_start` has its buffer at its first instruction, in any thread, with no TLS runtime.
The kernel trusts neither: a thread that reloads its own GS selector misleads only itself. **Rejected:**
`TPIDR_EL0` and `FS_BASE` — the thread pointers of the AArch64 and x86_64 ELF TLS ABIs, which a relibc
port needs untouched; and seL4's current answer, a thread-local (`__sel4_ipc_buffer`, libsel4
`functions.h`) — sound, but it needs TLS before the first syscall.

**Single fetch (O-5).** Once SMP exists, another thread of the same process can rewrite the buffer
mid-syscall. So every buffer word the kernel acts on is read **exactly once** into kernel memory before
any check depends on it; lengths and capability counts come from the tag register, already in the saved
frame, bounded by 64 and 3; the `caps[]` words are copied into a kernel array and every one is resolved
and checked for `TRANSFER` before any moves — all or nothing (RFC-0004 §6); payload words are copied
frame to frame by RFC-0005 (proposed)'s volatile-copy module and never interpreted, and no Rust reference
is ever formed into the buffer. *(research/0002 Part 5: virtio's double-fetch lesson — "validate a
copied-out snapshot; never re-read after validation".)* **Rejected:** six physical registers on AArch64
(portability); seL4's 120-word maximum (a longer non-preemptible copy for payloads RFC-0004 routes
elsewhere).

## 7. Handles in one register

RFC-0003's `Handle` is a `u32` index and a 64-bit generation — ninety-six bits, which fit no register.
The ABI word is **`slot_generation << 20 | index`**: 2^20 slots per process, Linux's default
per-process descriptor ceiling (`fs.nr_open` = 1,048,576); and 2^44 slot generations, about two hundred
days of one close-and-reopen per microsecond on a single slot before it retires. `Generation::FIRST` is
1, so word `0` is never live and serves as null. This is the width RFC-0003's 2026-07-30 amendment asked
to be stated. **Only the slot generation crosses the ABI;** a capability's minted *object* generation
stays in kernel memory. So the accepted crate changes narrowly: `Handle::to_word`/`from_word` and a
constant `Handle::MAX_SLOT_GENERATION` (2^44 − 1); `CapabilityTable::remove` retires a slot whose next
generation would exceed it, through the `Retired` state the table already has, so a generation the ABI
cannot carry retires its slot rather than aliasing; a const assertion keeps every kernel table's `N`
≤ 2^20. `Generation` itself is untouched — objects keep all 64 bits — and its doc comment, which
promises 64 bits, gains the slot bound. Phase 1 tables have `N` = 64, fixed, failing closed with
`TableFull`. **Rejected:** two registers per handle (halves the message budget); 32/32 (2^32 reuses is
seventy minutes at one per microsecond, and the LIFO free list concentrates reuse on one slot); a
16-bit index (65,536 capabilities is too few for the server workloads the constitution targets).

## 8. The error model

**Transport status** (`x0`/`rax`) is what the syscall layer found, identical whether the target is a
kernel object or a server. Every failure is atomic — nothing moved, nothing half-sent — and the detail
register names the culprit: 0 the invoked handle, 1–3 an attached capability, 4 the tag, 5 the reply
handle.

| Status | Meaning |
|--------|---------|
| `Ok` | done |
| `InvalidSyscall` | no such number — a closed error, not a fault: the surface stays total and needs no fault-handler policy |
| `InvalidArgument` | a reserved bit, a length over 64, a non-zero `svc` immediate, an undefined rights bit |
| `InvalidHandle` | word 0, index out of range, empty slot, or stale slot generation — the handle names nothing *of yours*: the crate's `OutOfBounds`, `Empty`, `StaleGeneration` |
| `Revoked` | the slot is live but its object was destroyed — the crate's new `ObjectDestroyed`; close the handle |
| `WrongType` | the handle is live but names the wrong kind of object for this syscall |
| `InsufficientRights` | the capability lacks the right the operation needs |
| `TableFull` | a moved capability found no slot in the receiving table; nothing moved |
| `WouldBlock` | `nb_send` or `nb_recv` found no partner |
| `PeerGone` | the object or party waited on was destroyed mid-wait |
| `BudgetRefused` | `call` whose donated context can never meet the endpoint's `min_budget`, refused up front (RFC-0004's amendment; seL4 RFC-14) |
| `BudgetExpired` | the donated budget expired inside the server; the kernel completed the wait on the reply object (RFC-0004's amendment) |

`Revoked` needs the second crate change. Today `StaleGeneration` covers both a reused slot — the
holder's own use-after-close — and a destroyed object; `resolve` already checks them in that order, so
the split is one new variant and one changed return. It lets a holder observe and survive destruction,
as research/0002 asks of revocation, without reporting its own use-after-close as something done to it.
When `BudgetRefused` and `BudgetExpired` arise is RFC-0006 (proposed)'s to define; its draft calls the
first `BudgetBelowThreshold`, and the name here is proposed because the encoding is this RFC's.

**Method result** (the reply tag's label) is what the object said, written by the kernel for kernel
objects and by the server for endpoints, in one vocabulary — so a server standing in for a kernel object
reports errors the same way *(seL4 returns invocation errors in the reply's label)*. It reuses the status
numbering where meanings coincide and adds per-object results: `InvalidMethod`, RFC-0005 (proposed)'s
`PoolExhausted` and `Fragmented`, and §9's `WindowClosed`.

**Faults** are EL0 exceptions that are not syscalls. They are not return values — the thread cannot
continue. The design suspends it and sends a fault message to a handler RFC-0006 (proposed) binds to the
thread, so a supervisor can restart a driver (O-17). **Interim:** until then the existing reporter prints
the fault, prefixed by the thread, and the thread stops; the core continues, and nothing restarts it.

## 9. The Process object and spawning (O-27)

A **Process** holds one RFC-0003 capability table, a reference to one AddressSpace (RFC-0005 (proposed))
and the threads bound to it (RFC-0006 (proposed)). The per-process table makes the process a real object
*(Zircon)* rather than seL4's convention of a thread pointing at a CSpace and a VSpace. Three ways to give
it its first authority:

- **A — inherit** *(Unix `fork`/`exec`)*. **Rejected** by O-27 by name: an inherited-by-default table is
  ambient authority smuggled across creation.
- **B — one bootstrap handle** *(Zircon: `zx_process_start` moves exactly one handle; a loader sends the
  rest as a processargs message)*. Minimal, but it leans on a buffered channel; over Setonix's unbuffered
  rendezvous the parent must serve its own child a protocol before the child can do anything, and every
  spawner holds a bootstrap endpoint per child.
- **C — explicit grants before start** *(seL4's root task writing a child's CSpace, on a flat table)*.
  **Verdict sought.**

A Process is created through RFC-0005 (proposed)'s Pool, naming an AddressSpace, and is born with an
**empty table**. Its methods:

| Method | Right | Effect |
|--------|-------|--------|
| `GRANT` | `WRITE` on the process; `TRANSFER` on each attached capability | move up to three attached capabilities into the child's table; the reply's MR0–MR2 carry their handle words *in the child* |
| `KILL` | `WRITE` | stop every thread, drop the table, release the address space; the generation bump makes every capability to the process inert |

`GRANT` is a `call` on the Process with capabilities attached: the kernel takes them from `caps[]`
exactly as an endpoint send does (§6) — fetched once, all resolved before any moves.

**The grant window.** `GRANT` succeeds only until the first `resume` of any thread bound to the process —
a one-way event RFC-0006 (proposed) reports by setting a flag on the Process; afterwards it answers
`WindowClosed`. From then on authority arrives only by IPC, which the process consents to by receiving. A
parent cannot reach into a running child's table, and a stolen Process capability cannot inject authority
into a live process. The window is **single-use** in research/0002's sense (Part 3, Vault's
response-wrapping) but gives none of its **prior-use detection**: the kernel does not tell the child who
granted what. Within the kernel the channel is the Process capability, minted to its creator alone;
detection across imperfectly trusted spawn channels belongs to the broker RFC, which owns those channels.

The child learns its handles as any program learns its arguments: the spawner writes them into `a0` and
`a1` (for the demo, the console and the endpoint), or points `a0` at a start block it wrote — a
runtime-row convention, not kernel ABI. No right joins RFC-0003's set: `GRANT`, `KILL` and thread
creation all use `WRITE` on the process — authority over *that one object*, held by its creator, not a
system-wide catch-all. A process holds no capability to itself unless handed one, and is destroyed when
its last thread exits.

## 10. The root task and boot info

The one act the kernel performs unasked: at boot it builds **exactly one** process from **exactly one**
image, the first boot module, and gives it every boot capability. *(seL4's root task; research/0002 Part 3:
"the bootstrap seam is a one-time kernel act".)* In order: an AddressSpace; the descriptor's segments
copied into fresh frames and mapped with their stated permissions (§11); a stack; an IPC buffer; a
read-only **BootInfo** page; a Process and a Thread with the entry state of §4, `a0` = BootInfo's address
and `a1` = 0; `eret`. RFC-0006 (proposed) provides the root's first scheduling context, whose parameters
are a named boot constant — as seL4's root task starts at the maximum priority with one configured
timeslice.

**BootInfo** is kernel ABI, defined in `user/abi`: a header (magic, version, length) and one entry per
initial capability — `{ kind, flags, handle, base, size }` — with a string table for module names. Every
initial capability is *listed*; none sits at a well-known slot, so the order can change without an ABI
break and a reviewer can enumerate the whole bootstrap grant. A host test pins its size within one page.
This RFC defines three kinds: the root's own Process, the Console (§12), and one read-only Region per
further boot module, with that module's load descriptor. RFC-0005 (proposed) provides the root's
AddressSpace, the root Pool covering all free RAM and the DTB Region; RFC-0006 (proposed) its Thread,
`SchedControl` and interrupt capabilities.

**The root is not a superuser.** It holds everything at first, but as ordinary, droppable,
generation-checked capabilities, and the rule that makes that true is checkable: **no kernel code
branches on "is this the root task"** — after `eret` the kernel does not know which process came first.
The bootstrap graph is acyclic by construction: kernel to root, root to children, and nothing the root
needs waits on a userspace service. **Rejected:** a kernel boot manifest of programs and their grants —
policy in the kernel; a `spawn_from_module` syscall — a loader offered to userspace as a kernel service;
and a bootstrap superuser, research/0002's `system:masters`.

## 11. Getting the first images into memory

QEMU's `-kernel` loads one ELF. The candidates for the rest:

- **Embed them in the kernel ELF** *(verdict sought)*. One artefact boots, and its hash pins every
  program in it.
- **QEMU's `-initrd` or `-device loader`.** Two artefacts, QEMU-only — and neither reaches a bare ELF. In
  QEMU's `hw/arm/boot.c` (read at 8.2.2), `arm_setup_direct_kernel_boot` loads an `-initrd` only for
  images it treats as Linux, never an ELF; for a non-Linux image `do_cpu_reset` sets the PC and nothing
  else, so no register names the DTB either, which QEMU places at the base of RAM only if the image leaves
  room there. Today's image *is* the base of RAM, so it gets no DTB; RFC-0005 (proposed) relinks to
  `0x4020_0000` for that reason. A `-device loader` blob lands where only the command line knows.
- **Link the children into the root's ELF** *(common seL4 practice: a CPIO archive in the root image)*.
  The kernel knows one module, but the root becomes specific to each boot image and the children stop
  being separate immutable objects it can hand on.

**`xtask` parses ELF; the kernel does not.** `xtask` reads each program's headers with a dependency-free
ELF64 reader and emits a fixed-shape **load descriptor**: the entry point; a text segment (`R+X`), an
optional read-only segment (`R`) and a data segment (`R+W`, with a `bss` size), each page-aligned in the
user half; and the stack, IPC-buffer and BootInfo addresses. It **refuses** at build time any image with a
writable-and-executable segment, overlapping or unaligned segments, dynamic linking, TLS or any other
program header — W^X checked before the image exists (O-13). The kernel checks one fixed structure against
fixed bounds and never sees an ELF: Nexen's finding that kernel code validating structures built elsewhere
is the bug farm (research/0002 Part 7), applied to our own boot path. **Rejected:** an in-kernel `elf/`
crate — small and host-testable, but a parser in the TCB for no gain.

**The bundle.** `xtask` builds the user crates, writes `payload.bin` — a versioned header (an `abi` type)
and one entry per module: name, descriptor, segment bytes — and builds the kernel with `SETONIX_PAYLOAD`
naming it; `kernel/build.rs` places it page-aligned in a read-only `.payload` section (`KEEP`). **With the
variable unset it emits an empty bundle**, so a bare `cargo build` or `cargo clippy --package
setonix-kernel` still links, boots and greets, printing `boot modules: 0` before halting as today.
*(Hubris's `xtask dist`; seL4's elfloader, which parses before the kernel runs.)*

The kernel sees modules through one arch-neutral function, `arch::boot_modules()`. On AArch64 it returns
the embedded bundle; the x86_64 UEFI stub, when written, can return files it loaded — so embedding is
Phase 1's *source* of modules, not the design, and the kernel's contract is a range plus descriptors. The
kernel loads **entry 0 only**; every other entry reaches the root as a **read-only Region** over its pages
(RFC-0005 (proposed) provides those frames) with its descriptor in BootInfo, so the root copies segments
out without parsing anything and the image stays immutable (pillar 1). *(Genode's core serves boot modules
to init as read-only ROM modules.)* No "second process" logic exists in the kernel.

**How the programs are built.** Each is a `no_std`, `no_main` workspace crate under `user/` —
`user/demo/server` and `user/demo/client` — depending only on `abi`, with no `unsafe`, linked by
`user/link/<arch>.ld` at a fixed address in RFC-0005 (proposed)'s user half, static and non-PIE. The
AArch64 target already defaults to static relocation; `x86_64-unknown-none` defaults to position-independent
output and the *kernel* code model, so `xtask` passes `-C relocation-model=static -C code-model=small` for
user crates, in a separate target directory. The programs stay soft-float: per-thread FP/SIMD is
RFC-0006's, and the demo needs none.

## 12. The console as a capability

Userspace reaches the kernel's UART only through a **Console** kernel object with one method, `WRITE`,
invoked by `call`: the byte count in MR0 and the bytes from MR1 on — up to 504 per call, so a short line
never leaves the registers. It needs the `WRITE` right. The root receives the Console with
`DUPLICATE | TRANSFER | WRITE` — no `READ`: there is no input path — and derives `WRITE`-only copies for
the children it chooses. A program not handed one cannot print, which is the point.

**Rejected:** seL4's `DebugPutChar`, which any thread may issue in a `CONFIG_PRINTING` build — ambient
authority, however small; and a console at a well-known handle number. **Adopted in spirit:** Zircon's
debuglog, a handle-rights object write-only by default. Because §5 makes invocation uniform, once a
userspace UART driver exists the root hands out an endpoint speaking `WRITE`, no client changes, and the
Console object is **deleted** (increment 13). **Interim, named:** until then this is a device path
reachable from userspace inside the kernel — tiny, write-only and exercised by every boot-test, so not
VENOM's shape — but on real hardware at 115,200 baud a 504-byte write holds the non-preemptible kernel
for about 44 ms, a bound RFC-0006's latency budget must carry until the driver moves out.

## 13. `user/abi` — the kernel's own binding

**Which ledger row.** The libc/runtime row ("port code — relibc pieces") gives a program a conventional
environment: allocation, formatting, start-up conventions, POSIX-shaped calls. It does not issue
syscalls. relibc sits on `redox_syscall`, a separate crate versioned with the Redox kernel; seL4 draws the
same line, keeping libsel4 in the kernel's repository, generated from the same interface specifications
as the kernel, with runtimes elsewhere. The binding is the user half of §4's table — there is nothing
anywhere to port it from. **Verdict sought: the microkernel-core row, write ourselves.** A future relibc
port adds a Setonix platform layer *above* it; anything in `user/abi` that would read the same on another
kernel belongs to the runtime row and moves out. The maintainer may wish to append "syscall ABI binding
(`user/abi`)" to that row's parenthesis — constitution text, and his. If he reads it as the runtime row
instead, it is write-ourselves-by-necessity there, and that row's "port code" verdict needs a recorded
exception.

It holds syscall numbers; `Handle`, `Tag` and `Status`; the `IpcBuffer`, `BootInfo`, bundle and
descriptor layouts, with `const` layout assertions; and safe typed wrappers for the twelve syscalls and
each kernel object's methods — no allocator, no formatting, no start-block convention. **One source of
truth:** the kernel compiles the data half too, with `default-features = false` so the `syscalls` feature
that gates `src/arch/**` stays off, and `xtask` uses it for the bundle. Kernel, tools and userspace cannot
drift, because they compile the same constants *(STYLE.md § Single Source of Truth)*.

**The third designated `unsafe` tree**, approved in principle on 2026-09-27, is proposed as
`user/abi/src/arch/**`: the `svc`/`syscall` inline assembly; the `_start` stub, which turns `a0` into a
`&'static BootInfo` for the root and calls `main`; and the accessors whose soundness rests on a kernel
promise about this process's memory — the IPC-buffer view through its register, and the slice over a
Region just mapped into the caller (the root's loader writes child segments through it). User programs
contain no `unsafe`. The `CLAUDE.md` edit adds a third bullet to § `unsafe` policy — "`user/abi/src/arch/**`
— trap instructions, the process entry stub and the kernel-shared memory the ABI defines" — adds `user/` to
§ Layout, and changes `xtask`'s entry there from "no deps" to "no external deps"; the
`grep -rn "allow(unsafe_code)"` audit widens from `kernel/src/` to the repository.

## 14. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4** | the root task and boot info; invoking kernel objects as IPC, errors in the reply label; four physical message registers; `ReplyRecv`; libsel4 shipped with its kernel | CNodes and depth-addressed pointers; `DebugPutChar`; 120-word messages; the IPC-buffer thread-local |
| **Zircon / Fuchsia** | the process as an object holding a table and an address space; debuglog as a write-only handle | a syscall per operation; one handle at start as the only channel; negative status integers |
| **Linux** | `svc #0` + `x8`; `syscall` + `r10`; the canonical-`rcx` check; `nr_open` as a sizing reference | everything the registers carry |
| **x86_64 ELF TLS** | a thread block whose first word holds its own address | `FS` itself, left to TLS |
| **Hubris** | the build tool assembles one bootable image | a static task set with no spawn |
| **Redox** | `redox_syscall` versioned with the kernel, relibc above it | ambient path syscalls (research/0001) |
| **Genode** | boot modules handed to init as read-only memory | — |

## 15. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-1 unforgeability | handles reach the kernel only as words resolved through the caller's table; word 0 and out-of-range indices fail closed (§7) | **binds at B1** — RFC-0003's mechanism made live |
| O-2 non-widenability | `cap_derive` is the only duplication and is subset-only; `GRANT` and transfer move, never widen (§5, §9) | **binds at B1** |
| O-4 no ambient authority | twelve syscalls, each handle-indexed or authority-free; console by capability (§5, §12) | **discharged at the ABI**; each new kernel method is checked at review |
| O-5 argument validation | no pointer arguments; reserved bits zero; single fetch; caps all-or-nothing; kernel-built `SPSR_EL1`; canonical `rcx` (§4, §6, §8) | **discharged for the syscall surface**, with RFC-0005 for user-memory access |
| O-6 kernel memory safety | one new designated `unsafe` tree, named and bounded (§13) | **stays Built**, with a wider grep |
| O-7 no unprivileged exhaustion | 512-byte copy bound; creation paid by a named Pool; fixed tables fail closed; the console's bounded hold named (§6, §7, §12) | **contributes** — RFC-0005 owns the accounting |
| O-9 no ambient side channel | result registers carry results or zero; no grant after start; no ambient console (§4, §9, §12) | **contributes** |
| O-13 W^X | the build refuses writable-and-executable segments; the kernel maps per descriptor (§11) | **contributes** — RFC-0005 owns enforcement |
| O-17 contained drivers | fault messages to a supervisor (§8) | **designed, deferred** — the interim stops the thread |
| O-27 explicit authority at spawn | empty table at birth; grants only by explicit move; window shut at first resume; root's grant enumerated; demo proves a denial (§9, §10, §19) | **discharged** |

## 16. Graves checked (§3)

- **Policy in the kernel.** The kernel runs one image and lists what it granted; what runs, holding what,
  is the root task's. The rejected manifest and `spawn_from_module` are this grave's two doors; the root's
  boot scheduling constant is the one named residue.
- **The catch-all right.** No root identity survives `eret`; no right is added; `WRITE` on a Process is
  authority over one object. The root holds much, but droppably and enumerably.
- **Baroque hierarchies; multi-copy IPC.** Handles stay flat indices with no receive window; the slow path
  copies once, frame to frame, and BootInfo is a flat list.
- **Bolted-on multicore.** Entry state is per-CPU from the first stub (`SP_EL1` and `TPIDR_EL1`; `swapgs`
  and per-CPU stacks); the single-fetch rule is written for a second core rewriting the buffer; the
  syscall path takes no global lock. Two cores on one table stays RFC-0003 §14.2's question.
- **Drivers pulled in; unused device paths.** The Console is the kernel's own diagnostic UART, exposed
  write-only as a named interim with its deletion scheduled; it is the only device method reachable from
  EL0, and every boot-test uses it.

## 17. Costs — what this makes harder

- **`call` is overloaded:** per-type method decoding and two error layers — the price of interposition.
- **Four physical registers:** past 32 bytes a message touches memory; raising the count breaks binaries.
  A 65–120-word message seL4 would copy needs a Region here.
- **Handles are 20/44 for good,** and the accepted `capability` crate changes twice: a slot-generation
  bound and the `ObjectDestroyed` split.
- **Two thread registers belong to the ABI:** `TPIDRRO_EL0` and the user GS base are no longer free for a
  runtime.
- **The root task is the most sensitive process.** Before it drops authority, a bug there compromises the
  system, as in seL4. It must stay small.
- **No ambient printing:** a process without a Console fails silently. **Late grants need a willing
  child:** after the window closes, only IPC delivers authority; and the window detects no prior use.
- **Embedding couples builds:** a user change relinks the kernel, the kernel's reproducibility now
  includes the programs', and `xtask` gains its first dependency, in-workspace.
- **No PAN on the demo CPU,** and a canonical check on every x86_64 `sysretq`.

## 18. Open questions

1. **Fault messages** — their layout (this RFC's) and the thread's fault binding (RFC-0006's).
2. **Observing death.** A supervisor needs to learn a process died (O-17); a notification bound at
   creation is the likely shape.
3. **Badges.** `cap_derive`'s badge word and the badge output register are reserved zero until RFC-0003a.
4. **The console after Phase 2** — whether any Console capability stays mintable once a userspace driver
   owns the UART.
5. **The `unsafe` tree's name.** Two of its items are memory, not architecture; the maintainer may prefer
   a sibling `src/kernel_mem/` beside `src/arch/`.
6. **The 64-word budget** — a verdict now, to be revisited with the first measurements.

## 19. Implementation increments

Each is one reviewable PR; **[demo]** marks what the Phase-1 demo needs. Userspace boot-tests expect
prefixed strings, since bare `Kaya!` already matches the kernel's greeting. The demo carries three named
interims, each costed above: faults print and stop rather than reaching a supervisor (§8); the Console is
a device path inside the kernel (§12); boot modules come from the kernel image (§11) — plus RFC-0005's
static object pools and the root's boot scheduling constant (§10). The root task doubling as the demo's
server is not an interim: it is userspace composition, and yields exactly the two EL0 processes asked for.

1. **[demo] `capability`: the handle word.** `to_word`/`from_word`, the slot-generation bound, the
   `ObjectDestroyed` split. Host tests: round trips; word 0 and index ≥ 2^20 rejected; a slot retiring at
   the bound while an object generation passes it; use-after-close and destruction told apart.
2. **[demo] `user/abi`, data half.** Numbers, `Tag`, `Status`, `IpcBuffer`, `BootInfo`, bundle and
   descriptor types, with `const` layout assertions; no `unsafe`. Host tests: every reserved-bit and
   over-length encoding rejected. CI builds it for both targets.
3. **[demo] AArch64 trap path, MMU off.** Lower-EL frame save, dispatcher, restore, `eret`; `yield` and
   `thread_exit`; every other number `InvalidSyscall`; lower-EL faults stop the thread, not the core. A
   feature like `provoke-exception`, `syscall-selftest`, drops to EL0 with the MMU still off — the
   softfloat target is `+strict-align`, so EL0's Device-memory data accesses are safe — and executes
   `svc #0`. Boot-test `--features syscall-selftest --expect "syscall round trip"`. Lands before RFC-0005.
4. **[demo] `user/abi/src/arch/**`** — trap stubs, `_start`, the buffer register read — **with the
   `CLAUDE.md` edit in the same PR**. Built for both targets under clippy; first executed by increment 7.
5. **[demo] The bundle.** `xtask`'s ELF reader and descriptor writer, host-tested on fixtures including
   writable-and-executable, overlapping, truncated and wrong-machine images; `build.rs` and `.payload`.
   Boot-tests: a bare build `--expect "boot modules: 0"`; `cargo xtask` `--expect "boot modules: 2"`.
6. **[demo] Objects, `cap_*` and the Console,** on RFC-0005's static interim tables: the self-test's EL0
   code prints through a Console handle. Boot-test `--features syscall-selftest --expect "[el0] Kaya!"`.
7. **[demo] The root reaches EL0** on RFC-0005's address spaces and RFC-0006's threads: construction
   (§10), BootInfo, entry state. Boot-test `--expect "[server] up"`.
8. **[demo] The Process object.** Grant window and teardown as pure logic in a host-tested `process/`
   crate, then wired in. Host tests: born empty; `GRANT` all-or-nothing; `WindowClosed` after first
   resume; `KILL` empties the table.
9. **[demo] The root spawns the client,** granting a `WRITE`-only Console and the endpoint. The client
   prints `[client] up`, then invokes a handle word it was never granted, receives `InvalidHandle` and
   prints `[client] denied, as expected` — O-27 proved observably, not asserted.
10. **[demo] IPC syscalls** over RFC-0004 endpoints, with RFC-0006's blocking states. Boot-test lines
    in order: `[client] -> Kaya!`, `[server] <- Kaya!`, `[client] <- Kaya!`; RFC-0006's increments add
    the preemption evidence and the `BudgetRefused` and `BudgetExpired` tests.
11. **Fault messages and death notification** (§18.1–2). Boot-test: a deliberately faulting child is
    reported to the root, which prints `[server] child faulted`.
12. **x86_64 trap path and `user/abi` stubs:** `EFER.SCE`, `LSTAR`/`STAR`/`FMASK`, `swapgs`, the
    canonical check with `iretq` fallback. Proves §4's second column links; it boots with the UEFI stub.
13. **Delete the Console object** when the Phase-2 UART driver lands — scheduled now so it stays interim.

## 20. What this unblocks

The first userspace, and with it every Phase-2 paper: the **scheme registry** and the **broker** (both
built from processes, grants and `call`), the **driver framework** (fault messages, interrupt
notifications, the console moving out), a **relibc port** (a platform layer over `user/abi`, with both
TLS registers still its own), and the **x86_64 bring-up**, whose trap path is already on paper. RFC-0003a
gains a concrete ABI to place badges in.
