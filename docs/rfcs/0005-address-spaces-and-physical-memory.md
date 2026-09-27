<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0005 — Address spaces and physical memory

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 (kernel doctrine); RFC-0003 (one new right, three new objects; `capability/src/rights.rs`); RFC-0004 §9.1; the boot stub, `aarch64.ld` and the exception reporter; `kernel/src/mm/**`; a new host-tested `memory/` crate; threat model §10 (one proposed assumption) |
| Depends on | RFC-0003 (capability table), RFC-0004 (IPC); interlocks with RFC-0006 (threads and scheduling) and RFC-0007 (system calls and process bootstrap), proposed the same day |
| Discharges | O-7 (the memory and object half), O-8, O-13 (the mapping half); O-5 for every path by which the kernel touches user memory; O-3's destruction case extended to memory; answers RFC-0003 §14.3 and RFC-0004 §9.1 |

> **Proposed verdicts.**
>
> 1. **Discovery:** RAM comes from firmware tables into one bounded `MemoryMap`; the image moves to 0x4020_0000, because QEMU gives an ELF at RAM base no DTB (§4).
> 2. **Frame allocator:** a bitmap over 4 KiB frames, colour-capable from day one; every frame zeroed before any process can see it (§4).
> 3. **Who pays (O-7):** typed fixed-capacity slot arrays are the object store; every frame and slot taken after boot is charged to a capability-named `Pool` (§5).
> 4. **Layout:** a higher-half kernel with the *same* layout on both architectures; 47-bit halves; a non-executable physmap; guarded kernel stacks (§6).
> 5. **Objects:** `Region` (eager, zeroed, at most 8 extents) and `AddressSpace` join RFC-0003's objects beside `Pool`; each mapping is a charged record (§7).
> 6. **Operations:** `map` / `unmap` / `protect` are mechanical, capability-checked, bounded and atomic; `protect` only narrows; the kernel never picks an address (§8).
> 7. **Rights:** `EXECUTE` joins RFC-0003's set as bit 5 — map-executable on a Region, mint-executable on a Pool, meaningless elsewhere (§8).
> 8. **W^X (O-13):** unrepresentable in the mapping type, enforced per Region across *all* address spaces, backed by `SCTLR_EL1.WXN` (§9).
> 9. **Sharing (RFC-0004 §9.1):** *share* is derive-and-transfer; *donate* is transfer made exclusive by `reissue`, a destruction-case operation (§10).
> 10. **User memory (O-5):** the kernel never dereferences a user virtual address, so no syscall takes a pointer argument, ever (§11).
> 11. **TLB:** ASIDs/PCIDs are kernel-private tags with generation rollover; maintenance is inner-shareable from the first line (§12).
> 12. **Early boot:** the stub enables the MMU with throw-away tables; Rust runs only at the high alias and installs the final W^X map (§13).

## 1. The question

**Where does memory come from, who pays for it, and how does a process reach exactly the memory it holds
capabilities for — never writable and executable at once, and without the kernel trusting an address a
process gave it?** Mechanism only: how much memory, which program may execute code, and where a heap goes
are userspace's.

## 2. Which pillar

**Pillar 2 and the doctrine that capabilities are the only authority, where authority becomes hardware.** A
`READ` capability that becomes a writable page is O-2 violated by the silicon. Pillar 1 lands here too: the
threat model files integrity *in memory* under A1, and W^X is how signed code stays what was signed.

## 3. What the hardware gives — one shape, two encodings

Both Tier-1 MMUs, as configured here, are one machine: **four levels of 512 eight-byte entries, 4 KiB pages,
nine index bits per level, and 47-bit user and kernel halves whose top level uses 256 entries each.**
AArch64 gets this from the 4 KiB granule with `TCR_EL1.T0SZ = T1SZ = 17` (a level-0 start); x86_64 from
IA-32e four-level paging without LA57. The differences are in the leaf encoding:

| Property | AArch64 (VMSAv8-64, 4 KiB granule) | x86_64 (IA-32e, 4-level) |
|---|---|---|
| User / kernel split | two roots: `TTBR0_EL1` (low), `TTBR1_EL1` (high) | one root, `CR3`; PML4 entries 0–255 user, 256–511 kernel |
| Present | bits 1:0 = `0b11` (valid page) at level 3 | bit 0 (P) |
| Writable | `AP[2]` (bit 7) clear | bit 1 (R/W) set |
| User-accessible | `AP[1]` (bit 6) set | bit 2 (U/S) set |
| No-execute | `PXN` (bit 53) and `UXN` (bit 54), separately | `XD` (bit 63), only with `IA32_EFER.NXE` |
| Global | `nG` (bit 11) clear | `G` (bit 8) set, with `CR4.PGE` |
| Accessed | `AF` (bit 10), set by software on this CPU | bit 5 (A), set by hardware |
| Address-space tag | 16-bit ASID in `TTBR0_EL1[63:48]` | 12-bit PCID in `CR3[11:0]` |

*(Bits checked against Linux's `arch/arm64/include/asm/pgtable-hwdef.h` — Arm's site was unreachable from
this session — and Intel SDM Vol. 3A §4.5. QEMU's `cortex-a72` has 44-bit physical addresses, 16-bit ASIDs,
no hardware access-flag update and no PAN: `target/arm/tcg/cpu64.c` sets `ID_AA64MMFR0_EL1 = 0x1124` and
leaves `ID_AA64MMFR1_EL1` at 0.)* Hence **one table engine**, generic over a per-architecture `Descriptor`
encoder, host-tested on both against a fake frame store in `memory/`; only turning a frame number into
`&mut [u64; 512]` through the physmap (§6) is `unsafe`, in `kernel/src/mm/`.

## 4. Physical memory: discovery and the frame allocator

**Discovery.** QEMU documents two fixed facts about `virt` — flash at 0, RAM at 0x4000_0000 — and says every
other address "may vary across QEMU versions" and must be read from the DTB, which a bare-metal ELF finds
"at the start of RAM" with no register pointing at it (`docs/system/arm/virt.rst`). But `hw/arm/boot.c`
puts it there only if the image leaves room below itself, and today's image *is* the base of RAM. Checked
on QEMU 8.2.2 on 2026-09-27: an ELF at 0x4000_0000 boots with **no DTB anywhere in RAM** and `x0 = 0`; at
0x4020_0000 it finds one (magic `0xd00dfeed`) at 0x4000_0000. **So the image loads at 0x4020_0000**, 2 MiB
in and block-aligned; increment 1 re-checks the pinned QEMU 11.0.3. A minimal FDT reader, written for the
purpose (the Borrow Ledger's boot-path row; Devicetree Specification v0.4), reads the cell sizes, `/memory`,
`/memreserve/` and `/reserved-memory`, bounds-checks every offset and fails closed. Threat model §10 names no
firmware assumption; this RFC proposes one — *firmware tables describe memory truthfully* — through
`docs/CHANGELOG.md`, as §10 already assumes the CPU. On x86_64 the UEFI stub (owned by no RFC yet) hands
over `GetMemoryMap`'s output. **Both produce one `MemoryMap`**: at most 64 sorted, disjoint `(PhysRange,
Kind)` entries — `Usable`, `KernelImage`, `DeviceTree`, `Reserved`, `Mmio` — failing closed beyond that,
with image, boot stack, DTB and boot tables tagged before the allocator sees it. Nothing above `arch` names
FDT or UEFI. The DTB passes to the root task read-only (RFC-0007 (proposed) provides the handover).

**The frame allocator is a bitmap**: one bit per 4 KiB frame, its storage taken from the first free frames
(16 KiB for 512 MiB). It allocates frames from a *colour set*, or runs of `n` at an alignment; a double
free fails closed. Colour is `pfn mod C`, `C = 1` until time protection is wanted *(Ge et al., EuroSys '19,
via research/0002 Part 6)*. **Every frame is zeroed before a process can observe it** — a recycled frame
is an O-8 breach that needs no bug. *Rejected:* a **buddy allocator** *(Linux)*, whose coalescing fights
colouring, until measurement asks; a **free list**, which cannot find `n` contiguous frames without a scan.

## 5. Who pays: the object store and Pools (O-7)

The kernel has no heap and must never acquire one a process can drain; every kernel object and page-table
frame is memory some process caused it to spend. **A — seL4 Untyped and retype** is the proven strong form,
but reusing an Untyped revokes everything retyped from it through the derivation tree RFC-0003 §4 refused;
it would decide RFC-0003a by the back door and make object layouts ABI. **Rejected.** **B — a kernel heap
with creation policy** *(Zircon)* limits *whether* a process creates objects, not *how much* memory they
take; exhaustion is global and unnamed. **Rejected.** **C — static per-type tables** *(Hubris)* are bounded
but cannot say *who* filled one — research/0002 Part 6's AWS Kinesis outage. **Rejected alone.** **D — C's
structure with counted attribution: verdict sought**, this paper's one deliberate merge *(the requester pays,
as in seL4; a counted quota, as in Fiasco.OC's factory limits and Genode's quota trading)*.

**The object store is typed slot arrays.** Each object type has one fixed-capacity array in `.bss`; a slot
holds the object, a generation and a reference count, and **the slot index is the object's identity**.
`ObjectRef` (`capability/src/object.rs`) is `(type, index)` and `current_generation()` is a slot read — no
raw object pointer, slab or size class. A process's capability table lives in its slot
(`CapabilityTable<O, N>` is fixed-capacity). A destroyed object frees its frames at once; its slot is reused
only when the count, stale capabilities' references included, reaches zero, and is retired, as RFC-0003
requires, if its generation cannot advance.

**A Pool is the accounting layer**: an RFC-0003 object holding balances — frames, and slots of each type. At
boot every free frame and slot becomes the balance of one root Pool handed to the root task (RFC-0007
(proposed) provides the handover). Thereafter, for each resource `r` (frames, or one type's slots):

```text
Σ balance(r) over all Pools  ==  units of r free        -- at every instant after boot
every unit of r in use       ==  kept by the kernel at boot, or charged to exactly one Pool
```

- **Everything is charged**: Region frames; page-table frames, to the AddressSpace's Pool; one `Mapping`
  slot per mapping; kernel stacks; every object slot. Each creation presents a Pool with `WRITE`.
- **Charge first, then take.** Conservation guarantees the allocator can then supply; the only later
  failure, `TooFragmented` for a multi-frame extent, refunds before returning.
- **Failure is local and attributed.** `PoolExhausted` names the Pool and resource; no array can fill while
  a Pool holds its slots, so no unnamed limit exists after boot. Nothing overcommits *(research/0002 Part 6,
  "budgets are minima")*. Refund goes to the payer, whose index each object records — no tree.
- **Pools subdivide by moving balance.** `pool_split(pool)` makes an empty child with the parent's rights;
  `pool_move(from, into, r, n)` moves `n` units of one resource; `pool_merge(from, into)` succeeds only
  when `from` has no outstanding charges. Balance is conserved, never minted — O-2's shape for quantity.
- **Rights:** `READ` queries; `WRITE` spends and moves; `EXECUTE` lets minted Regions carry `EXECUTE` (§9).

This is not option B's heap: nothing grows after boot, and the only variable memory — frames — comes from
the bitmap in page units, charged. The kernel keeps its image, the map, the bitmap, the slot arrays, its
final tables and the boot stack.

## 6. The kernel's virtual layout

Both architectures get the **same layout**, the cheapest way to keep the HAL honest. The image sits in the
top 2 GiB because `x86_64-unknown-none` uses `code-model: kernel`, which requires it; AArch64 follows.

| Range (both architectures) | Contents | Attributes |
|----------------------------|----------|------------|
| `0x0000_0000_0001_0000` – `0x0000_7FFF_FFFF_EFFF` | the current AddressSpace (§7) | per mapping; not-global; never kernel-executable |
| `0xFFFF_8000_0000_0000` – `+64 TiB` | physmap: every discovered RAM frame at `base + pa` | Normal write-back; RW; execute-never; global; largest blocks alignment allows |
| `0xFFFF_C000_0000_0000` – `+1 TiB` | MMIO window: kernel-owned devices only (console, interrupt controller) | Device-nGnRE / uncached; RW; execute-never |
| `0xFFFF_FFFF_0000_0000` – `0xFFFF_FFFF_7FFF_FFFF` | kernel stacks, an unmapped guard page below each | Normal; RW; execute-never; frames charged like any object |
| `0xFFFF_FFFF_8000_0000` – top | image at `+0x20_0000` | `.text` RX, `.rodata` R, `.data`/`.bss` RW execute-never |

- **Kernel stacks have their own window** so each has a guard page, which the hole-free physmap cannot give.
  RFC-0006 (proposed) decides how many — per core or per thread — and allocates here; the boot stack too.
- **The user floor is 64 KiB**, so a kernel NULL dereference never lands on user data *(Linux
  `mmap_min_addr`)* — a fixed mechanism constant where Linux has a sysctl (§17).
- **The user ceiling leaves the last page below the hole unmapped**, closing Intel's `SYSRET` hazard
  (CVE-2012-0217) for RFC-0007's x86_64 exit path by layout, as Linux does. The physmap covers RAM, never
  MMIO, so no frame is aliased with conflicting attributes.
- **The kernel half never changes after boot.** AArch64 shares `TTBR1_EL1` for free; x86_64 copies PML4
  entries 256–511 into each address space, safe only because they are fixed *(seL4 x86)*.

*Rejected:* an **identity-mapped kernel** (impossible under the kernel code model); **no physmap** (an
invalidation per IPC-buffer touch, for a side-channel gain threat model §7 puts out of scope).

## 7. The objects: Region, AddressSpace and Mapping

**Frame ownership is pKVM's state machine** (research/0002 Part 7): every frame has exactly one owner — the
kernel, or one `Region`. **A Region** is a fixed number of pages backed by **at most eight extents** of
contiguous frames, allocated eagerly, zeroed and charged; a request the bitmap cannot meet in eight fails
`TooFragmented`, and DMA-ready memory is one extent at an alignment. Rights are `READ`, `WRITE`, `EXECUTE`,
with RFC-0003's `DUPLICATE`, `TRANSFER`, `REVOKE`. Mappings and IPC-buffer bindings hold counted references,
so closing the last handle does not pull memory from under a live mapping *(Zircon: a mapping keeps its VMO
alive)*; the forceful paths are destruction and `reissue` (§10).

**An AddressSpace** is a user root table, an ASID or PCID (§12), the Pool paying for its tables, and its
mappings; `WRITE` permits `map`/`unmap`/`protect`, `READ` a query. Binding it to a process is RFC-0007's.
**A Mapping** is a charged kernel record, not a capability: `{as, region, first_page, vaddr, pages, perms}`,
linked by slot index into its AddressSpace's list and its Region's.

**Why a Region must remember its mappings.** RFC-0003's 2026-07-30 amendment makes an invariant
load-bearing: *a resolved capability is never observed or cached outside the kernel table* — why a
generation bump alone makes capabilities inert. **A page-table entry is exactly such a cache**, consulted by
hardware on every access and never re-checked. The MMU makes the exception unavoidable; the per-Region
mapping list is the compensation, and the reason a generation bump alone cannot revoke memory.

**Device Regions** are minted at boot over `Mmio` ranges the kernel does not keep — never executable, zeroed
or charged; built in Phase 2. No operation maps a physical address given as a number: the catch-all right in
another costume. *Rejected:* **Zircon VMOs and VMARs** — lazy commit, copy-on-write, pagers and nesting are
policy-rich and put faults inside kernel paths; **seL4's capability per frame** — 256 handles per MiB;
**userspace-built page tables** (Xen PV) — research/0002 Part 7's bug farm; O-5 is a rights check, not a walk.

## 8. Map, unmap, protect — and the rights they check

Every operation takes handles (RFC-0003 §9); RFC-0007 (proposed) encodes them.

| Operation | Needs | Refused when |
|-----------|-------|--------------|
| `region_create(pool, pages, extents, align)` | pool `WRITE`; pool `EXECUTE` to mint `EXECUTE` | `PoolExhausted`; `TooFragmented`; zero pages |
| `as_create(pool)` | pool `WRITE` | `PoolExhausted` (tags roll over, §12) |
| `map(as, region, first_page, vaddr, pages, perms)` | as `WRITE`; region rights ⊇ `perms` | `vaddr` unaligned, below the floor or outside the user half; overlap (never silent replacement); beyond the Region; `pages > 512`; `perms ∉ {R, RW, RX}`; W^X (§9); `PoolExhausted` |
| `unmap(as, vaddr)` | as `WRITE` | no mapping based at `vaddr` — the unit is the whole mapping |
| `protect(as, vaddr, perms)` | as `WRITE` | `perms` not a subset of the mapping's current `perms` |
| `reissue(region)` | region `REVOKE` (a proposed reading, §10) | stale handle |

User numbers are validated *as numbers*. **The kernel never chooses a virtual address** *(seL4's VSpace
discipline)*: a kernel that picks addresses has an allocation policy in it. **`map` is atomic**: it counts
the table frames the range lacks and charges them, with the Mapping slot, *before* writing anything, so a
failed charge maps nothing; then it links tables and writes leaves. One call is bounded at 512 leaves and
six new tables — an unaligned range spans two leaf tables, each possibly with new parents at two levels.
Nothing splits a mapping. `protect` only narrows; widening is `map` again with the Region capability *(O-2's
shape; seL4's remap-with-capability)*. 4 KiB pages only; blocks later, invisible in the ABI.

**Verdict sought: `EXECUTE` joins RFC-0003's rights as bit 5**, as RFC-0003 §5 allows "when a subsystem
needs one". Folding execution into `READ` makes every readable Region potential code; a Region *kind* fails
the loader, which writes code before running it — a right in disguise. seL4 leaves execution an ungoverned
attribute; Fuchsia's `ZX_RIGHT_EXECUTE` *(Zircon)* is the lineage. The bit has two meanings, deliberately —
*may map executable* on a Region, *may mint executable Regions* on a Pool — one narrow authority over code,
refused on other objects. **`Rights::ALL` does not extend automatically**: `rights.rs` writes it as an
explicit union of five constants, and its doc comment claiming otherwise is misleading. The same PR edits
`ALL` and the comment, extends `Debug` and `rights_from_low_bits`, and widens the exhaustive test to `0..64`.

The handshake RFC-0003 §14.3 asked for is one rule: **the installed permission is the requested `perms`,
refused unless a subset of the Region capability's rights at map time.**

| perms | needs on the Region | AArch64 stage 1 | x86_64 |
|-------|---------------------|-----------------|--------|
| R | `READ` | `AP[2:1] = 0b11`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 1` |
| RW | `READ`, `WRITE` | `AP[2:1] = 0b01`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 1`, `XD = 1` |
| RX | `READ`, `EXECUTE` | `AP[2:1] = 0b11`, `UXN = 0`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 0` |

Every user entry also sets `AF = 1`, `nG = 1`, inner-shareable, `AttrIndx` → Normal write-back. Write-only
is refused; execute-only exists on AArch64 but not on x86_64 without protection keys, so neither offers it.

## 9. W^X (O-13), in four layers

O-13's intent — a data-only bug cannot become code execution — needs more than its letter:

1. **Unrepresentable.** `MapPerms` is an enum of `Read`, `ReadWrite`, `ReadExecute`; both encoders are total
   functions from it, so no writable-executable entry can be built. Host-tested exhaustively.
2. **Per Region, across every address space.** A Region counts its writable uses (an IPC-buffer binding is
   one) and its executable mappings; `map` refuses `RX` while any writable use exists anywhere, and `RW`
   while any `RX` mapping does. Per-page W^X alone leaves frames writable here and executable there.
3. **Over time.** `EXECUTE` is minted only from a Pool carrying `EXECUTE`: the broker hands apps Pools
   without it, the loader holds one, a JIT gets one by explicit grant *(Fuchsia moved executable memory from
   ambient job policy to the `VmexResource` capability)*. O-13's Phase-3 load-path clause is policy on this.
4. **Hardware backstop.** AArch64 sets `SCTLR_EL1.WXN` (bit 19) once the final tables are in (§13); the
   architecture already makes every EL0-writable page privileged-execute-never, and user entries set `PXN`
   besides. x86_64 has no `WXN`; it sets `CR0.WP` and `SMEP` where present, and layers 1–3 are the defence.

`map` with `RX` makes AArch64's instruction side coherent: `DC CVAU` through the physmap, `DSB ISH`, `IC IVAU`
on a PIPT instruction cache and `IC IALLUIS` otherwise (on VIPT-aliasing caches the physmap alias may miss
the user one — Linux's `sync_icache_aliases`), `DSB ISH`, `ISB`. QEMU's cortex-a72 reports PIPT (`CTR_EL0 =
0x8444c004`); x86_64 is coherent in hardware. **QEMU models no cache, so errors here pass every test.** The
physmap aliases user code as writable at EL1; writing through it only to zero frames, fill tables and reach
IPC buffers is a convention (§17).

## 10. Sharing and donation (RFC-0004 §9.1)

pKVM's *share* (the owner keeps access) and *donate* (the donor loses it) — research/0002 Part 7 — need one op:

- **Share** is `derive` then transfer over IPC: the borrower receives, say, `READ | WRITE` without
  `EXECUTE`, `DUPLICATE` or `REVOKE`, and maps it where it chooses; bulk data then travels as RFC-0004 §5's
  descriptor. No `share` syscall, and no page-table work during IPC — the long-IPC grave stays shut.
- **Donate** is transferring a capability carrying `REVOKE`, made exclusive by **`reissue(region)`**: tear
  down every mapping and IPC-buffer binding of the Region in every address space (§7 says why the walk is
  unavoidable), complete the invalidation (§12), advance the generation so every other capability is stale,
  and return a fresh capability with the caller's rights over the same frames. **Destruction** is the same
  walk without the re-mint; frames return to the payer only after the invalidation completes.

`reissue` is **RFC-0003 §7's destruction case** — generation bump and re-mint, B2's shape on one object — and
pre-empts nothing in RFC-0003a's B1/B2/B3 choice. Gating it on `REVOKE` is a **proposed reading**: RFC-0003
§5 defines `REVOKE` over capabilities derived from the holder's, and `reissue` also reaches holders that were
not. Copies the donor derived first die at the bump; a former sharer takes an ordinary fault (RFC-0006
(proposed) provides delivery) — the "observable and survivable" revocation research/0002 Part 6 asks for.

## 11. How the kernel touches user memory (O-5)

**The kernel never dereferences a user virtual address** — not through a checked copy or a fixup table, not
at all *(seL4: the IPC buffer is a frame reached through the kernel window)*. Everything the kernel reads or
writes for a process is named by capability and reached through the physmap:

- **The IPC buffer** — RFC-0004's spill area and slow-path copy — is one page of a Region bound to a thread
  by capability (RFC-0006 (proposed) records it on the Thread; RFC-0007 (proposed) provides the syscall).
  Binding needs `READ | WRITE`, holds a counted reference, is a writable use for §9, and `reissue` severs it.
- **No kernel path faults on user memory**: frames are eager and pinned, so the slow path copies frame to
  frame, and the in-kernel fault concurrency that sank L4's long IPC cannot arise.
- **Peer-writable memory is never referenced.** No Rust reference is formed into it: one `kernel/src/mm/`
  module copies with volatile accesses, and anything the kernel must *interpret* is copied once and validated
  as that snapshot *(rust-vmm; virtio's double-fetch lesson, research/0002 Part 5)*.

**The constraint RFC-0007 inherits: no syscall takes a pointer argument, ever.** Data travels in registers, the
IPC buffer, or as a Region handle plus offset; RFC-0003 §9 forbade naming *resources* by address, and this
extends it to data. *Rejected:* `copy_from_user` with fixups (Linux), making kernel faults a normal path.
`SMAP` and `PSTATE.PAN` back the rule where present; **cortex-a72 has no PAN**, so there it stands on structure.

## 12. TLBs, ASIDs and PCIDs

- **Tags are kernel-private.** An ASID (16 bits when `ID_AA64MMFR0_EL1.ASIDBits = 0b0010`, else 8;
  `TCR_EL1.A1 = 0`, so it rides in `TTBR0_EL1`) or a PCID (12 bits, where CPUID reports it). Tag 0 is
  reserved. Exhaustion bumps a generation, invalidates all and reassigns lazily *(Linux
  `arch/arm64/mm/context.c`)*. *Rejected:* seL4's ASID pools as capabilities — a cache tag is not authority.
- **Kernel entries are global, user entries never**; without PCID (QEMU's default x86_64 CPU) a `CR3` load
  flushes the non-global ones. **`map` needs no invalidation**: neither architecture caches a faulting
  entry (the Arm ARM; SDM Vol. 3A §4.10), though x86_64's paging-structure caches can cause one spurious
  fault the handler re-checks. `unmap`, `protect` and teardown store, then `DSB ISHST`, `TLBI VALE1IS` (ASID
  and address, leaf only), `DSB ISH`, `ISB`; x86_64 uses `INVLPG`, and `INVPCID` or a stale-on-switch mark.
  **Freeing a table frame needs more**: `VALE1IS` leaves walk caches alone, so any path that frees an
  intermediate table (§18) uses `TLBI VAE1IS` or `ASIDE1IS`; x86_64's `INVLPG` and `INVPCID` already flush
  the PCID's paging-structure caches (SDM Vol. 3A §4.10.4.1). Live remaps use break-before-make.
- **O-8's ordering: invalidate, then free.** No frame returns to a Pool while any core can translate to it.
- **Inner-shareable from day one.** The demo is `-smp 1`, but every `TLBI` is the broadcast form. x86_64 has
  no broadcast; its shootdown IPIs are RFC-0006 (proposed)'s multi-core shape, walking §7's mapping lists.

## 13. Early boot: turning the MMU on

The image is linked at `0xFFFF_FFFF_8020_0000` and loaded at 0x4020_0000 (`AT()` in `aarch64.ld`); QEMU loads
segments at `p_paddr` and translates the entry point to its physical alias (`include/hw/elf_ops.h.inc`;
checked on 8.2.2), and the stub is already position-independent. With the MMU off, the stub:

1. Parks secondaries, installs the stack and zeroes `.bss` at physical addresses, as today.
2. `MAIR_EL1`: index 0 = `0xFF` (Normal write-back, read/write-allocate), index 1 = `0x04` (Device-nGnRE).
3. `TCR_EL1`: `T0SZ = T1SZ = 17`; `TG0 = 0b00` and `TG1 = 0b10`, both 4 KiB (the trap); `SH = 0b11`;
   `IRGN/ORGN = 0b01`; `IPS` from `PARange` (44 bits here); `AS = 1`; `A1 = 0`; `TBI = 0`; `HA = HD = 0`.
4. Four page-aligned boot tables in `.bss`: `TTBR0_EL1` identity-maps 0x4000_0000 as a 1 GiB Normal block and
   0x0000_0000 as a 1 GiB Device block, so the UART survives; `TTBR1_EL1` maps `0xFFFF_FFFF_8000_0000` to
   0x4000_0000 as a 1 GiB Normal block. Five descriptors, all `AF = 1`.
5. `DC IVAC` over the tables, written non-cacheable, so no stale line shadows them once walks are cacheable
   (Linux `head.S` does the same); `TLBI VMALLE1`; `DSB ISH`; both `TTBR`s; `ISB`; `SCTLR_EL1` written whole
   with `M`, `C`, `I` (bits 0, 2, 12) and `nTWI = nTWE = 0`, as RFC-0006 (proposed) asks; `ISB`.
6. Branches to the high alias by absolute address, moves `SP` and `VBAR_EL1` there, and calls `rust_entry`.
   **Rust never executes at the low alias.**

Rust then, in an order that never leaves the exception reporter without a console: (1) reads the DTB through
the high block and builds the `MemoryMap`, bitmap and slot arrays; (2) builds §6's final tables; (3) swaps
`TTBR1_EL1` by an assembly routine run from the identity map — empty table, `TLBI VMALLE1`, final table —
since turning blocks into pages under a live translation needs break-before-make *(Linux
`idmap_cpu_replace_ttbr1`)*; (4) **repoints the console at the UART's MMIO-window address** — the physical
`UART0_BASE` in `uart.rs` dies silently once the identity map goes; (5) only then sets `TCR_EL1.EPD0 = 1`,
so nothing walks `TTBR0_EL1` until the first AddressSpace loads, sets `WXN`, and issues `TLBI VMALLE1`,
`DSB ISH`, `ISB` (`WXN` may be cached in the TLB); the boot tables' frames are freed.

**From MMU-on to step 5 the kernel is one RWX gigabyte** — before any process exists, but a stray write in
early Rust goes uncaught. The reporter gains the decode this needs: aborts print `FAR_EL1` and the fault
level from `ESR_EL1.ISS.DFSC`; on x86_64, `#PF` prints `CR2` and its error code. **x86_64**, once the UEFI
stub exists, is already paging; Rust builds §6's PML4, sets `IA32_EFER.NXE` (bit 11 — without it `XD` is a
reserved-bit fault), `CR0.WP` (bit 16, so ring 0 honours read-only pages), `CR4.PGE` (bit 7) and
`CR4.PCIDE`/`SMEP`/`SMAP` (bits 17, 20, 21) where CPUID permits, and loads `CR3`. *Rejected:* a separate
loader *(seL4's elfloader)*; Rust building tables with the MMU off, sound only without absolute addresses.

## 14. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4** | frames as capability objects; the kernel window; never touching user addresses; all remaining memory to the first task; user-chosen layout | Untyped/retype and the CDT; execute as an ungoverned attribute; ASID pools as capabilities |
| **pKVM** *(research/0002 Part 7)* | one owner per frame; share and donate as the only transitions | stage-2 mechanics; a separate page-state table |
| **Zircon / Fuchsia** | a region mapped into many spaces, kept alive by mappings; `EXECUTE` as a right; `VmexResource` | the kernel heap; lazily committed VMOs; VMARs |
| **Hubris** | build-time-sized kernel tables | a fully static task set |
| **Fiasco.OC, Genode** | kernel memory as a counted, tradeable quota | quotas on a general heap |
| **Linux** | the linear map; ASID rollover; arm64 descriptor headers as a checked reference; software PAN as an option | the buddy allocator; `copy_from_user` and its fixups |
| **Ge et al., EuroSys '19** | the colour-capable allocator | time protection itself, for later |

## 15. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-3 revocability | destruction and `reissue` walk the Region's mapping list, invalidate, then bump; a page-table entry is a cached resolution RFC-0003's invariant cannot cover, so the generation bump alone is not enough for memory (§7, §10) | **destruction case extended to memory**; selective revocation stays RFC-0003a's |
| O-5 argument validation | no user virtual address ever dereferenced; memory reached by capability through the physmap; snapshot-validate peer-writable data (§8, §11) | **discharged for the memory surface**; RFC-0007 inherits "no pointer arguments" |
| O-6 kernel memory safety | new `unsafe` confined to `mm/**` (physmap, volatile copy) and `arch/**` (tables, TLB, boot); engine, allocator and store are safe host-tested code | **stays Built** — every new block listed per PR |
| O-7 no unprivileged exhaustion | typed slot arrays; every post-boot frame and slot charged to a named Pool; charge-first atomic `map`; bounded work per call (§5, §8) | **discharged for memory and object slots**; CPU time is RFC-0006's half |
| O-8 address-space disjointness | per-process tables; Regions only by capability; zero before exposure; invalidate before free; privileged-only kernel half (§4, §7, §12) | **discharged** |
| O-9 no ambient side channel | no shared mapping a capability did not establish (§10) | **contributes** |
| O-13 W^X | four layers, including per Region across all address spaces (§9) | **discharged at mapping**; the load-path clause is Phase-3 policy over layer 3 |
| O-2, O-4 | `perms` ⊆ capability rights; `protect` narrows; no `brk`, anonymous `mmap` or physical-address map | **maintained** |
| O-18 confined DMA | single-extent Regions; Device Regions minted at boot | **groundwork only** — Phase 2 |

## 16. Graves checked (§3)

- **Policy in the kernel.** No address choice, demand paging, copy-on-write, overcommit or OOM response.
- **Baroque capability hierarchies.** No Untyped tree; Pools are flat and balance moves rather than derives.
- **Multi-copy IPC.** The slow path copies once, frame to frame; bulk data is shared, never copied (§10).
- **Bolted-on multicore.** Broadcast invalidation from the first line; the tag allocator is the SMP design.
- **The catch-all right.** `EXECUTE` is narrow; there is deliberately no "map any physical address" right.
- **Compiled-in unused device paths (VENOM).** The kernel maps only the devices it drives itself.

## 17. Costs — what this makes harder

- **Capacities are ceilings**: raising a slot count is a rebuild, and `pool_move` is one call per resource.
- **Regions are eager and bounded**: no lazy or sparse Regions; a large one can fail `TooFragmented`.
- **47-bit halves on AArch64** give up half of what `T0SZ = 16` allows, for one layout and one user ABI;
  **the 64 KiB floor is a fixed constant**, where Linux lets the administrator tune it.
- **No pointer arguments** (§11): an ergonomic cost on every syscall RFC-0007 designs, for a bug class gone.
- **The physmap is a large target**: one kernel write bug reaches all RAM, user code included (§9) — a
  software invariant, as are W^X on x86_64 and O-5 without PAN on cortex-a72.
- **Cache maintenance is untestable under QEMU** (§9, §13); and **the load address is a QEMU `virt` fact**.
- **Reclaim needs cooperation or revocation**: stale capabilities pin slots and transferred ones keep a Pool
  charged, so `pool_merge` waits — RFC-0003a's problem made concrete.

## 18. Open questions

1. **Page-table frames on unmap.** Freed only with the AddressSpace; eager freeing needs §12's walk-cache rule.
2. **One accounting domain?** research/0002 Part 6 wants memory, object quotas and scheduling contexts under
   one capability-named domain; the Pool can take time as another resource. RFC-0006 decides.
3. **Which right authorises `reissue` and destruction.** Proposed `REVOKE` (§10); RFC-0003a and RFC-0007 settle it.
4. **Hardware backing for two software rules:** software PAN on Armv8.0 (a `TTBR0_EL1` change per kernel
   entry), and unmapping executable user frames from the physmap.
5. **Preemptible teardown.** The O(mappings) walk must be restartable to use RFC-0006 (proposed)'s preemption points.
6. **The firmware assumption** (§4) for threat model §10, via `docs/CHANGELOG.md`.

## 19. Implementation increments

One reviewable PR each; **[demo]** marks what the demo needs. Pure logic lands in a new `memory/` crate —
`no_std` in the kernel, `std` under `cargo test`, built for both targets in CI, no `unsafe`, as `capability/`.

1. **[demo] Relink at 0x4020_0000; prove the DTB.** MMU still off. Boot-test `--expect "DTB 0xd00dfeed at 0x40000000"`.
2. **[demo] Addresses, `MemoryMap`, FDT reader.** Host tests on a `dumpdtb` fixture and hostile blobs;
   `CLAUDE.md` § Layout gains `memory/`. Boot-test `mm: 512 MiB usable at 0x40000000, N frames, M reserved`.
3. **[demo] Frame bitmap.** Host churn tests against a shadow model: no frame twice, none reserved.
4. **[demo] `EXECUTE` in `capability/`.** Bit 5, `ALL` edited by hand and its comment corrected, `Debug`,
   the test widened to `0..64`. Host-only; demo-needed because the root maps the client's text.
5. **[demo] `MapPerms` and both encoders.** Exhaustive host tests: nothing writable and executable; every
   user entry `PXN`, `AF`, not-global; every row of §8's table.
6. **[demo] The table engine** over a frame-access trait. Host tests on both formats: overlap, kernel-half
   refusal, the 512-leaf and six-table bound, nothing mapped when a table cannot be paid for.
7. **[demo] Object store and Pools (§5).** Host tests: conservation under random create, destroy, split,
   move and merge; `PoolExhausted` names the payer.
8. **[demo] MMU on (§13, stub).** Higher-half relink, boot tables, `VBAR_EL1` high, `FAR_EL1`/DFSC decoding.
   Boot-test: `Kaya!` from the high alias, and the exception self-test still passes.
9. **[demo] Final kernel map (§13, Rust)**, the console repointed in the same PR; `EPD0`, `WXN`. Boot-test
   `--features provoke-wx` writes to `.text`, expecting `data abort, same EL` and `FAR = 0xffffffff8…`.
10. **[demo] Region, AddressSpace, Mapping** (§7–§9, §12) charged to the root Pool, with per-Region W^X
    counts, the tag allocator (rollover host-tested) and `TTBR0_EL1` switching. Boot-test: `AT S1E0R` under
    two ASIDs resolves one shared Region to the same frame and private ones apart: `mm: shared region agrees`.
11. **User-memory module (§11).** Boot-test with a message beyond the register budget. Not demo.
12. **Destruction and `reissue`.** Boot-test that a reissued sharer faults. Not demo.
13. **PAN, SMAP, SMEP** where reported, with a `provoke-user-deref` self-test under `-cpu max`. Not demo.
14. **x86_64** tables, `CR3` and PCID, after the UEFI stub. Not demo.
15. **Device Regions** — Phase 2, with the first userspace driver.

**Interims the demo carries, with their cost:**

- **One root Pool.** `pool_split`/`move`/`merge` are host-tested, not yet syscalls: server and client
  charges are indistinguishable.
- **No destruction or `reissue`**: a killed process's frames and slots stay charged; O-3's memory case is
  designed, not built. **No IPC-buffer module**: messages beyond the register budget cannot be sent.
- **Hard-coded UART and GIC physical addresses**, against QEMU's warning that they may vary.
- **One RWX gigabyte** between MMU-on and increment 9, before any process exists; cache maintenance unverified.
- **Not an interim:** the kernel maps the root's image at boot — RFC-0007 (proposed)'s one bootstrap act —
  through increment 10's `map` with kernel-minted Region capabilities, so §8's rights check runs even there.

## 20. What this unblocks

Increments 1–10 give RFC-0006 (proposed) address spaces, guarded kernel stacks and a Pool to charge threads
to, and RFC-0007 (proposed) the objects its Process, root task and boot info are made of, plus a constraint
it inherits rather than discovers: **no syscall takes a pointer argument**. The root task receives all memory
as one boot-minted Pool — RFC-0003 §14.4's bootstrap without an ambient grantor; increment 12 closes RFC-0004
§9.1 in code. Increments 1–3 change nothing about how the kernel runs and can land immediately.
