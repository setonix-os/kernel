<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0005 — Address spaces and physical memory

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 (kernel doctrine) and its "minimal assembly" clause (§18.7); RFC-0003 (a dated amendment adding one right; three new objects; `capability/src/rights.rs` and `object.rs`'s doc comment); RFC-0004 §9.1; the boot stub, `aarch64.ld` and the exception reporter; `kernel/src/mm/**`; a new host-tested `memory/` crate; threat model §10 and O-7's text (proposals via `docs/CHANGELOG.md`) |
| Depends on | RFC-0003 (capability table), RFC-0004 (IPC); interlocks with RFC-0006 (threads and scheduling) and RFC-0007 (system calls and process bootstrap), proposed the same day |
| Discharges | O-7 for kernel allocation (reclaim across a transfer stays open), O-8, O-13 (the mapping half); O-5 for every path by which the kernel touches user memory; O-3's destruction case extended to memory; answers RFC-0003 §14.3 and RFC-0004 §9.1 (share) |

> **Proposed verdicts.** **[demo]** marks what tonight's demo stands on; the rest is forward design.
>
> 1. **[demo] Discovery:** firmware tables into one bounded `MemoryMap`; the image moves to 0x4020_0000, because QEMU places no DTB for an ELF loaded at the base of RAM (§4).
> 2. **[demo] Frames:** a colour-capable bitmap over 4 KiB frames; fresh frames are zeroed, boot data is handed over read-only, so no frame reaches a process holding another's data (§4, §7).
> 3. **[demo] Who pays (O-7):** typed slot arrays reused at once by generation; every post-boot frame and slot charged to a `Pool` presented by capability — `map` included; the Pool is the one accounting domain for counted units (§5).
> 4. **[demo] Layout:** a higher-half kernel with the same layout on both architectures; 47-bit halves; a non-executable physmap in which the image is read-only; per-core stacks in a guarded window, kept by the kernel (§6).
> 5. **[demo] Objects:** `Region` (eager and zeroed, or boot-minted read-only over boot data) and `AddressSpace` join RFC-0003's objects beside `Pool`; each mapping is a charged record (§7).
> 6. **[demo] Operations:** `map` / `unmap` / `protect` are mechanical, capability-checked, bounded and atomic; `protect` only narrows; the kernel never picks an address (§8).
> 7. **[demo] Rights:** `EXECUTE` joins RFC-0003's set as bit 5 — map-executable on a Region, mint-executable on a Pool, refused at mint on every other type — by dated amendment (§8).
> 8. **[demo] W^X (O-13):** unrepresentable in the mapping type, enforced per Region across all address spaces, backed by `SCTLR_EL1.WXN` (§9).
> 9. **Sharing (RFC-0004 §9.1):** *share* is derive-and-transfer; destruction walks the Region's mappings; *donation* is not proposed here but handed to RFC-0003a as input (§10).
> 10. **[demo] User memory (O-5):** the kernel never dereferences a user virtual address; where a syscall carries one, it is a number, validated as a number (§11).
> 11. **TLB:** tags are kernel-private and recycled; this RFC owns the shootdown and rollover protocol for SMP, RFC-0006 provides the IPI; single-core half **[demo]** (§12).
> 12. **[demo] Early boot:** the stub enables the MMU with a stated `SCTLR_EL1`; Rust runs only at the high alias and installs the final W^X map (§13).

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
and 47-bit user and kernel halves whose top level uses 256 entries each.** AArch64 gets this from the 4 KiB
granule with `TCR_EL1.T0SZ = T1SZ = 17`; x86_64 from IA-32e four-level paging without LA57.

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
encoder and a frame-access trait, host-tested on both against a fake frame store in `memory/`; only turning
a frame number into `&mut [u64; 512]` through a kernel window (§6, §13) is `unsafe`, in `kernel/src/mm/`.

## 4. Physical memory: discovery and the frame allocator

**Discovery.** QEMU documents two fixed facts about `virt` — flash at 0, RAM at 0x4000_0000 — and says every
other address "may vary across QEMU versions" and must be read from the DTB, which a bare-metal ELF finds
"at the start of RAM" with no register pointing at it (`docs/system/arm/virt.rst`). `hw/arm/boot.c` puts it
there only below the image (`dtb_limit` is the image's lowest address), and today's image *is* the base of
RAM. Checked on QEMU 8.2.2 on 2026-09-27: an ELF at 0x4000_0000 boots with **no DTB in RAM** and `x0 = 0`; at
0x4020_0000 one is found (magic `0xd00dfeed`) at 0x4000_0000. **So the image loads at 0x4020_0000**, 2 MiB in
and block-aligned — which caps the DTB at 2 MiB; increment 1 re-checks the pinned QEMU 11.0.3. A minimal FDT
reader, written for the purpose (Borrow Ledger boot-path row; Devicetree Specification v0.4), reads the cell
sizes, `/memory`, `/memreserve/` and `/reserved-memory`, bounds-checks every offset and fails closed. Threat
model §10 names no firmware assumption; this RFC proposes one — *firmware tables describe memory
truthfully* — through `docs/CHANGELOG.md`. On x86_64 the UEFI stub (owned by no RFC yet) hands over
`GetMemoryMap`'s output after `ExitBootServices`, when boot-services ranges become usable. **Both produce
one `MemoryMap`**: adjacent same-kind ranges coalesced, then at most **256** sorted, disjoint `(PhysRange,
Kind)` entries (6 KiB of `.bss`; real UEFI maps run to 100–200 raw descriptors) — `Usable`, `KernelImage`,
`DeviceTree`, `Reserved`, `Mmio` — failing closed beyond that, with image, boot stack, DTB and boot tables
tagged before the allocator sees it. In Phase 1 the `Mmio` entries are the hard-coded interim addresses
(§19). Nothing above `arch` names FDT or UEFI.

**The frame allocator is a bitmap**: one bit per 4 KiB frame, its storage taken from the first free frames
(16 KiB for 512 MiB). It allocates from a *colour set*, or runs of `n` at an alignment; a double free fails
closed. Colour is `pfn mod C`, `C = 1` until time protection is wanted *(Ge et al., EuroSys '19, via
research/0002 Part 6)*. A run search is O(frames / 64) words — bounded by RAM, not by what the caller may
spend; a per-alignment cursor is the named remedy if measurement asks. **Fresh frames are zeroed before a
process can observe them**, and **a page-table frame is zeroed before its parent descriptor is written**
(a `DSB` between) — a recycled table with stale valid entries would translate into another process's
frames, an O-8 breach no process ever "observes". *Rejected:* a **buddy allocator** *(Linux)*, whose
coalescing fights colouring; a **free list**, which cannot find `n` contiguous frames without a scan.

## 5. Who pays: the object store and Pools (O-7)

The kernel has no heap and must never acquire one a process can drain. **A — seL4 Untyped and retype** is
the proven strong form, but reusing an Untyped revokes everything retyped from it through a derivation
tree, which would decide RFC-0003a's B1 by the back door (RFC-0003 §7 and its 2026-07-30 amendment) and
make object layouts ABI. **Rejected.** **B — a kernel heap with creation policy** *(Zircon)* limits
*whether* a process creates objects, not *how much* memory they take; exhaustion is global and unnamed.
**Rejected.** **C — static per-type tables** *(Hubris)* are bounded but cannot say *who* filled one —
research/0002 Part 6's Kinesis outage. **Rejected alone.** **D — C's structure with counted attribution:
verdict sought** *(the requester pays, as in seL4; a counted quota, as in Fiasco.OC's factory limits and
Genode's quota trading)*.

**The object store is typed slot arrays.** Each object type has one fixed-capacity array in `.bss`; a slot
holds the object, a generation and a count, and the slot index is the object's identity. The kernel's
implementation of `ObjectRef` (`capability/src/object.rs`, a trait) is `(type, index, generation)`, and
`current_generation()` is a slot read — no raw pointer, slab or size class. **Only live references count**:
capabilities minted at the slot's current generation, Mappings and IPC-buffer bindings; a reference whose
generation is not the slot's counts nothing, on clone or drop. So a destroyed object's slot is **reused at
once with its generation advanced**, stale capabilities fail closed on it — exactly what RFC-0003 §8's
generation exists to allow — and no past holder can pin another process's charge. A slot whose generation
cannot advance is retired, its unit debited once from its payer and thereafter kept by the kernel (objects
keep 64-bit generations, RFC-0007 (proposed) §6, so this is a boundary, not a budget). `object.rs`'s
"takes a reference count" comment gains that qualification in increment 7.

**Where it lives.** The generic slot array and Pool are safe code in `memory/`, generic over the object type.
The concrete store — one `Kernel` value holding every type's array, RFC-0004/0006/0007 objects included — is
instantiated in the kernel crate and reached through **one** `static` wrapper in `kernel/src/mm/` carrying one
SAFETY argument: one core, the kernel entered with interrupts masked (RFC-0006 (proposed) V1), the big lock
when SMP lands. No other kernel global accretes *(research/0002 Part 6)*.

**A Pool is the accounting layer**: an RFC-0003 object holding balances — frames, and slots of each type. At
boot every free frame and slot becomes the balance of one root Pool (RFC-0007 (proposed) hands it over).
Thereafter, for each resource `r`:

```text
Σ balance(r) over all Pools  ==  units of r free        -- at every instant after boot
every unit of r in use       ==  kept by the kernel (at boot, or retired), or charged to exactly one Pool
```

- **Charged:** Region frames; every object slot; page-table frames and the `Mapping` slot, to the Pool
  `map` names (§8). **Kept, never charged:** the image, bitmap, slot arrays, kernel tables, per-core
  stacks and boot-minted Regions (§7). Each charge presents a Pool with `WRITE`; nothing spends ambiently.
- **Charge first, then take.** Conservation guarantees the allocator can supply; the only later failure,
  `Fragmented` for a multi-frame extent, refunds before returning.
- **Failure is local and attributed.** `PoolExhausted` names the Pool and resource; nothing overcommits
  *(research/0002 Part 6, "budgets are minima")*. Refund goes to the payer, whose index each object records.
- **Pools subdivide by moving balance.** `pool_split(pool)` makes an empty child with the parent's rights,
  recording the parent as its funder; `pool_move(from, into, r, n)`; `pool_merge(from, into)` when `from`
  has no outstanding charges. Balance is conserved, never minted — O-2's shape for quantity.
- **A Pool whose last capability closes** returns its free balance to its funder at once; its slot stays as
  a forwarding record until its outstanding charges come home, each refund forwarded — a chain bounded by
  the Pool slot count. The root Pool has no funder: closing it hands its balance to the kernel for good.
- **Rights:** `READ` queries; `WRITE` spends and moves; `EXECUTE` lets minted Regions carry `EXECUTE` (§9).

**One accounting domain — decided here.** research/0002 Part 6 asks for one capability-named domain to
which memory, object quotas and scheduling contexts attach. **The Pool is that domain for every counted
unit**: frames and every type's slots, Threads' and SchedContexts' included. **Time is not a unit**:
RFC-0006 (proposed) sells it as per-core bandwidth through `Core`, which moving balance cannot conserve,
and folding it in would make every Pool holder a seller of CPU. That is a deliberate split from the
research; congruence survives by attribution, since every SchedContext is charged to a Pool. RFC-0006 is
told so; O-7's text should then read *"kernel memory is charged to a capability-named Pool; CPU time to a
SchedContext funded through a `Core`"*, proposed through `docs/CHANGELOG.md`.

## 6. The kernel's virtual layout

Both architectures get the **same layout**, the cheapest way to keep the HAL honest. The image sits in the
top 2 GiB, which `x86_64-unknown-none`'s default kernel code model expects; overriding it is possible
(RFC-0007 does so for userspace) but buys nothing. AArch64 follows.

| Range (both architectures) | Contents | Attributes |
|----------------------------|----------|------------|
| `0x0000_0000_0001_0000` – `0x0000_7FFF_FFFF_EFFF` | the current AddressSpace (§7) | per mapping; not-global; never kernel-executable |
| `0xFFFF_8000_0000_0000` – `+64 TiB` | physmap: every discovered RAM frame at `base + pa` | Normal write-back; RW, but the image's `.text`/`.rodata` read-only; execute-never; global |
| `0xFFFF_C000_0000_0000` – `+1 TiB` | MMIO window: kernel-owned devices only (console, interrupt controller) | Device-nGnRE / uncached; RW; execute-never |
| `0xFFFF_FFFF_0000_0000` – `0xFFFF_FFFF_7FFF_FFFF` | per-core kernel and emergency stacks, an unmapped guard page below each | Normal; RW; execute-never; kept by the kernel at boot, never charged |
| `0xFFFF_FFFF_8000_0000` – top | image at `+0x20_0000` | `.text` RX, `.rodata` R, `.data`/`.bss` RW execute-never |

- **Stacks.** RFC-0006 (proposed) V1 chose one kernel stack per core; they are allocated at boot, and core
  0's boot stack moves from `aarch64.ld`'s unguarded slot into this window in increment 10. A guard page
  makes an overflow a fault, but the fault's entry then pushes onto the same stack: **RFC-0006 (proposed)
  provides the overflow stack** (x86_64 IST; on AArch64 the EL1-origin vector switching to the emergency
  stack, since a kernel fault is fatal). Until then an overflow is a recursive abort — detected, not diagnosed.
- **The user floor is 64 KiB**, so a kernel NULL dereference never lands on user data *(Linux
  `mmap_min_addr`)*. **The user ceiling leaves the last page below the hole unmapped**, closing Intel's
  `SYSRET` hazard (CVE-2012-0217) for RFC-0007's exit path by layout. The physmap covers RAM, never MMIO.
- **The kernel's top-level entries never change after boot.** AArch64 shares `TTBR1_EL1`; x86_64 copies PML4
  entries 256–511 into each address space *(seL4 x86)*, so every kernel-half PML4 entry that will ever be
  used — physmap for discovered RAM, MMIO window, stack window, image — is populated at boot; a PML4E first
  filled later would be missing from every earlier copy (Linux's vmalloc-sync bug family). Lower-level kernel
  tables may change; such a change is global and invalidates on every core (§12). *Rejected:* an
  identity-mapped kernel — possible under an overridden code model, but it puts the kernel where user
  addresses go and breaks the symmetric halves; no physmap (an invalidation per IPC-buffer touch).

## 7. The objects: Region, AddressSpace and Mapping

**Frame ownership is pKVM's state machine** (research/0002 Part 7): every frame has exactly one owner — the
kernel, or one `Region`. **A Region** is a fixed number of pages backed by at most eight extents of
contiguous frames. Rights are `READ`, `WRITE`, `EXECUTE`, with RFC-0003's `DUPLICATE`, `TRANSFER`, `REVOKE`.
Mappings and IPC-buffer bindings hold counted references, so closing the last capability does not pull
memory from under a live mapping *(Zircon: a mapping keeps its VMO alive)*. Three kinds, one object:

- **Ordinary** — `region_create`: allocated eagerly, zeroed, charged; a request the bitmap cannot meet in
  eight extents fails `Fragmented`; DMA-ready memory is one extent at an alignment. Zeroing is the long
  part: it runs 512 pages per step between RFC-0006 (proposed)'s preemption points, the half-built Region
  held on the calling thread, and no capability exists until it is done.
- **Boot-minted, read-only** — once, at boot, over boot data: the DTB and each boot module RFC-0007
  (proposed) embeds. Their `DeviceTree`/`KernelImage` frames **move** into the Region's ownership; they are
  never zeroed (public boot data, so O-8 is untouched), never charged, and minted without `WRITE`: the DTB
  `READ`, a module `READ | EXECUTE | DUPLICATE | TRANSFER`, so text maps `RX` in place and W^X holds because
  no writable use can ever exist. RFC-0007's bundle must page-align every module segment.
- **Device** — minted at boot over `Mmio` ranges the kernel does not keep; never executable, zeroed or
  charged; Phase 2. No operation maps a physical address given as a number: the catch-all right in costume.

Regions the kernel fills for the root before handover (its writable segments, stack, BootInfo) are ordinary:
created and charged to the root Pool, bytes copied in through the physmap, and only then a capability minted.

**An AddressSpace** is a user root table, a tag (§12), the Pool paying for its tables — the only Pool `map`
may charge — and its mappings; `WRITE` permits `map`/`unmap`/`protect`, `READ` a query. Binding it to a
process is RFC-0007's. **A Mapping** is a charged kernel record, not a capability: `{as, region, first_page,
vaddr, pages, perms}`, linked by slot index into its AddressSpace's list and its Region's.

**Why a Region must remember its mappings.** RFC-0003's 2026-07-30 amendment makes an invariant
load-bearing: *a resolved capability is never observed or cached outside the kernel table*. **A page-table
entry is exactly such a cache**, consulted by hardware and never re-checked. The per-Region mapping list is
the compensation, and the reason a generation bump alone cannot revoke memory. *Rejected:* **Zircon VMOs
and VMARs** — lazy commit, copy-on-write, pagers and nesting are policy-rich and put faults in kernel paths;
**seL4's capability per frame** — 256 handles per MiB; **userspace-built page tables** (Xen PV).

## 8. Map, unmap, protect — and the rights they check

Every operation takes handles (RFC-0003 §9); RFC-0007 (proposed) encodes them as methods. A second handle —
`map`'s Region and Pool — travels as a **handle word in a message register**, resolved in the caller's
table, never transferred: RFC-0003 §6's move would otherwise take the Region from its mapper.

| Operation | Needs | Refused when |
|-----------|-------|--------------|
| `region_create(pool, pages, max_extents, align, rights)` | pool `WRITE`; pool `EXECUTE` if `rights` has `EXECUTE` | `PoolExhausted`; `Fragmented`; zero pages; `align` not a power-of-two page count; `rights` ⊄ Region rights |
| `as_create(pool, rights)` | pool `WRITE` | `PoolExhausted` |
| `map(as, region, pool, first_page, vaddr, pages, perms)` | as `WRITE`; region rights ⊇ `perms`; `pool` is the AddressSpace's own, with `WRITE` | `vaddr` unaligned, below the floor or outside the user half; overlap; beyond the Region; `pages > 512`; `perms ∉ {R, RW, RX}`; W^X (§9); `PoolExhausted` |
| `unmap(as, vaddr)` | as `WRITE` | no mapping based at `vaddr` — the unit is the whole mapping |
| `protect(as, vaddr, perms)` | as `WRITE` | `perms` not a subset of the mapping's current `perms` |

User numbers are validated *as numbers*. **The kernel never chooses a virtual address** *(seL4's VSpace
discipline)*. **Why `map` names a Pool:** spending is authority (O-4), so a holder of an AddressSpace
capability alone cannot drain its Pool; requiring *the AddressSpace's own* Pool keeps one payer per space,
so refunds need no per-table record *(seL4 has the caller supply page-table objects)*. *Rejected:* any Pool
(a payer record per table frame); a Pool bound at `as_create` and spent by `WRITE` (spending never granted).
**`map` is atomic**: it counts the table frames the range lacks and charges them, with the Mapping slot,
*before* writing anything; then it zeroes and links tables, writes leaves and ends with `DSB ISHST`, so the
walker sees them before `eret`. One call is bounded at 512 leaves and six new tables. Nothing splits a
mapping. `protect` only narrows; widening is `map` again with the Region capability *(seL4's remap)*. 4 KiB
pages only; blocks later, invisible in the ABI.

**Verdict sought: `EXECUTE` joins RFC-0003's rights as bit 5**, as RFC-0003 §5 allows "when a subsystem
needs one". Folding execution into `READ` makes every readable Region potential code; a Region *kind* fails
the loader, which writes code before running it. seL4 leaves execution an ungoverned attribute; Fuchsia's
`ZX_RIGHT_EXECUTE` is the lineage. It means *may map executable* on a Region and *may mint executable
Regions* on a Pool, and is **refused at mint on every other type**: the store's mint path rejects it
(host-tested, increment 7), and `derive` cannot add it (O-2). `rights.rs`'s `ALL` is an explicit union
despite its comment; increment 4 edits both. **On acceptance, RFC-0003 gains a dated amendment adding
`EXECUTE` to §5's table, logged in `docs/CHANGELOG.md`.** RFC-0003 §14.3's handshake is one rule: **the
installed permission is the requested `perms`, refused unless a subset of the Region capability's rights.**

| perms | needs on the Region | AArch64 stage 1 | x86_64 |
|-------|---------------------|-----------------|--------|
| R | `READ` | `AP[2:1] = 0b11`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 1` |
| RW | `READ`, `WRITE` | `AP[2:1] = 0b01`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 1`, `XD = 1` |
| RX | `READ`, `EXECUTE` | `AP[2:1] = 0b11`, `UXN = 0`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 0` |

Every user entry also sets `AF = 1`, `nG = 1`, inner-shareable, `AttrIndx` → Normal write-back. Write-only
is refused; execute-only exists on AArch64 but not on x86_64 without protection keys, so neither offers it.

## 9. W^X (O-13), in four layers

1. **Unrepresentable.** `MapPerms` is an enum of `Read`, `ReadWrite`, `ReadExecute`; both encoders are total
   functions from it, so no writable-executable entry can be built. Host-tested exhaustively.
2. **Per Region, across every address space.** A Region counts its writable uses (`RW` mappings and
   IPC-buffer bindings) and its `RX` mappings. `map RX` is refused while any writable use exists; `map RW`
   and IPC-buffer binding are refused while any `RX` mapping exists. `unmap`, `protect` and unbinding
   decrement **only when their invalidation has completed on every core** (§12) — before that, a stale
   writable entry may still be live somewhere. Per-page W^X alone leaves frames writable here and
   executable there.
3. **Over time.** `EXECUTE` is minted only from a Pool carrying `EXECUTE`: the broker hands apps Pools
   without it, the loader holds one, a JIT gets one by explicit grant *(Fuchsia's `VmexResource`)*. O-13's
   Phase-3 load-path clause is policy on this.
4. **Hardware backstop.** AArch64 sets `SCTLR_EL1.WXN` (bit 19) once the final tables are in (§13); user
   entries set `PXN`, and the image's text has no writable alias, since the physmap maps it read-only
   *(Linux `mark_linear_text_alias_ro`)*. x86_64 has no `WXN`; it sets `CR0.WP` and `SMEP` where present.

`map RX` makes AArch64's instruction side coherent: `DC CVAU` through the physmap, `DSB ISH`, `IC IVAU` on a
PIPT instruction cache and `IC IALLUIS` otherwise (Linux's `sync_icache_aliases`), `DSB ISH`, `ISB`. QEMU's
cortex-a72 reports PIPT (`CTR_EL0 = 0x8444c004`); x86_64 is coherent in hardware. **QEMU models no cache,
so errors here pass every test.** These, and §12's `TLBI`s and `TTBR` writes, are single-instruction `asm!`
intrinsics inside `kernel/src/arch/**` with no Rust equivalent, as `wfe` is today — a reading of the
constitution's "minimal assembly" the maintainer is asked to confirm (§18.7). The physmap still aliases
*user* code writable at EL1; writing through it only to zero, fill tables and reach IPC buffers is a
convention (§17).

## 10. Sharing and destruction (RFC-0004 §9.1)

- **Share** is `derive` then transfer over IPC: the borrower receives, say, `READ | WRITE` without
  `EXECUTE`, `DUPLICATE` or `REVOKE`, and maps it where it chooses; bulk data then travels as RFC-0004 §5's
  descriptor. No `share` syscall and no page-table work during IPC — the long-IPC grave stays shut.
- **Destruction** — at the last live reference, or by RFC-0003 §7.1's destruction case (the right is open,
  §18.3) — marks the Region *dying* so no new mapping, binding or copy resolves it, tears down every mapping
  and IPC-buffer binding in every address space (§7 says why the walk is unavoidable), completes the
  invalidation (§12), advances the generation and refunds the payer. Its preemption points fall between
  whole mappings.
- **Donation is not proposed.** pKVM's *donate* would be `reissue` — bump and re-mint for the caller over the
  same frames, moving the charge to the recipient's Pool — which is RFC-0003 §7's option B2 on one object.
  Gating it on `REVOKE` would widen that right ("capabilities derived from this one") to reach peers, and
  under the amendment's option **(a)** a peer-transferred Region carrying `REVOKE` could cut the broker off.
  So it is **input to RFC-0003a's (a)/(b) choice**, not a verdict here; the demo needs none of it.

**Concurrency.** Under RFC-0006 (proposed) V1 — non-preemptible, one big lock when SMP lands — no kernel
access to a Region's frames (zeroing, table fill, IPC copy) interleaves with its destruction: each runs
whole within one kernel entry and re-resolves the Region there, and the dying mark covers the gaps between
destruction's steps. If locking ever becomes finer, the order is **Region before AddressSpace**. A former
sharer takes an ordinary fault: RFC-0006 (proposed) binds its delivery and RFC-0007 (proposed) defines the
message; under RFC-0007's interim the thread stops.

## 11. How the kernel touches user memory (O-5)

**The kernel never dereferences a user virtual address** — not through a checked copy or a fixup table, not
at all *(seL4: the IPC buffer is a frame reached through the kernel window)*. **Where a syscall carries one
— a `map` destination, an IPC-buffer location — it is a number, validated as a number and used only to
program tables or registers.** Everything the kernel reads or writes for a process is named by capability
and reached through the physmap:

- **The IPC buffer** is a 1 KiB-aligned slice of one page of a Region (RFC-0007 (proposed) §5 sets its size
  and syscall; RFC-0006 (proposed) records it on the Thread). Binding needs `READ | WRITE`, holds a counted
  reference, is a writable use for §9, and destruction severs it.
- **No kernel path faults on user memory**: frames are eager and pinned, so the slow path copies frame to
  frame, and the in-kernel fault concurrency that sank L4's long IPC cannot arise.
- **Peer-writable memory is never referenced.** No Rust reference is formed into it: one `kernel/src/mm/`
  module copies with volatile accesses, and anything the kernel must interpret is copied once and validated
  as that snapshot *(virtio's double-fetch lesson, research/0002 Part 5)*.

RFC-0007 inherits that sentence exactly. RFC-0003 §9 forbade naming *resources* by address; this extends it
to data. *Rejected:* `copy_from_user` with fixups (Linux), making kernel faults a normal path. `SMAP` and
`PSTATE.PAN` back the rule where present; **cortex-a72 has no PAN**, so there it stands on structure.

## 12. TLBs, ASIDs and PCIDs

- **Tags are kernel-private**: an ASID (16 bits when `ID_AA64MMFR0_EL1.ASIDBits = 0b0010`, else 8) or a
  PCID (12 bits, where CPUID reports it); tag 0 reserved. **Destroying an AddressSpace returns its tag**
  after `TLBI ASIDE1IS` (or `INVPCID` single-context), so churn does not force rollover. *Rejected:* seL4's
  ASID pools — a tag is not authority.
- **Kernel entries are global, user entries never.** `map` needs no invalidation: neither architecture caches
  a faulting entry (the Arm ARM's rule; its section is uncited because the manual was unreachable; SDM
  Vol. 3A §4.10), and a fault whose entry is valid on re-walk is retried — x86_64's paging-structure caches
  can cause one. `unmap`, `protect` and teardown store, then `DSB ISHST`, `TLBI VALE1IS` (ASID and address,
  leaf only), `DSB ISH`, `ISB`; x86_64 uses `INVLPG`, and `INVPCID` or a stale mark for other tags. Freeing an
  intermediate table uses `TLBI VAE1IS` or `ASIDE1IS`, since `VALE1IS` spares walk caches; x86_64's `INVLPG`
  and `INVPCID` already flush the PCID's paging-structure caches (SDM Vol. 3A §4.10.4.1).
- **O-8's ordering: invalidate, then free.** No frame returns to a Pool while any core can translate to it.

**Multi-core — this RFC owns the protocol; RFC-0006 (proposed) provides the IPI.** Each AddressSpace keeps
the set of cores that have run it since its last full flush *(Linux `mm_cpumask`)*.

- **Shootdown.** AArch64 needs no IPI: the `…IS` forms reach the inner-shareable domain and `DSB ISH` waits
  for completion on every core. x86_64 has no broadcast: the initiator sends RFC-0006's IPI to the cores in
  the set, each issues `INVLPG`/`INVPCID` and acknowledges, and the initiator waits for every
  acknowledgement before any frame is freed or any W^X count falls. A core spinning on the big lock polls
  its per-core flush request while it spins, so the lock holder never waits on a core waiting for it.
- **Rollover.** On exhaustion the generation bumps; every core's *active* tag is reserved into the new
  generation; every core gets a pending-flush mark and flushes locally (`TLBI VMALLE1`) at its next
  address-space switch. **A tag is reallocated only after every core that ran under it has flushed**
  *(Linux `arch/arm64/mm/context.c`: `active_asids`, `reserved_asids`, `tlb_flush_pending`)*.
- **x86_64 PCIDs are per core** (SDM Vol. 3A §4.10.1): each core caches a few AddressSpace→PCID pairs
  *(Linux keeps six)*, and an invalidation on a core where the space is not current is a per-core stale mark
  consumed at the next switch to it.

The demo is `-smp 1`, but every `TLBI` is the broadcast form from the first line; the rest is built with
RFC-0006 increment 14.

## 13. Early boot: turning the MMU on

The image is linked at `0xFFFF_FFFF_8020_0000` and loaded at 0x4020_0000 (`AT()` in `aarch64.ld`). QEMU loads
segments at `p_paddr` and translates the entry point to its physical alias **only if** it lies in an
executable segment whose `p_vaddr != p_paddr` (`include/hw/elf_ops.h`, `load_elf`, at v8.2.2) — which
`AT()` guarantees while `.text.boot` stays first in the first executable segment. With the MMU off, the stub:

1. Parks secondaries, installs the stack and zeroes `.bss` at physical addresses, as today.
2. `MAIR_EL1`: index 0 = `0xFF` (Normal write-back, read/write-allocate), index 1 = `0x04` (Device-nGnRE).
3. `TCR_EL1`: `T0SZ = T1SZ = 17`; `TG0 = 0b00`, `TG1 = 0b10` (both 4 KiB); `SH = 0b11`; `IRGN/ORGN = 0b01`;
   `IPS` from `PARange`; `AS` from `ASIDBits`; `A1 = 0`; `TBI0 = TBI1 = 0`; `HA = HD = 0`.
4. Four page-aligned boot tables in `.bss`, `DC IVAC` before and after they are written, so no dirty
   firmware line is evicted over them and no stale line shadows them *(Linux `head.S`)*: `TTBR0_EL1`
   identity-maps 0x4000_0000 as a 1 GiB Normal block and 0x0000_0000 as a 1 GiB Device block, `UXN` and
   `PXN` set, so the UART survives; `TTBR1_EL1` maps `0xFFFF_FFFF_8000_0000` to 0x4000_0000 as a 1 GiB
   Normal block. All `AF = 1`. The Normal blocks span 512 MiB with no RAM behind it — a QEMU-only interim.
5. `TLBI VMALLE1`; `IC IALLU`; `DSB NSH`; `ISB`; both `TTBR`s; `ISB`; `SCTLR_EL1` = **`0x30D4581D`**; `ISB`.
6. Branches to the high alias by absolute address, moves `SP` and `VBAR_EL1` there, and calls `rust_entry`.
   **Rust never executes at the low alias.**

| `SCTLR_EL1` field (bit) | Value | Why |
|-------------------------|-------|-----|
| `M` (0), `C` (2), `I` (12) | 1 | MMU and caches on |
| `A` (1) | 0 | alignment is the compiler's; both targets are `+strict-align` |
| `SA` (3), `SA0` (4) | 1 | stack-alignment checks at EL1 and EL0 |
| `CP15BEN` (5), `ITD` (7), `SED` (8) | 0 | AArch32 controls; no thread runs AArch32 (RFC-0007 builds every `SPSR`) |
| `UMA` (9) | 0 | EL0 cannot mask interrupts, so it cannot defeat preemption |
| `DZE` (14) | 1 | EL0 `DC ZVA` is a store its mapping already permits |
| `UCT` (15), `UCI` (26) | 0 | EL0 does no cache maintenance; the kernel does it at `map RX` (§9) |
| `nTWI` (16), `nTWE` (18) | 0, 1 | `wfi` traps and `wfe` does not — RFC-0006 (proposed) §8 |
| `WXN` (19) | 0, then 1 | set by Rust step (5): `0x30DC581D` |
| `E0E` (24), `EE` (25) | 0 | little-endian |
| 11, 20, 22, 23, 28, 29 | 1 | RES1 on Armv8.0: Linux v4.19's `SCTLR_EL1_RES1`, plus bit 23 (`SPAN`, RES1 before Armv8.1), which Linux always sets; not re-checked against the Arm ARM |

Every other bit is 0: RES0 on Armv8.0, or a later extension's control left off. Rust then, in an order that
never leaves the reporter without a console: (1) reads the DTB and builds the `MemoryMap`, bitmap and slot
arrays through the **boot window** — the frame-access trait takes a window offset, here `pa − 0x4000_0000 +
0xFFFF_FFFF_8000_0000`, so all RAM touched this early lies in the first boot GiB; (2) builds §6's final
tables, stack window included; (3) swaps `TTBR1_EL1` by an assembly routine run from the identity map — empty
table, `TLBI VMALLE1`, final table — since turning blocks into pages under a live translation needs
break-before-make *(Linux `idmap_cpu_replace_ttbr1`)*, then re-references the bitmap and switches the trait
to the physmap offset; (4) **repoints every kernel MMIO user at the MMIO window** — the UART, and the GIC
if RFC-0006 has brought it up; (5) sets `TCR_EL1.EPD0 = 1` (cleared by the first AddressSpace install), sets
`WXN`, and issues `TLBI VMALLE1`, `DSB ISH`, `ISB`. **The boot tables are never freed**: they are 16 KiB of
`.bss`, and freeing them would give image frames a second owner.

The reporter gains a DFSC and fault-level decode (it prints `FAR` already); on x86_64 `#PF` prints `CR2` and
its error code. **x86_64**, once the UEFI stub exists, is already paging; Rust builds §6's PML4 with a
temporary identity entry for a trampoline page, sets `IA32_EFER.NXE` (bit 11), `CR0.WP` (bit 16), `CR4.PGE`
(bit 7) and `CR4.PCIDE`/`SMEP`/`SMAP` (bits 17, 20, 21) where CPUID permits; assembly loads `CR3`, jumps to
the high alias and drops the identity entry — the same "Rust runs only high" rule. *Rejected:* a separate
loader *(seL4's elfloader)*; Rust building tables with the MMU off, sound only without absolute addresses.

## 14. Lineage

| Source | What is taken | What is left |
|--------|---------------|--------------|
| **seL4** | frames as capability objects; the kernel window; never touching user addresses; all memory to the first task; caller-paid tables | Untyped/retype and the CDT; execute as an ungoverned attribute; ASID pools |
| **pKVM** *(research/0002 Part 7)* | one owner per frame; *share* | *donate*, until RFC-0003a; stage-2 mechanics |
| **Zircon / Fuchsia** | a region mapped into many spaces, kept alive by mappings; `EXECUTE` as a right; `VmexResource` | the kernel heap; lazy VMOs; VMARs |
| **Hubris; Fiasco.OC, Genode** | build-time-sized tables; kernel memory as a counted, tradeable quota | a static task set; quotas on a heap |
| **Linux** | the linear map, its text alias read-only; ASID rollover and `mm_cpumask`; per-core PCIDs; arm64 headers as a checked reference | the buddy allocator; `copy_from_user` and its fixups |
| **Ge et al., EuroSys '19** | the colour-capable allocator | time protection itself, for later |

## 15. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-3 revocability | destruction walks the Region's mapping list, invalidates, then bumps; a page-table entry is a cached resolution (§7, §10) | **destruction case extended to memory**; selective revocation and donation are RFC-0003a's |
| O-5 argument validation | no user virtual address dereferenced; carried ones validated as numbers; memory by capability through the physmap; snapshot-validate peer-writable data (§11) | **discharged for the memory surface**; RFC-0007 inherits §11's sentence |
| O-6 kernel memory safety | new `unsafe` confined to `mm/**` (window access, volatile copy, the store's one static) and `arch/**` (tables, TLB, boot) | **stays Built** — every new block listed per PR |
| O-7 no unprivileged exhaustion | typed arrays reused by generation; every post-boot unit charged to a Pool named by capability, `map` included; charge-first; `map` ≤ 512 leaves; zeroing in 512-page steps; run search bounded by RAM (§4, §5, §8) | **discharged for kernel allocation**; a transferred object stays charged to its payer until destroyed — **open** with RFC-0003a (§18.2); CPU time is RFC-0006's |
| O-8 address-space disjointness | per-process tables; Regions only by capability; zero before exposure, table frames too; invalidate before free (§4, §7, §12) | **discharged** |
| O-9 no ambient side channel | no shared mapping a capability did not establish (§10) | **contributes** |
| O-13 W^X | four layers, per Region across all address spaces, counts falling only on completed invalidation (§9) | **discharged at mapping**; the load-path clause is Phase-3 policy over layer 3 |
| O-2, O-4 | `perms` ⊆ capability rights; `protect` narrows; spending needs a Pool capability; no physical-address map | **maintained** |
| O-18 confined DMA | single-extent Regions; Device Regions minted at boot | **groundwork only** — Phase 2 |

## 16. Graves checked (§3)

- **Policy in the kernel.** No address choice, demand paging, copy-on-write, overcommit or OOM response.
- **Baroque capability hierarchies.** No Untyped tree; Pools are flat, with one funder index for closure.
- **Multi-copy IPC.** The slow path copies once, frame to frame; bulk data is shared, never copied (§10).
- **Bolted-on multicore.** Broadcast invalidation from the first line; shootdown, rollover and per-core tags
  designed now (§12); only the IPI is RFC-0006's.
- **The catch-all right.** `EXECUTE` is narrow and refused off-type; no "map any physical address" right.
- **Compiled-in unused device paths (VENOM).** The kernel maps only the devices it drives itself.

## 17. Costs — what this makes harder

- **Capacities are ceilings**: raising a slot count is a rebuild, and `pool_move` is one call per resource.
- **Regions are eager and bounded**: no lazy or sparse Regions; a large one can fail `Fragmented`.
- **`map` needs the AddressSpace's own Pool**: a loader mapping into a child holds the child's Pool.
- **47-bit halves on AArch64** give up half of what `T0SZ = 16` allows; **the 64 KiB floor is fixed**.
- **User addresses are numbers only** (§11): RFC-0007 designs every syscall around registers, the IPC
  buffer and Region handles, for a bug class gone.
- **The physmap is a large target**: one kernel write bug reaches all RAM, user code included (§9) — a
  software invariant, as are W^X on x86_64 and O-5 without PAN on cortex-a72.
- **At every boot the kernel runs RWX** from MMU-on to Rust step (5), before any process exists; a stray
  early write goes uncaught.
- **Cache maintenance is untestable under QEMU** (§9, §13); **the load address is a QEMU `virt` fact**.
- **Reclaim across a transfer needs cooperation**: a transferred object keeps its payer charged until it is
  destroyed, so `pool_merge` waits on other processes — RFC-0003a's problem made concrete.

## 18. Open questions

1. **Page-table frames on unmap.** Freed only with the AddressSpace; eager freeing needs §12's walk-cache rule.
2. **Donation and reclaim across a transfer** — `reissue` with a charge move, offered to RFC-0003a (§10).
3. **Which right authorises explicit destruction** — `REVOKE` would widen it as `reissue` would; RFC-0003a.
4. **Hardware backing for two software rules:** software PAN on Armv8.0 (a `TTBR0_EL1` change per kernel
   entry), and unmapping executable user frames from the physmap.
5. **Preemptible teardown** at RFC-0006 (proposed)'s preemption points: the O(mappings) walk made restartable.
6. **The firmware assumption** (§4) for threat model §10, and O-7's new text (§5), via `docs/CHANGELOG.md`.
7. **"Minimal assembly"** (Constitution §11.1): are runtime single-instruction `TLBI`/`DC`/`IC`/`TTBR`
   intrinsics in `arch/**` within it (§9), or does the clause need amending?
8. **One programme order** across RFC-0005/0006/0007 — §19 proposes one for the maintainer to fix.

## 19. Implementation increments

One reviewable PR each; **[demo]** marks what the demo needs. Pure logic lands in a new `memory/` crate —
`no_std` in the kernel, `std` under `cargo test`, built for both targets in CI, no `unsafe`, as `capability/`.
Each new `arch` function (memory map, window access, TLB, address-space switch) gets a documented x86_64
stub: the memory-map source returns an empty `MemoryMap`, and memory initialisation prints `mm: no memory
map (UEFI stub pending)` and halts, so the second Tier-1 build compiles the whole path from increment 2.

1. **[demo] Relink at 0x4020_0000; prove the DTB.** MMU off. `--expect "DTB 0xd00dfeed at 0x40000000"`.
2. **[demo] Addresses, `MemoryMap`, FDT reader.** Host tests on a `dumpdtb` fixture and hostile blobs, and
   on a recorded OVMF `GetMemoryMap` dump once the stub exists; touches the workspace `members`,
   `.github/workflows` and `CLAUDE.md` § Layout. `--expect "mm: 512 MiB RAM at 0x40000000"`.
3. **[demo] Frame bitmap.** Host churn tests against a shadow model: no frame twice, none reserved, no image
   frame ever free.
4. **[demo] `EXECUTE` in `capability/`.** Bit 5, `ALL` and its comment, `Debug`, the test widened to `0..64`;
   with RFC-0003's dated amendment. Host-only; demo-needed because the root maps the client's text.
5. **[demo] `MapPerms` and both encoders.** Exhaustive: nothing writable and executable; every user entry
   `PXN`, `AF`, not-global; every row of §8's table.
6. **[demo] The table engine** over the frame-access trait. Host tests on both formats: overlap, the 512-leaf
   and six-table bound, nothing mapped when unpaid, tables zeroed before linking, a kernel-half map never
   writing the root.
7. **[demo] Slot store and one Pool.** Charge, refund, conservation; reuse at generation + 1; stale
   references count nothing; `EXECUTE` refused at mint off-type; `object.rs`'s comment. Host-only.
8. **`pool_split`/`move`/`merge` and closure forwarding.** Host tests under random churn. Not demo.
9. **[demo] MMU on (§13, stub).** Boot tables, the `SCTLR_EL1` value, `VBAR_EL1` high, DFSC decode.
   Prints the address of `kernel_main`: `--expect "mm: running at 0xffffffff80"`.
10. **[demo] Final kernel map (§13, Rust)**: physmap with the image read-only, MMIO window with every kernel
    MMIO user repointed, stack window with core 0's stack moved behind a guard; `EPD0`, `WXN`. Three
    self-tests, one `--expect` each, and the store behind its one static: `provoke-ro-text` and `provoke-ro-alias` write `.text` through the
    image and the physmap, expecting `data abort, same EL`; `provoke-wxn` branches into `.data`, expecting
    `instruction abort, same EL`.
11. **[demo] Region and Mapping records** with the W^X counts; host-only in `memory/`.
12. **[demo] The tag allocator**: rollover, per-core reservation, return on destruction; host-only.
13. **[demo] AddressSpace and boot-minted Regions**; `TTBR0_EL1` with ASID, `EPD0` cleared on first
    install. Boot-test: `AT S1E0R` under two ASIDs resolves one shared Region to one frame and private ones
    apart: `mm: shared region agrees`.
14. **[demo] Pool, Region and AddressSpace methods through RFC-0007's `call`**, handle words in message
    registers, with RFC-0007 increment 6: an EL0 self-test maps a fresh Region and prints `[el0] map ok`.
15. **`unmap` and `protect`** with invalidation and count decrements (§9, §12). Not demo.
16. **User-memory module (§11).** A message beyond the register budget. Not demo.
17. **Destruction** (§10), after RFC-0007 increment 11's fault message: a destroyed sharer's fault reaches its
    handler. Not demo.
18. **Stack overflow**, with RFC-0006's overflow stack: `provoke-stack-overflow` reaches the reporter.
19. **PAN, SMAP, SMEP** where reported, with a `provoke-user-deref` self-test under `-cpu max`.
20. **x86_64** tables, `CR3` and PCID, after the UEFI stub; **Device Regions** in Phase 2.

**Programme order proposed** (§18.8): 1–7 beside RFC-0006 1–2 and RFC-0007 1–5; RFC-0007's MMU-off EL0
self-test (its increment 3) lands before 9, which retires it; 9–10 before RFC-0006 3 — or 10 repoints the GIC
too — and before RFC-0006 4, which needs the stack window; 11–13 before RFC-0006 5 and RFC-0007 6–7.

**Interims the demo carries, with their cost:**

- **One root Pool.** Increment 8 is not a syscall yet: server and client charges are indistinguishable.
- **No destruction or `unmap`**: a killed process's frames and slots stay charged; O-3's memory case is
  designed, not built. **No IPC-buffer module**: messages beyond the register budget cannot be sent.
- **Hard-coded UART and GIC physical addresses**, against QEMU's warning that they may vary.
- **Until increment 10, the kernel runs RWX throughout**; cache maintenance stays unverified.
- **1 GiB boot blocks over unbacked space** (§13): real hardware needs 2 MiB blocks sized from the DTB.
- **Zeroing in one step** until RFC-0006's preemption points; the demo's Regions are a few pages.
- **An overflow of the guarded stack is a recursive abort** until increment 18.
- **Not an interim:** the root's image reaches it through boot-minted and kernel-filled Regions and
  increment 13's `map`, so §8's rights check runs even at boot.

## 20. What this unblocks

Increments 1–14 give RFC-0006 (proposed) address spaces, a guarded per-core stack and a Pool to charge
threads to, and RFC-0007 (proposed) the objects its Process, root task and boot info are made of, plus a
constraint it inherits rather than discovers: **the kernel never dereferences a user virtual address; where
a syscall carries one, it is a number, validated as a number and used only to program tables or registers.**
The root task receives all memory as one boot-minted Pool — RFC-0003 §14.4's bootstrap without an ambient
grantor. Increments 3–8 are host-only and 1–2 change only where the kernel loads and what it prints: all can
land now.
