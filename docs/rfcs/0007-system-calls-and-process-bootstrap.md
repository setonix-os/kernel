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
> 4. **IPC-buffer discovery:** a kernel-set, user-read-only register — `TPIDRRO_EL0`; the user GS base with a self-pointer at `gs:[0]` (§6).
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

RFC-0003 §9 fixed the rule (resources only by handle; yielding, halting oneself and self-queries are the
authority-free exceptions) and RFC-0004 §10 the IPC semantics, including its 2026-08-01 amendment's
up-front minimum-budget refusal and error completion; both left the encoding here. This RFC designs the
encoding, the trap, the process as a kernel object, and the bootstrap seam RFC-0003 §14.4 named: minting
the first capabilities without an ambient grantor. It does not design address spaces (RFC-0005), threads
or scheduling (RFC-0006), or any policy about which programs run holding what — that is the root task's,
in userspace, and later the broker's.

## 2. Which pillar

**Pillar 2 and the kernel doctrine that capabilities are the only authority.** The syscall surface is the
whole of B1; one entry that names a resource without a handle makes RFC-0003's table decorative. Spawn is
where research/0002 found the freshest scar (Part 4: runc CVE-2024-21626, one inherited descriptor became
host traversal), and O-27 exists because of it. Pillar 1 is served in passing: boot images reach
userspace as read-only regions, immutable from their first instruction.

## 3. Entering the kernel

**AArch64 — `svc #0`.** From EL0, `svc` takes a synchronous exception to `VBAR_EL1 + 0x400` (lower EL,
AArch64 — entry 8 of the table `vectors.s` installs) with `ESR_EL1.EC = 0x15`, the immediate in
`ISS[15:0]` and `ELR_EL1` already past the `svc`; entry masks D, A, I and F and selects `SP_EL1` *(Arm ARM
DDI 0487)*. The stub saves `x0`–`x30`, `SP_EL0`, `ELR_EL1` and `SPSR_EL1` into the thread's frame
(RFC-0006 (proposed) provides its layout, in the TCB) and dispatches on EC: `0x15` to the syscall
dispatcher, anything else is a *fault* (§7). A non-zero immediate answers `InvalidArgument`, so a later
ABI revision is detectable. Return is `eret` with an `SPSR_EL1` the kernel builds: `M` = EL0t, DAIF clear,
NZCV from the frame; `DIT` (Armv8.4) and `SSBS` (Armv8.5), which EL0 can set itself with `msr`, are
carried where implemented; every other bit (`SS`, `IL`, `PAN`, `UAO`, `TCO`, `BTYPE`) is zero. The
Cortex-A72 implements neither, so on the demo CPU only NZCV crosses. *(Linux arm64 and seL4 both use
`svc #0`.)*

**x86_64 — `syscall` / `sysretq`.** `syscall` needs `IA32_EFER.SCE`; it loads `RIP` from `IA32_LSTAR`,
leaves the return `RIP` in `rcx` and `RFLAGS` in `r11`, clears the `RFLAGS` bits in `IA32_FMASK`, takes
`CS`/`SS` from `IA32_STAR` — and does **not** switch stacks *(Intel SDM Vol. 3A, "Fast System Calls in
64-Bit Mode")*. The stub begins with `swapgs`, parks the user `rsp` in per-CPU scratch and loads the
kernel stack, never dereferencing the user's. `FMASK` clears IF, DF, TF and AC: interrupts off, string
direction sane, user access closed where SMAP exists. Two hazards are designed in, not found later: a
**non-canonical `rcx`** makes `sysretq` raise #GP *in ring 0* on the user's stack on Intel parts —
CVE-2012-0217, which took Xen, FreeBSD, NetBSD and Windows — so although RFC-0005 (proposed) leaves the
top user page unmapped and RFC-0006 (proposed)'s `write_regs` refuses such a `RIP`, the exit path checks
anyway and falls back to `iretq`; and **the window before the stack switch**, where an NMI or machine
check would run on a user-chosen `rsp`, gets IST stacks from RFC-0006 (proposed). `sysretq` fixes the GDT
order (user data at `STAR[63:48] + 8`, code at `+ 16`). **Rejected:** `sysenter`, which AMD does not
implement in long mode, and an `int` gate, which pays a full IDT delivery per call.

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
`rax`, `rsi`, `rdx`, `r10`, `r8`, `r9`, `r12`, `r13`) carry a result or zero, so no kernel value leaves in
a register (O-9 hygiene). `x8`/`rax` follow Linux, leaving the AAPCS64 argument registers to the message;
the x86_64 column avoids `rbx` and `rbp`, which Rust inline assembly cannot name, and the two `syscall`
destroys. seL4 has the same shape in other seats — `x7`; `rdx` *(libsel4 `sel4_arch/syscalls.h`)*.

**Entry state of every thread,** root included, on both architectures: PC = entry; SP as given; `a0` and
`a1` — `x0`/`x1`, `rdi`/`rsi`, so `_start` is an ordinary two-argument `extern "C"` function — the two
start values; every other general register zero; EL0t with DAIF clear (`SPSR_EL1` = 0), or ring 3 with
`RFLAGS` = `0x202`; FP/SIMD trapped (`CPACR_EL1.FPEN` = 0; `CR0.TS` set) until RFC-0006 (proposed)
enables it for the thread; `TPIDR_EL0` and `FS_BASE` zero; the IPC-buffer register set (§5). A Thread is
created in this state; the spawner writes PC, SP, `a0` and `a1` through RFC-0006 (proposed)'s
`write_regs`, and the kernel writes the root's (§9).

**No syscall argument is a pointer.** The only user memory the syscall path touches is the IPC buffer,
reached by frame (§5). On the demo CPU that discipline is the whole defence: the Cortex-A72 is Armv8.0
and lacks PAN (Armv8.1).

## 4. The syscall set

- **A — one syscall per operation** *(Zircon, Linux)*. Legible per call, but the table grows with every
  object type, each entry is a new path to audit, and no server can ever stand in for a kernel object.
- **B — uniform invocation** *(seL4; KeyKOS before it)*. A handful of IPC syscalls; an operation on a
  kernel object is a `call` on its capability, method in the tag's label, answered as a server would.
- **C — B plus an `invoke` number.** Keeps the small table and loses the point: the caller can again tell
  a kernel object from a server.

**Verdict sought: B.** The payoff is interposition. A Console capability and an endpoint to a logging
server are invoked alike, so Phase 2's UART driver takes over the console protocol without a client
changing, and the broker can hand out a forwarder in place of any kernel object — RFC-0003a option (a)'s
precondition. A `call` on a kernel object runs on the caller's own time: nothing is donated or blocks.

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

`reply_recv` fuses two RFC-0004 operations so a server loop is one kernel entry, as the direct-switch
fast path needs; `notify` is `send` on a notification, which never blocks. The `cap_*` calls take and
return a zero tag and act only on the caller's own table, so §8 needs no table capability; a rights word
with an undefined bit answers `InvalidArgument`, and the dispatcher builds `Rights` from the crate's named
constants, so the crate never accepts raw bits. Transfer is always a move; a sender that keeps a copy
runs `cap_derive` first (RFC-0003 §6). **Creation** is a method on a Pool capability, which RFC-0005
(proposed) provides; per-type parameters are encoded here, and an endpoint's creation carries
**`min_budget`** in MR1 — RFC-0006 (proposed)'s time unit, 0 for none, fixed for the endpoint's life.
**Rejected:** a `debug_putc` (§11); anything taking a PID, path or address; seL4's depth-addressed
capability pointers, which exist for CNode trees this kernel does not have.

## 5. Messages, the register budget and the IPC buffer (RFC-0004 §9.4)

The **tag** is one word: bits 0–6 length in words (0–64); bits 7–8 capabilities attached (0–3); bit 9 set
by the kernel when a bound notification woke a `recv` (MR0 then holds the pending bits); bits 10–15
reserved, zero; bits 16–63 a 48-bit label — user-defined for endpoints, the method number for kernel
objects, the method result on a kernel reply. *(seL4's `seL4_MessageInfo`: label, extra caps, length.)*

**Four physical message registers on both architectures:** seL4 uses four on both 64-bit targets,
research/0001 found the in-register payoff small and shrinking (10% on ARM11, negative on modern x86),
and a count that differed by architecture would make one message fast on one Tier-1 target only.
Messages up to **64 words** spill MR4–MR63 to the buffer — below seL4's 120 on purpose: 512 bytes bounds
the non-preemptible copy, and RFC-0004 §5 sends bulk through shared regions. **Three capabilities**, a
2-bit field with no invalid encodings. **Rejected:** six physical registers on AArch64 (portability);
seL4's 120 words (a longer non-preemptible copy for payloads RFC-0004 routes elsewhere).

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

**The buffer is kernel-reachable memory, named by capability.** It is a 1 KiB-aligned slice of one page
of a Region, bound when the thread is created by the Region capability (`READ | WRITE`), the offset, and
the user address where the spawner mapped it; RFC-0006 (proposed) records the binding, RFC-0005
(proposed) provides the Region and the physmap. The kernel reads and writes it only through its own
mapping of that frame; the user address serves word 0 and the discovery register and is never
dereferenced. Received capabilities land in fresh slots (RFC-0004 §6): no seL4-style receive window.

**Discovery: a kernel-set, user-read-only register.** The kernel loads the buffer's user address into
`TPIDRRO_EL0` (readable at EL0, writable only at EL1) and into the user GS base on x86_64, where
`CR4.FSGSBASE` stays off (RFC-0006 (proposed)) and `abi` reads the self-pointer at `gs:[0]` — the x86_64
ELF TLS convention of a block whose first word holds its own address *(Drepper, "ELF Handling For
Thread-Local Storage")*. RFC-0006 (proposed) already restores both on every switch and leaves their use
here, so `_start` has its buffer at its first instruction, in any thread, with no TLS runtime. The kernel
trusts neither: a thread that reloads its own GS selector misleads only itself. **Rejected:** `TPIDR_EL0`
and `FS_BASE`, the thread pointers of the AArch64 and x86_64 ELF TLS ABIs, which a relibc port needs
untouched; and seL4's current answer, a thread-local (`__sel4_ipc_buffer`, libsel4 `functions.h`) —
sound, but it needs TLS before the first syscall.

**Single fetch (O-5).** Once SMP exists another thread of the process can rewrite the buffer
mid-syscall. So every buffer word the kernel acts on is read **exactly once** into kernel memory before any
check depends on it; lengths and counts come from the tag register, already in the frame, bounded by 64
and 3; the `caps[]` words are copied into a kernel array and all are resolved and checked for `TRANSFER`
before any moves (RFC-0004 §6); payload words go frame to frame through RFC-0005 (proposed)'s
volatile-copy module, never interpreted, and no Rust reference is ever formed into the buffer.
*(research/0002 Part 5, virtio's double-fetch lesson: "validate a copied-out snapshot; never re-read".)*

## 6. Handles in one register

RFC-0003's `Handle` is a `u32` index and a 64-bit generation — ninety-six bits, which fit no register.
The ABI word is **`slot_generation << 20 | index`**: 2^20 slots per process, Linux's default descriptor
ceiling (`fs.nr_open` = 1,048,576); and 2^44 slot generations, about two hundred days of one
close-and-reopen per microsecond on one slot before it retires. `Generation::FIRST` is 1, so word `0` is
never live and serves as null. This is the width RFC-0003's 2026-07-30 amendment asked to be stated.
**Only the slot generation crosses the ABI;** a capability's object generation stays in kernel memory. So
the accepted crate changes narrowly: `Handle::to_word`/`from_word` and `Handle::MAX_SLOT_GENERATION`
(2^44 − 1); `CapabilityTable::remove` retires a slot whose next generation would exceed it, through the
`Retired` state it already has; a const assertion keeps every kernel table's `N` ≤ 2^20. `Generation` is
untouched — objects keep 64 bits — and its doc comment gains the slot bound. Phase 1 tables have
`N` = 64, fixed, failing closed. **Rejected:** two registers per handle (halves the message budget); 32/32
(2^32 reuses is seventy minutes at one per microsecond, and the LIFO free list concentrates reuse on one
slot); a 16-bit index (too few for the server workloads the constitution targets).

## 7. The error model

**Transport status** (`x0`/`rax`) is what the syscall layer found, identical for a kernel object or a
server. Every failure is atomic — nothing moved, nothing half-sent — and the detail register names the
culprit: 0 the invoked handle, 1–3 an attached capability, 4 the tag, 5 the reply handle.

| Status | Meaning |
|--------|---------|
| `Ok` | done |
| `InvalidSyscall` | no such number — a closed error, not a fault: the surface stays total and needs no fault-handler policy |
| `InvalidArgument` | a reserved bit, a length over 64, a non-zero `svc` immediate, an undefined rights bit |
| `InvalidHandle` | word 0, index out of range, empty slot, stale slot generation — the handle names nothing *of yours* (the crate's `OutOfBounds`, `Empty`, `StaleGeneration`) |
| `Revoked` | the slot is live but its object was destroyed — the crate's new `ObjectDestroyed`; close the handle |
| `WrongType` / `InsufficientRights` | live handle, wrong kind of object; or lacking the right the operation needs |
| `TableFull` | a moved capability found no slot in the receiving table; nothing moved |
| `WouldBlock` | `nb_send` or `nb_recv` found no partner |
| `PeerGone` | the object or party waited on was destroyed mid-wait |
| `BudgetRefused` | a `call` whose donated context can never meet the endpoint's `min_budget`, refused up front (RFC-0004's amendment; seL4 RFC-14) |
| `BudgetExpired` | the donated budget expired inside the server; the kernel completed the wait on the reply object (RFC-0004's amendment) |

`Revoked` needs the second crate change: `StaleGeneration` today covers both a reused slot — the holder's
own use-after-close — and a destroyed object. `resolve` already checks them in that order, so the split is
one variant and one changed return, and it lets a holder observe and survive destruction, as
research/0002 asks of revocation, without reporting its own bug as something done to it. When the two
budget statuses arise is RFC-0006 (proposed)'s to define; its draft names the first
`BudgetBelowThreshold`, and the name here is proposed because the encoding is this RFC's.

**Method result** (the reply label) is what the object said, written by the kernel for kernel objects and
by the server for endpoints, in one vocabulary — so a stand-in server reports errors the same way *(seL4
returns invocation errors in the reply's label)*. It reuses the status numbers where meanings coincide and
adds `InvalidMethod`, RFC-0005 (proposed)'s `PoolExhausted` and `Fragmented`, and §8's `WindowClosed`.

**Faults** — EL0 exceptions that are not syscalls — are not return values. The design suspends the thread
and sends a fault message to a handler RFC-0006 (proposed) binds, so a supervisor can restart a driver
(O-17). **Interim:** the reporter prints the fault, prefixed by the thread; the thread stops, the core
continues, nothing restarts it.

## 8. The Process object and spawning (O-27)

A **Process** holds one RFC-0003 capability table, one AddressSpace (RFC-0005 (proposed)) and its threads
(RFC-0006 (proposed)) — a real object *(Zircon)*, not seL4's convention of a thread naming a CSpace and a
VSpace. Three ways to give it its first authority:

- **A — inherit** *(Unix `fork`/`exec`)*. **Rejected** by O-27 by name: ambient authority smuggled across
  creation.
- **B — one bootstrap handle** *(Zircon: `zx_process_start` moves one handle; a loader sends the rest as
  processargs)*. Minimal, but over Setonix's unbuffered rendezvous the parent must serve its own child a
  protocol before the child can act, holding a bootstrap endpoint per child.
- **C — explicit grants before start** *(seL4's root task writing a child's CSpace)*. **Verdict sought.**

A Process is created through RFC-0005 (proposed)'s Pool, naming an AddressSpace, with an **empty table**:

| Method | Right | Effect |
|--------|-------|--------|
| `GRANT` | `WRITE` on the process; `TRANSFER` on each attached capability | move up to three attached capabilities into the child's table; the reply's MR0–MR2 carry their handle words *in the child* |
| `KILL` | `WRITE` | stop every thread, drop the table, release the address space; the generation bump makes every capability to the process inert |

`GRANT` is a `call` on the Process with capabilities attached: the kernel takes them from `caps[]` exactly
as an endpoint send does (§5) — fetched once, all resolved before any moves.

**The grant window.** `GRANT` succeeds only until the first `resume` of any thread bound to the process —
a one-way event RFC-0006 (proposed) reports by setting a flag on the Process; afterwards it answers
`WindowClosed`. From then on authority arrives only by IPC, which the process consents to by receiving: a
parent cannot reach into a running child's table, and a stolen Process capability cannot inject authority
into a live process. The window is **single-use** in research/0002's sense (Part 3, from Vault's
response-wrapping) but gives none of its **prior-use detection** — the child is not told who granted what.
Inside the kernel the channel is the Process capability, minted to its creator alone; detection across
imperfectly trusted spawn channels belongs to the broker RFC, which owns those channels.

The child learns its handles as a program learns its arguments: the spawner writes them into `a0` and
`a1` (the demo's console and endpoint), or points `a0` at a start block — a runtime-row convention, not
kernel ABI. No right joins RFC-0003's set: `GRANT`, `KILL` and thread creation all use `WRITE` on that one
Process, held by its creator — not a catch-all. A process holds no capability to itself unless handed one,
and is destroyed when its last thread exits.

## 9. The root task and boot info

The one act the kernel performs unasked: at boot it builds **exactly one** process from the first boot
module and gives it every boot capability *(seL4's root task; research/0002 Part 3: "the bootstrap seam is
a one-time kernel act")*: an AddressSpace; the descriptor's segments copied into fresh frames and mapped
as stated (§10); a stack; an IPC buffer; a read-only **BootInfo** page; a Process and a Thread in §3's
entry state with `a0` = BootInfo's address and `a1` = 0; `eret`. RFC-0006 (proposed) provides the root's
first scheduling context, its parameters a named boot constant — as seL4's root starts at the maximum
priority with one configured timeslice.

**BootInfo** is kernel ABI, in `user/abi`: a header (magic, version, length) and one entry per initial
capability — `{ kind, flags, handle, base, size }` — with a string table for module names, pinned within
one page by a host test. Every initial capability is *listed*; none sits at a well-known slot, so the
order can change without an ABI break and a reviewer can enumerate the whole bootstrap grant. This RFC
defines three kinds: the root's own Process, the Console (§11), and one read-only Region per further boot
module, with its load descriptor. RFC-0005 (proposed) provides the AddressSpace, the root Pool over all
free RAM and the DTB Region; RFC-0006 (proposed) the Thread, `SchedControl` and interrupt capabilities.

**The root is not a superuser.** It holds everything at first, as ordinary, droppable, generation-checked
capabilities, and the rule that keeps it so is checkable: **no kernel code branches on "is this the root
task"**. The bootstrap graph is acyclic by construction — kernel to root, root to children — and nothing
the root needs waits on a userspace service. **Rejected:** a kernel boot manifest of programs and grants
(policy in the kernel); a `spawn_from_module` syscall (a loader offered as a kernel service); and a
bootstrap superuser, research/0002's `system:masters`.

## 10. Getting the first images into memory

QEMU's `-kernel` loads one ELF. The candidates for the rest:

- **Embed them in the kernel ELF** *(verdict sought)*. One artefact boots; its hash pins every program.
- **QEMU's `-initrd` or `-device loader`.** Two artefacts, QEMU-only — and neither reaches a bare ELF. In
  `hw/arm/boot.c` (read at QEMU 8.2.2), `arm_setup_direct_kernel_boot` loads an `-initrd` only for images
  it treats as Linux, never an ELF, and for a non-Linux image `do_cpu_reset` sets the PC and nothing else:
  no register names the DTB either, which QEMU puts at the base of RAM only if the image leaves room.
  Today's image *is* the base of RAM, so it gets none; RFC-0005 (proposed) relinks to `0x4020_0000` for
  that. A `-device loader` blob lands where only the command line knows.
- **Link the children into the root's ELF** *(common seL4 practice, a CPIO archive)*. The root becomes
  specific to each image, and the children stop being separate immutable objects it can hand on.

**`xtask` parses ELF; the kernel does not.** `xtask` reads each program's headers with a dependency-free,
host-tested ELF64 reader and emits a fixed-shape **load descriptor**: the entry point; a text segment
(`R+X`), an optional read-only segment (`R`) and a data segment (`R+W`, with a `bss` size), page-aligned in
the user half; the stack, IPC-buffer and BootInfo addresses. It **refuses** at build time any
writable-and-executable, overlapping or unaligned segment, dynamic linking, TLS or any other program
header — W^X checked before the image exists (O-13). The kernel checks one fixed structure against fixed
bounds and never sees an ELF: Nexen's finding that kernel code validating structures built elsewhere is
the bug farm (research/0002 Part 7), applied to our own boot path. **Rejected:** an in-kernel `elf/`
crate — small and host-testable, but a parser in the TCB for no gain.

**The bundle.** `xtask` builds the user crates, writes `payload.bin` — a versioned `abi` header and per
module a name, descriptor and segment bytes — and builds the kernel with `SETONIX_PAYLOAD` naming it;
`kernel/build.rs` places it page-aligned in a read-only `.payload` section (`KEEP`). **Unset, it emits an
empty bundle**, so a bare `cargo build` or `cargo clippy --package setonix-kernel` still links, boots and
greets, printing `boot modules: 0` before halting as today. *(Hubris's `xtask dist`; seL4's elfloader,
which parses before the kernel runs.)*

The kernel sees modules through one arch-neutral function, `arch::boot_modules()`: the embedded bundle on
AArch64, files the UEFI stub loaded on x86_64 — so embedding is Phase 1's *source*, and the kernel's
contract is a range plus descriptors. It loads **entry 0 only**; every other entry reaches the root as a
**read-only Region** over its pages (RFC-0005 (proposed) provides the frames) with its descriptor in
BootInfo, so the root copies segments out without parsing anything and the image stays immutable
(pillar 1). *(Genode's core hands boot modules to init as read-only ROM modules.)*

**The programs** are `no_std`, `no_main` workspace crates — `user/demo/server`, `user/demo/client` —
depending only on `abi`, with no `unsafe`, linked static and non-PIE by `user/link/<arch>.ld` in RFC-0005
(proposed)'s user half. `x86_64-unknown-none` defaults to position-independent output and the *kernel*
code model, so `xtask` passes `-C relocation-model=static -C code-model=small` for user crates, in their
own target directory; the AArch64 target is static already. They stay soft-float, as the demo needs.

## 11. The console as a capability

Userspace reaches the kernel's UART only through a **Console** kernel object with one method, `WRITE`,
by `call`: the byte count in MR0, the bytes from MR1 on — up to 504 per call, so a short line never leaves
the registers — needing the `WRITE` right. The root receives it with `DUPLICATE | TRANSFER | WRITE` (no
`READ`: there is no input path) and derives `WRITE`-only copies for the children it chooses. A program not
handed one cannot print, which is the point.

**Rejected:** seL4's `DebugPutChar`, which any thread may issue in a `CONFIG_PRINTING` build — ambient
authority, however small — and a console at a well-known handle. **Adopted in spirit:** Zircon's debuglog,
a handle-rights object write-only by default. Because §4 makes invocation uniform, once a userspace UART
driver exists the root hands out an endpoint speaking `WRITE`, no client changes, and the Console object is
**deleted** (increment 13). **Interim, named:** until then a device path inside the kernel is reachable
from userspace — tiny, write-only, exercised by every boot-test — and on real hardware at 115,200 baud a
504-byte write holds the non-preemptible kernel for about 44 ms, a bound RFC-0006's latency budget carries.

## 12. `user/abi` — the kernel's own binding

**Which ledger row.** The libc/runtime row ("port code — relibc pieces") gives a program an environment:
allocation, formatting, start-up conventions, POSIX-shaped calls. It does not issue syscalls. relibc sits
on `redox_syscall`, versioned with the Redox kernel; seL4 keeps libsel4 in the kernel's repository,
generated from the kernel's own interface specifications, with runtimes elsewhere. The binding is the user
half of §3's table — there is nothing to port it from. **Verdict sought: the microkernel-core row, write
ourselves.** A future relibc port adds a platform layer *above* it; anything in `user/abi` that would read
the same on another kernel moves out. The maintainer may wish to append "syscall ABI binding
(`user/abi`)" to that row — constitution text, and his. If he reads it as the runtime row instead, it is
write-ourselves-by-necessity there, and that row's "port code" verdict needs a recorded exception.

It holds syscall numbers; `Handle`, `Tag` and `Status`; the `IpcBuffer`, `BootInfo`, bundle and
descriptor layouts, with `const` layout assertions; and safe wrappers for the twelve syscalls and each
kernel object's methods — no allocator, no formatting. **One source of truth:** the kernel compiles the
data half with `default-features = false`, so the `syscalls` feature gating `src/arch/**` stays off, and
`xtask` uses it for the bundle; kernel, tools and userspace compile the same constants.

**The third designated `unsafe` tree**, approved in principle on 2026-09-27, is proposed as
`user/abi/src/arch/**`: the `svc`/`syscall` assembly; the `_start` stub, which turns `a0` into the root's
`&'static BootInfo` and calls `main`; and accessors whose soundness rests on a kernel promise about this
process's memory — the IPC-buffer view, and the slice over a Region just mapped into the caller (the root's
loader writes child segments through it). User programs contain no `unsafe`. **The `CLAUDE.md` edit**
adds to § `unsafe` policy "`user/abi/src/arch/**` — trap instructions, the process entry stub and the
kernel-shared memory the ABI defines", adds `user/` to § Layout and changes `xtask`'s "no deps" to "no
external deps"; the `grep -rn "allow(unsafe_code)"` audit widens from `kernel/src/` to the repository.

## 13. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4** | the root task and boot info; kernel objects invoked as IPC, errors in the reply label; four physical message registers; `ReplyRecv`; libsel4 shipped with its kernel | CNodes and depth-addressed pointers; `DebugPutChar`; 120-word messages; the IPC-buffer thread-local |
| **Zircon / Fuchsia** | the process as an object holding a table and an address space; debuglog as a write-only handle | a syscall per operation; one start handle as the only channel; negative status integers |
| **Linux** | `svc #0` + `x8`; `syscall` + `r10`; the canonical-`rcx` check; `nr_open` as a sizing reference | everything the registers carry |
| **x86_64 ELF TLS** | a block whose first word holds its own address | `FS` itself, left to TLS |
| **Hubris** | the build tool assembles one bootable image | a static task set with no spawn |
| **Redox** | `redox_syscall` versioned with the kernel, relibc above it | ambient path syscalls (research/0001) |
| **Genode** | boot modules handed to init as read-only memory | — |

## 14. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-1 unforgeability | handles reach the kernel only as words resolved through the caller's table; word 0 and out-of-range indices fail closed (§6) | **binds at B1** — RFC-0003's mechanism made live |
| O-2 non-widenability | `cap_derive` is the only duplication and is subset-only; `GRANT` and transfer move, never widen (§4, §8) | **binds at B1** |
| O-4 no ambient authority | twelve syscalls, each handle-indexed or authority-free; console by capability (§4, §11) | **discharged at the ABI**; each new kernel method checked at review |
| O-5 argument validation | no pointer arguments; reserved bits zero; single fetch; caps all-or-nothing; kernel-built `SPSR_EL1`; canonical `rcx` (§3, §5, §7) | **discharged for the syscall surface**, with RFC-0005 for user memory |
| O-6 kernel memory safety | one new designated `unsafe` tree, named and bounded (§12) | **stays Built**, with a wider grep |
| O-7 no unprivileged exhaustion | 512-byte copy bound; creation paid by a named Pool; fixed tables fail closed; the console's hold bounded (§5, §6, §11) | **contributes** — RFC-0005 owns the accounting |
| O-9 no ambient side channel | result registers carry results or zero; no grant after start; no ambient console (§3, §8, §11) | **contributes** |
| O-13 W^X | the build refuses writable-and-executable segments; the kernel maps per descriptor (§10) | **contributes** — RFC-0005 owns enforcement |
| O-17 contained drivers | fault messages to a supervisor (§7) | **designed, deferred** — the interim stops the thread |
| O-27 explicit authority at spawn | empty table at birth; grants only by explicit move; window shut at first resume; root's grant enumerated; the demo proves a denial (§8, §9, §18) | **discharged** |

## 15. Graves checked (§3)

- **Policy in the kernel.** The kernel runs one image and lists what it granted; what runs, holding what,
  is the root's. The rejected manifest and `spawn_from_module` are this grave's two doors; the root's boot
  scheduling constant is the one named residue.
- **The catch-all right.** No root identity survives `eret`; no right is added; `WRITE` on a Process is
  authority over one object. The root holds much, but droppably and enumerably.
- **Baroque hierarchies; multi-copy IPC.** Flat handles, no receive window, a flat BootInfo; the slow path
  copies once, frame to frame.
- **Bolted-on multicore.** Entry state is per-CPU from the first stub (`SP_EL1`, `TPIDR_EL1`; `swapgs`);
  single fetch is written for a second core rewriting the buffer; the syscall path takes no global lock.
  Two cores on one table stays RFC-0003 §14.2's question.
- **Drivers pulled in; unused device paths.** The Console is the kernel's own diagnostic UART, write-only,
  used by every boot-test and scheduled for deletion; it is the only device method reachable from EL0.

## 16. Costs — what this makes harder

- **`call` is overloaded:** per-type method decoding and two error layers — the price of interposition.
- **Four physical registers:** past 32 bytes a message touches memory, and a 65–120-word message seL4
  would copy needs a Region here; raising either count later breaks binaries.
- **Handles are 20/44 for good,** and the accepted `capability` crate changes twice.
- **Two thread registers belong to the ABI:** `TPIDRRO_EL0` and the user GS base are not a runtime's.
- **The root task is the most sensitive process:** until it drops authority, a bug there compromises the
  system, as in seL4. It must stay small.
- **No ambient printing,** so a process without a Console fails silently; **late grants need a willing
  child**, and the grant window detects no prior use.
- **Embedding couples builds:** a user change relinks the kernel, the kernel's reproducibility includes the
  programs', and `xtask` gains its first, in-workspace dependency.
- **No PAN on the demo CPU,** and a canonical check on every x86_64 `sysretq`.

## 17. Open questions

1. **Fault messages** — their layout (this RFC's) and the thread's fault binding (RFC-0006's).
2. **Observing death** — a supervisor must learn a process died (O-17); a notification bound at creation
   is the likely shape.
3. **Badges** — `cap_derive`'s badge word and the badge register are reserved zero until RFC-0003a.
4. **The console after Phase 2** — whether any Console capability stays mintable once a driver owns the UART.
5. **The `unsafe` tree's name** — two of its items are memory, not architecture; the maintainer may prefer
   a sibling `src/kernel_mem/`.
6. **The 64-word budget** — a verdict now, to be revisited with the first measurements.

## 18. Implementation increments

Each is one reviewable PR; **[demo]** marks what the Phase-1 demo needs. Userspace boot-tests expect
prefixed strings, since bare `Kaya!` already matches the kernel's greeting. The demo's interims, each
costed above: faults print and stop (§7); the Console is a device path in the kernel (§11); modules come
from the kernel image (§10); RFC-0005's static object pools; the root's boot scheduling constant (§9). The
root doubling as the server is not an interim: it is userspace composition, and yields the two processes.

1. **[demo] `capability`: the handle word** — `to_word`/`from_word`, the slot bound, `ObjectDestroyed`.
   Host tests: round trips; word 0 and index ≥ 2^20 rejected; a slot retiring at the bound while an
   object generation passes it; use-after-close and destruction told apart.
2. **[demo] `user/abi`, data half** — numbers, `Tag`, `Status`, `IpcBuffer`, `BootInfo`, bundle and
   descriptor types, `const` layout assertions, no `unsafe`. Host tests reject every reserved-bit and
   over-length encoding; CI builds it for both targets.
3. **[demo] AArch64 trap path, MMU off** — lower-EL save, dispatcher, restore, `eret`; `yield`,
   `thread_exit`, `InvalidSyscall`; faults stop the thread, not the core. A feature like
   `provoke-exception`, `syscall-selftest`, drops to EL0 with the MMU off — the softfloat target is
   `+strict-align`, so EL0's Device-memory accesses are safe — and runs `svc #0`. Boot-test
   `--features syscall-selftest --expect "syscall round trip"`. Lands before RFC-0005's first mapping.
4. **[demo] `user/abi/src/arch/**`** — trap stubs, `_start`, the buffer-register read — **with the
   `CLAUDE.md` edit in the same PR**. Built for both targets under clippy; first run by increment 7.
5. **[demo] The bundle** — `xtask`'s ELF reader and descriptor writer, host-tested on writable-and-executable,
   overlapping, truncated and wrong-machine fixtures; `build.rs`, `.payload`. Boot-tests: a bare build
   `--expect "boot modules: 0"`; `cargo xtask` `--expect "boot modules: 2"`.
6. **[demo] Objects, `cap_*` and the Console** on RFC-0005's static interim tables. Boot-test
   `--features syscall-selftest --expect "[el0] Kaya!"`, printed from EL0 through a Console handle.
7. **[demo] The root reaches EL0** on RFC-0005's address spaces and RFC-0006's threads: construction,
   BootInfo, entry state (§9). Boot-test `--expect "[server] up"`.
8. **[demo] The Process object** — grant window and teardown as pure logic in a host-tested `process/`
   crate, then wired in. Host tests: born empty; `GRANT` all-or-nothing; `WindowClosed` after first resume;
   `KILL` empties the table.
9. **[demo] The root spawns the client,** granting a `WRITE`-only Console and the endpoint. The client
   prints `[client] up`, invokes a handle word it was never granted, receives `InvalidHandle` and prints
   `[client] denied, as expected` — O-27 proved observably, not asserted.
10. **[demo] IPC syscalls** over RFC-0004 endpoints with RFC-0006's blocking states. Boot-test lines in
    order: `[client] -> Kaya!`, `[server] <- Kaya!`, `[client] <- Kaya!`. RFC-0006's increments add the
    preemption evidence and the `BudgetRefused` and `BudgetExpired` tests.
11. **Fault messages and death notification** (§17.1–2). Boot-test: a deliberately faulting child is
    reported to the root, which prints `[server] child faulted`.
12. **x86_64 trap path and `user/abi` stubs** — `EFER.SCE`, `LSTAR`/`STAR`/`FMASK`, `swapgs`, the
    canonical check with `iretq` fallback. Proves §3's second column links; boots with the UEFI stub.
13. **Delete the Console object** when the Phase-2 UART driver lands — scheduled now so it stays interim.

## 19. What this unblocks

The first userspace, and with it every Phase-2 paper: the **scheme registry** and the **broker** (built
from processes, grants and `call`), the **driver framework** (fault messages, interrupt notifications, the
console moving out), a **relibc port** (a platform layer over `user/abi`, both TLS registers still its
own), and the **x86_64 bring-up**, whose trap path is on paper. RFC-0003a gains an ABI to place badges in.
