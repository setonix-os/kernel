<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# RFC-0005 — Address spaces and physical memory

| Field | Value |
|-------|-------|
| Status | **Proposed** — 2026-09-27, awaiting the maintainer's verdict |
| Author | Drafted by Claude Code as sparring partner; verdict the maintainer's |
| Date | 2026-09-27 |
| Affects | Constitution §3 (kernel doctrine) and §11.1's "minimal assembly" (§17.7); RFC-0003 (a dated amendment adding one right; three new objects; `capability/src/rights.rs` and `object.rs`'s doc comment); RFC-0004 §9.1; the boot stub, `aarch64.ld` and the exception reporter; `kernel/src/mm/**`; a new host-tested `memory/` crate; threat model §10 and O-7's text (proposals via `docs/CHANGELOG.md`) |
| Depends on | RFC-0003 (capability table), RFC-0004 (IPC); interlocks with RFC-0006 (threads and scheduling) and RFC-0007 (system calls and process bootstrap), proposed the same day |
| Discharges | O-7 for kernel allocation (reclaim across a transfer stays open), O-8, O-13 (the mapping half); O-5 for every path by which the kernel touches user memory; O-3's destruction case extended to memory; answers RFC-0003 §14.3 and RFC-0004 §9.1 (share) |

> **Proposed verdicts.** **[demo]** marks what the demo stands on; the rest is forward design.
>
> 1. **[demo] Discovery:** firmware tables into one bounded `MemoryMap`; the image moves to 0x4020_0000, where QEMU leaves room for the DTB (§4).
> 2. **[demo] Frames:** a colour-capable bitmap; fresh frames and table frames zeroed, boot data handed over read-only (§4, §7).
> 3. **[demo] Who pays (O-7):** typed slot arrays reused by generation; every post-boot unit charged to a Pool named by capability, `map`'s too (§5).
> 4. **[demo] One accounting domain:** the Pool, for every counted unit; time stays with RFC-0006's `Core`, a stated split from research/0002 (§5).
> 5. **[demo] Layout:** the same higher-half layout on both; the image read-only in the physmap; per-core stacks guarded, kept by the kernel (§6).
> 6. **[demo] Objects:** `Region` (eager and zeroed, or boot-minted read-only) and `AddressSpace` join RFC-0003's objects beside `Pool` (§7).
> 7. **[demo] Operations:** `map` / `unmap` / `protect` are capability-checked, bounded and atomic; the kernel never picks an address (§8).
> 8. **[demo] Rights:** `EXECUTE` joins RFC-0003's set as bit 5 — on Regions and Pools only, refused at mint elsewhere — by dated amendment (§8).
> 9. **[demo] W^X (O-13):** unrepresentable, enforced per Region across all address spaces, backed by `SCTLR_EL1.WXN` (§9).
> 10. **Sharing (RFC-0004 §9.1):** *share* is derive-and-transfer; destruction walks mappings; *donation* goes to RFC-0003a as input (§10).
> 11. **[demo] User memory (O-5):** the kernel never dereferences a user virtual address; a carried one is a number, validated as one (§11).
> 12. **TLB:** kernel-private, recycled tags; this RFC owns SMP shootdown and rollover, RFC-0006 provides the IPI; single-core half [demo] (§12).
> 13. **[demo] Early boot:** the stub enables the MMU with a stated `SCTLR_EL1`; Rust runs only high and installs the final W^X map (§13).

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
and 47-bit user and kernel halves whose top level uses 256 entries each** — AArch64's 4 KiB granule with
`TCR_EL1.T0SZ = T1SZ = 17`, x86_64's four-level paging without LA57. The leaves differ:

| Property | AArch64 (VMSAv8-64, 4 KiB granule) | x86_64 (IA-32e, 4-level) |
|---|---|---|
| User / kernel split | two roots: `TTBR0_EL1` (low), `TTBR1_EL1` (high) | one root, `CR3`; PML4 entries 0–255 user, 256–511 kernel |
| Writable | `AP[2]` (bit 7) clear | bit 1 (R/W) set |
| User-accessible | `AP[1]` (bit 6) set | bit 2 (U/S) set |
| No-execute | `PXN` (bit 53) and `UXN` (bit 54), separately | `XD` (bit 63), only with `IA32_EFER.NXE` |
| Global | `nG` (bit 11) clear | `G` (bit 8) set, with `CR4.PGE` |
| Accessed | `AF` (bit 10), set by software on this CPU | bit 5 (A), set by hardware |
| Address-space tag | 16-bit ASID in `TTBR0_EL1[63:48]` | 12-bit PCID in `CR3[11:0]` |

*(Checked against Linux's `arch/arm64/include/asm/pgtable-hwdef.h` — Arm's site was unreachable — and SDM
Vol. 3A §4.5. QEMU's `cortex-a72` has 44-bit physical addresses, 16-bit ASIDs, no hardware access flag and
no PAN: `target/arm/tcg/cpu64.c`, `ID_AA64MMFR0_EL1 = 0x1124`, `MMFR1` 0.)* Hence **one table engine**,
generic over a `Descriptor` encoder and a frame-access trait, host-tested in `memory/`; only turning a frame
number into `&mut [u64; 512]` through a kernel window (§6, §13) is `unsafe`, in `kernel/src/mm/`.

## 4. Physical memory: discovery and the frame allocator

**Discovery.** QEMU fixes RAM at 0x4000_0000 on `virt`; every other address "may vary" and comes from the
DTB, "at the start of RAM" with no register naming it (`docs/system/arm/virt.rst`) — and `hw/arm/boot.c`
puts it only *below* the image (`dtb_limit`). Checked on QEMU 8.2.2 on 2026-09-27: an ELF at 0x4000_0000
finds **no DTB in RAM**; at 0x4020_0000 it finds one at 0x4000_0000. **So the image loads at 0x4020_0000**,
capping the DTB at 2 MiB; increment 1 re-checks QEMU 11.0.3. A minimal FDT reader (Borrow Ledger boot-path
row; Devicetree Specification v0.4) reads the cell sizes, `/memory`, `/memreserve/` and `/reserved-memory`,
bounds-checks every offset and fails closed; threat model §10 gains a proposed assumption, *firmware tables
describe memory truthfully*. x86_64's UEFI stub (no RFC yet) hands over `GetMemoryMap` after
`ExitBootServices`. **Both produce one `MemoryMap`**: same-kind neighbours coalesced, then at most **256**
sorted, disjoint `(PhysRange, Kind)` entries (raw UEFI maps run to 100–200) — `Usable`, `KernelImage`,
`DeviceTree`, `Reserved`, and `Mmio` from Phase 1's hard-coded addresses — failing closed beyond that; image,
boot stack, DTB and boot tables tagged first. Nothing above `arch` names FDT or UEFI.

**The frame allocator is a bitmap** (16 KiB per 512 MiB, in the first free frames) allocating from a *colour
set* — `pfn mod C`, `C = 1` until time protection *(Ge et al., EuroSys '19, via research/0002 Part 6)* — or
runs of `n` at an alignment; a double free fails closed. A run search is bounded by RAM, not the caller; a
per-alignment cursor if measurement asks. **Fresh frames are zeroed before a process can observe them, and a
table frame before its parent descriptor is written** (a `DSB` between), lest stale entries translate into
another's frames. *Rejected:* a **buddy allocator** *(Linux)*, whose coalescing fights colouring; a **free
list**, which cannot find `n` contiguous frames without a scan.

## 5. Who pays: the object store and Pools (O-7)

The kernel has no heap and must never acquire one a process can drain. **A — seL4 Untyped/retype** reclaims
through a derivation tree, deciding RFC-0003a's B1 by the back door (RFC-0003 §7 and its 2026-07-30
amendment), and makes object layouts ABI: **rejected**. **B — a heap with creation policy** *(Zircon)* limits
*whether*, not *how much*: **rejected**. **C — static per-type tables** *(Hubris)* cannot say *who* filled one
(research/0002 Part 6): **rejected alone**. **D — C with counted attribution: verdict sought** *(the requester
pays, as in seL4; counted quotas, as in Fiasco.OC's factory limits and Genode's quota trading)*.

**The store is typed slot arrays**, one fixed-capacity array per type in `.bss`; a slot holds the object, a
generation and a count. The kernel's implementation of `ObjectRef` (`capability/src/object.rs`, a trait) is
`(type, index, generation)`; `current_generation()` is a slot read. **Only live references count** —
capabilities at the slot's current generation, Mappings, IPC-buffer bindings; one at any other generation
counts nothing on clone or drop. A destroyed object's slot is therefore **reused at once, generation
advanced**, stale capabilities failing closed — what RFC-0003 §8's generation exists for — and no past holder
pins another's charge. A slot whose generation cannot advance retires, debited once from its payer (objects
keep 64-bit generations, RFC-0007 (proposed) §6). `object.rs`'s "takes a reference count" is qualified to
match in increment 7. **Where it lives:** generic array and Pool in `memory/`; the concrete store, one
`Kernel` value with every type's array (RFC-0004/0006/0007 objects too), behind **one** `static` in
`kernel/src/mm/` whose SAFETY argument is one core entered with interrupts masked (RFC-0006 (proposed) V1),
then the big lock. No other kernel global accretes.

**A Pool is the accounting layer**: an RFC-0003 object holding balances of frames and of each type's slots.
At boot every free unit becomes the balance of one root Pool (RFC-0007 (proposed) hands it over). Then:

```text
Σ balance(r) over all Pools  ==  units of r free        -- at every instant after boot
every unit of r in use       ==  kept by the kernel (at boot, or retired), or charged to exactly one Pool
```

- **Charged:** Region frames; every object slot; table frames and the `Mapping` slot, to the Pool `map`
  names (§8). **Kept, never charged:** image, bitmap, arrays, kernel tables, per-core stacks, boot-minted
  Regions (§7). Every charge presents a Pool with `WRITE`.
- **Charge first, then take**, so the only later failure, `Fragmented`, refunds before returning.
  `PoolExhausted` names the Pool and resource; nothing overcommits *(research/0002 Part 6, "budgets are
  minima")*; refund goes to the payer, whose index each object records.
- **Pools subdivide by moving balance.** `pool_split(pool)` makes an empty child recording its funder;
  `pool_move(from, into, r, n)`; `pool_merge(from, into)` once `from` has no outstanding charges. **A Pool
  whose last capability closes** returns its free balance to its funder and stays as a forwarding slot until
  its charges come home — a chain bounded by the Pool slot count; the root's balance goes to the kernel.
- **Rights:** `READ` queries; `WRITE` spends and moves; `EXECUTE` lets minted Regions carry `EXECUTE` (§9).

**One accounting domain — decided here, as the programme gives this RFC O-7's model.** research/0002 Part 6
wants one for memory, object quotas and scheduling contexts. **The Pool is it for every counted unit**,
SchedContext slots included. **Time is not a unit**: RFC-0006 (proposed) sells per-core bandwidth through
`Core`, which moving balance cannot conserve, and folding it in would make every Pool holder a CPU seller — a
deliberate split, congruent by attribution. O-7 would add *"kernel memory is charged to a capability-named
Pool; CPU time to a SchedContext funded through a `Core`"*, via `docs/CHANGELOG.md`.

## 6. The kernel's virtual layout

Both architectures get the **same layout**, the cheapest way to keep the HAL honest; the image sits in the
top 2 GiB, as `x86_64-unknown-none`'s default kernel code model expects (overridable, but that buys nothing).

| Range (both architectures) | Contents | Attributes |
|----------------------------|----------|------------|
| `0x0000_0000_0001_0000` – `0x0000_7FFF_FFFF_EFFF` | the current AddressSpace (§7) | per mapping; not-global; never kernel-executable |
| `0xFFFF_8000_0000_0000` – `+64 TiB` | physmap: every discovered RAM frame at `base + pa` | Normal write-back; execute-never; global; RW, but the image's `.text`/`.rodata` read-only |
| `0xFFFF_C000_0000_0000` – `+1 TiB` | MMIO window: kernel-owned devices only (console, interrupt controller) | Device-nGnRE / uncached; RW; execute-never |
| `0xFFFF_FFFF_0000_0000` – `0xFFFF_FFFF_7FFF_FFFF` | per-core kernel and emergency stacks, an unmapped guard page below each | Normal; RW; execute-never; kept by the kernel at boot |
| `0xFFFF_FFFF_8000_0000` – top | image at `+0x20_0000` | `.text` RX, `.rodata` R, `.data`/`.bss` RW execute-never |

- **Stacks.** RFC-0006 (proposed) V1 chose one kernel stack per core, allocated here at boot; core 0's boot
  stack leaves `aarch64.ld`'s unguarded slot in increment 10. A fault's entry pushes onto the overflowed
  stack, so **RFC-0006 (proposed) provides the overflow stack** (x86_64 IST; AArch64's EL1-origin vector
  moving to the emergency stack, a kernel fault being fatal). Until then an overflow is a recursive abort.
- **The user floor is 64 KiB** *(Linux `mmap_min_addr`)*; **the ceiling leaves the last page below the hole
  unmapped**, closing Intel's `SYSRET` hazard (CVE-2012-0217) for RFC-0007 by layout. The physmap is RAM only.
- **The kernel's top-level entries never change after boot.** AArch64 shares `TTBR1_EL1`; x86_64 copies PML4
  entries 256–511 into each address space *(seL4 x86)*, so every kernel-half PML4E ever used is populated at
  boot — one filled later would be missing from earlier copies (Linux's vmalloc-sync bugs); lower kernel
  tables may change, invalidated on every core (§12). *Rejected:* an identity-mapped kernel, where user
  addresses go; no physmap, an invalidation per IPC-buffer touch.

## 7. The objects: Region, AddressSpace and Mapping

**Frame ownership is pKVM's state machine** (research/0002 Part 7): every frame has exactly one owner — the
kernel, or one `Region`. **A Region** is a fixed number of pages over at most eight extents of contiguous
frames, with rights `READ`, `WRITE`, `EXECUTE` and RFC-0003's `DUPLICATE`, `TRANSFER`, `REVOKE`. Mappings
and IPC-buffer bindings hold counted references, so closing the last capability does not pull memory from
under a live mapping *(Zircon: a mapping keeps its VMO alive)*. One object, three kinds:

- **Ordinary** (`region_create`): eager, zeroed, charged; unmet in eight extents it fails `Fragmented`; DMA
  memory is one extent at an alignment. Zeroing runs 512 pages per step between RFC-0006 (proposed)'s
  preemption points, the half-built Region held on the caller's thread; no capability exists until done.
- **Boot-minted, read-only**, over the DTB and each boot module RFC-0007 (proposed) embeds (every segment
  page-aligned): their frames **move** into the Region, never zeroed (public boot data, so O-8 is untouched),
  never charged, never `WRITE` — the DTB `READ`, a module `READ | EXECUTE | DUPLICATE | TRANSFER`, so text
  maps `RX` in place and W^X holds because no writable use can exist.
- **Device**, minted at boot over `Mmio` the kernel does not keep; never executable, zeroed or charged;
  Phase 2. No operation maps a physical address given as a number: the catch-all right in costume.

What the kernel fills for the root before handover (writable segments, stack, BootInfo) goes into ordinary
Regions, charged to the root Pool, bytes copied in through the physmap, and only then a capability minted.
**An AddressSpace** is a user root table, a tag (§12), the Pool paying for its tables — the only Pool `map`
may charge — and its mappings; `WRITE` permits `map`/`unmap`/`protect`, `READ` a query; binding it to a
process is RFC-0007's. **A Mapping** is a charged kernel record, not a capability: `{as, region, first_page,
vaddr, pages, perms}`, linked into its AddressSpace's list and its Region's, because RFC-0003's 2026-07-30
amendment makes load-bearing that *a resolved capability is never cached outside the kernel table* — and **a
page-table entry is exactly such a cache**. *Rejected:* **Zircon VMOs and VMARs** (lazy commit, copy-on-write,
pagers, nesting: policy, and faults in kernel paths); **seL4's capability per frame**; **user-built tables** (Xen PV).

## 8. Map, unmap, protect — and the rights they check

Every operation takes handles (RFC-0003 §9), encoded as methods by RFC-0007 (proposed); a further handle
(`map`'s Region and Pool) is a **handle word in a message register**, resolved in the caller's table, never
transferred — RFC-0003 §6's move would take the Region from its mapper.

| Operation | Needs | Refused when |
|-----------|-------|--------------|
| `region_create(pool, pages, max_extents, align, rights)` | pool `WRITE`; pool `EXECUTE` if `rights` has it | `PoolExhausted`; `Fragmented`; zero pages; `align` not a power-of-two page count; `rights` not Region rights |
| `as_create(pool, rights)` | pool `WRITE` | `PoolExhausted` |
| `map(as, region, pool, first_page, vaddr, pages, perms)` | as `WRITE`; region rights ⊇ `perms`; `pool` the AddressSpace's own, `WRITE` | `vaddr` unaligned, below the floor or outside the user half; overlap; beyond the Region; `pages > 512`; `perms ∉ {R, RW, RX}`; W^X (§9); `PoolExhausted` |
| `unmap(as, vaddr)` | as `WRITE` | no mapping based at `vaddr` — the unit is the whole mapping |
| `protect(as, vaddr, perms)` | as `WRITE` | `perms` not a subset of the mapping's current `perms` |

User numbers are validated *as numbers*; **the kernel never chooses a virtual address** *(seL4's VSpace
discipline)*. **`map` names a Pool because spending is authority** (O-4); requiring the space's *own* Pool
keeps one payer per space, so refunds need no per-table record *(seL4's caller-supplied page tables)*.
*Rejected:* any Pool (a payer record per table); a Pool bound at `as_create` and spent through `WRITE`
(spending never granted). **`map` is atomic**: it charges missing tables and the Mapping slot first, then
zeroes and links tables, writes leaves and ends with `DSB ISHST` before `eret`; at most 512 leaves and six
new tables. `protect` only narrows; widening is `map` again *(seL4)*. 4 KiB pages; blocks later.

**Verdict sought: `EXECUTE` joins RFC-0003's rights as bit 5**, as §5 there allows "when a subsystem needs
one". Folded into `READ`, every readable Region is potential code; a Region *kind* fails the loader, which
writes code before running it. seL4 leaves execution ungoverned; Fuchsia's `ZX_RIGHT_EXECUTE` is the lineage.
It means *map executable* on a Region, *mint executable Regions* on a Pool, and is **refused at mint on every
other type** (increment 7); `derive` cannot add it. `rights.rs`'s `ALL` is an explicit union despite its
comment; increment 4 fixes both. **On acceptance, RFC-0003 gains a dated amendment adding `EXECUTE` to §5's
table, logged in `docs/CHANGELOG.md`.** RFC-0003 §14.3's handshake: **installed permission is `perms`,
refused unless a subset of the Region's rights.**

| perms | needs on the Region | AArch64 stage 1 | x86_64 |
|-------|---------------------|-----------------|--------|
| R | `READ` | `AP[2:1] = 0b11`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 1` |
| RW | `READ`, `WRITE` | `AP[2:1] = 0b01`, `UXN = 1`, `PXN = 1` | `P`, `U/S`, `R/W = 1`, `XD = 1` |
| RX | `READ`, `EXECUTE` | `AP[2:1] = 0b11`, `UXN = 0`, `PXN = 1` | `P`, `U/S`, `R/W = 0`, `XD = 0` |

Every user entry also sets `AF = 1`, `nG = 1`, inner-shareable, Normal write-back. Write-only is refused;
execute-only exists on AArch64 but not on x86_64 without protection keys, so neither offers it.

## 9. W^X (O-13), in four layers

1. **Unrepresentable.** `MapPerms` is `Read`, `ReadWrite` or `ReadExecute`; both encoders are total functions
   from it, so no writable-executable entry can be built. Host-tested exhaustively.
2. **Per Region, across every address space.** A Region counts writable uses (`RW` mappings, IPC-buffer
   bindings) and `RX` mappings: `map RX` is refused while any writable use exists, `map RW` and binding while
   any `RX` mapping does. `unmap`, `protect` and unbinding decrement **only once their invalidation has
   completed on every core** (§12) — until then a stale writable entry may be live somewhere.
3. **Over time.** `EXECUTE` is minted only from a Pool carrying it — the loader holds one, a JIT gets one by
   explicit grant *(Fuchsia's `VmexResource`)*. O-13's Phase-3 load-path clause is policy on this.
4. **Hardware backstop.** AArch64 sets `SCTLR_EL1.WXN` (bit 19) once the final tables are in (§13); user
   entries set `PXN`; kernel text has no writable alias, the physmap mapping it read-only *(Linux
   `mark_linear_text_alias_ro`)*. x86_64 has no `WXN`: `CR0.WP` and `SMEP` where present.

`map RX` makes AArch64's instruction side coherent: `DC CVAU` via the physmap, `DSB ISH`, `IC IVAU` if PIPT
(`IC IALLUIS` otherwise; Linux's `sync_icache_aliases`), `DSB ISH`, `ISB`; cortex-a72 reports PIPT (`CTR_EL0 =
0x8444c004`); x86_64 is coherent in hardware. **QEMU models no cache, so errors here pass every test.** These
and §12's `TLBI`s and `TTBR` writes are single-instruction `asm!` intrinsics in `kernel/src/arch/**`, as `wfe`
is today — a reading of "minimal assembly" for the maintainer to confirm (§17.7). The physmap still aliases
*user* code writable at EL1; writing through it only to zero, fill and reach IPC buffers is a convention (§16).

## 10. Sharing and destruction (RFC-0004 §9.1)

- **Share** is `derive` then transfer over IPC: the borrower receives, say, `READ | WRITE` without
  `EXECUTE`, `DUPLICATE` or `REVOKE`, and maps it where it chooses; bulk data travels as RFC-0004 §5's
  descriptor. No `share` syscall and no page-table work during IPC — the long-IPC grave stays shut.
- **Destruction** — at the last live reference, or through RFC-0003 §7.1's destruction case (its right is
  open, §17.3) — marks the Region *dying*, so no new mapping, binding or copy resolves it; tears down every
  mapping and binding in every address space; completes the invalidation (§12); advances the generation and
  refunds the payer. Its preemption points fall between whole mappings.
- **Donation is not proposed.** pKVM's *donate* would be `reissue` — bump, re-mint for the caller over the
  same frames, move the charge to the recipient's Pool — RFC-0003 §7's option B2 on one object. Gating it on
  `REVOKE` would widen that right beyond "capabilities derived from this one", and under the amendment's
  option **(a)** a peer-transferred Region carrying `REVOKE` could cut the broker off. It is **input to
  RFC-0003a's (a)/(b) choice**; the demo needs none of it.

**Concurrency.** Under RFC-0006 (proposed) V1 — non-preemptible, one big lock once SMP lands — no kernel
access to a Region's frames (zeroing, table fill, IPC copy) interleaves with its destruction: each runs whole
in one entry and re-resolves the Region; the dying mark covers destruction's gaps. Finer locking takes
**Region before AddressSpace**. A former sharer faults: RFC-0006 (proposed) binds delivery, RFC-0007 (proposed)
defines the message; under its interim the thread stops.

## 11. How the kernel touches user memory (O-5)

**The kernel never dereferences a user virtual address** — not through a checked copy or a fixup table, not
at all *(seL4: the IPC buffer is a frame reached through the kernel window)*. **Where a syscall carries one
— a `map` destination, an IPC-buffer location — it is a number, validated as a number and used only to
program tables or registers.** RFC-0007 inherits that sentence exactly. Everything the kernel reads or
writes for a process is named by capability and reached through the physmap:

- **The IPC buffer** is a 1 KiB-aligned slice of one page of a Region (RFC-0007 (proposed) §5 sets size and
  syscall; RFC-0006 (proposed) records it on the Thread). Binding needs `READ | WRITE`, holds a counted
  reference, is a writable use for §9, and destruction severs it.
- **No kernel path faults on user memory**: frames are eager and pinned, so the slow path copies frame to
  frame, and the in-kernel fault concurrency that sank L4's long IPC cannot arise.
- **Peer-writable memory is never referenced**: one `kernel/src/mm/` module copies with volatile accesses;
  anything interpreted is copied once and validated as that snapshot *(virtio's double fetch, research/0002
  Part 5)*. *Rejected:* `copy_from_user` with fixups (Linux). `SMAP` and `PAN` back the rule where present;
  **cortex-a72 has no PAN**, so there it stands on structure.

## 12. TLBs, ASIDs and PCIDs

- **Tags are kernel-private**: an ASID (16 bits when `ID_AA64MMFR0_EL1.ASIDBits = 0b0010`, else 8) or a PCID
  (12 bits, where CPUID reports it); tag 0 reserved. **Destroying an AddressSpace returns its tag** after
  `TLBI ASIDE1IS` (x86_64: `INVPCID` single-context), so churn does not force rollover. *Rejected:* seL4's
  ASID pools — a tag is not authority.
- **Kernel entries are global, user entries never.** `map` needs no invalidation: neither architecture caches
  a faulting entry (the Arm ARM, section uncited as it was unreachable; SDM Vol. 3A §4.10), and a fault whose
  entry is valid on re-walk is retried. `unmap`, `protect` and teardown store, `DSB ISHST`, `TLBI VALE1IS`,
  `DSB ISH`, `ISB`; x86_64 `INVLPG`. Freeing an intermediate table needs `TLBI VAE1IS` or `ASIDE1IS`, as
  `VALE1IS` spares walk caches; `INVLPG`/`INVPCID` flush them (SDM Vol. 3A §4.10.4.1). **O-8's ordering:
  invalidate, then free** — no frame returns to a Pool while any core can translate to it.

**Multi-core: this RFC owns the protocol; RFC-0006 (proposed) provides the IPI**, built with its SMP
increment; the `-smp 1` demo already uses only broadcast `TLBI`s. Each AddressSpace keeps the set of cores
that have run it since its last full flush *(Linux `mm_cpumask`)*.

- **Shootdown.** AArch64 needs no IPI: the `…IS` forms reach the inner-shareable domain and `DSB ISH` waits
  for completion everywhere. x86_64 has no broadcast: the initiator IPIs the cores in the set, each
  invalidates and acknowledges, and **no frame is freed nor W^X count lowered before every acknowledgement**.
  A core spinning on the big lock polls its flush request, so the holder never waits on a core waiting for it.
- **Rollover.** The generation bumps, every core's *active* tag is reserved into the new one, and every core
  gets a pending-flush mark, consumed by a local `TLBI VMALLE1` at its next switch. **A tag is reallocated
  only after every core that ran under it has flushed** *(Linux `arch/arm64/mm/context.c`: `active_asids`,
  `reserved_asids`, `tlb_flush_pending`)*.
- **x86_64 PCIDs are per core**: each core caches a few AddressSpace→PCID pairs *(Linux keeps six)*; an
  invalidation where the space is not current is a per-core stale mark consumed at the next switch to it.

## 13. Early boot: turning the MMU on

The image is linked at `0xFFFF_FFFF_8020_0000`, loaded at 0x4020_0000 (`AT()`). QEMU translates the entry to its
physical alias **only if** it lies in an executable segment with `p_vaddr != p_paddr` (`include/hw/elf_ops.h`,
`load_elf`, v8.2.2), which `AT()` gives while `.text.boot` leads that segment. With the MMU off, the stub:

1. Parks secondaries, installs the stack and zeroes `.bss` at physical addresses, as today.
2. `MAIR_EL1`: index 0 = `0xFF` (Normal write-back, read/write-allocate), index 1 = `0x04` (Device-nGnRE).
3. `TCR_EL1`: `T0SZ = T1SZ = 17`; `TG0 = 0b00`, `TG1 = 0b10` (both 4 KiB); `SH = 0b11`; `IRGN/ORGN = 0b01`;
   `IPS` from `PARange`; `AS` from `ASIDBits`; `A1 = 0`; `TBI0 = TBI1 = 0`; `HA = HD = 0`.
4. Four page-aligned boot tables in `.bss`, with `DC IVAC` before and after writing them, so no dirty firmware
   line is evicted over them and none shadows them *(Linux `head.S`)*: `TTBR0_EL1` identity-maps 0x4000_0000
   as a 1 GiB Normal block and 0x0000_0000 as a 1 GiB Device block with `UXN` and `PXN`, so the UART
   survives; `TTBR1_EL1` maps `0xFFFF_FFFF_8000_0000` to 0x4000_0000 as a 1 GiB Normal block; all `AF = 1`.
5. `TLBI VMALLE1`; `IC IALLU`; `DSB NSH`; `ISB`; both `TTBR`s; `ISB`; `SCTLR_EL1` = **`0x30D4581D`**; `ISB`.
6. Branches to the high alias by absolute address, moves `SP` and `VBAR_EL1` there, calls `rust_entry`.
   **Rust never executes at the low alias.**

| `SCTLR_EL1` field (bit) | Value | Why |
|-------------------------|-------|-----|
| `M` (0), `C` (2), `SA` (3), `SA0` (4), `I` (12) | 1 | MMU and caches on; stack-alignment checks at EL1 and EL0 |
| `A` (1), `CP15BEN` (5), `ITD` (7), `SED` (8), `E0E` (24), `EE` (25) | 0 | alignment is the compiler's (`+strict-align`); AArch32 controls, and no thread runs AArch32; little-endian |
| `UMA` (9), `UCT` (15), `UCI` (26) | 0 | EL0 may neither mask interrupts (defeating preemption) nor maintain caches — the kernel does that at `map RX` (§9) |
| `DZE` (14) | 1 | EL0 `DC ZVA` is a store its mapping already permits |
| `nTWI` (16), `nTWE` (18) | 0, 1 | `wfi` traps and `wfe` does not — RFC-0006 (proposed) §8 |
| `WXN` (19) | 0, then 1 | set by Rust step (5), giving `0x30DC581D` |
| 11, 20, 22, 23, 28, 29 | 1 | RES1 on Armv8.0: Linux v4.19's `SCTLR_EL1_RES1`, plus bit 23 (`SPAN`, RES1 before Armv8.1), which Linux always sets; not re-checked against the Arm ARM |

Every other bit is 0 (RES0 on Armv8.0, or a later extension left off). Rust then, never leaving the reporter
without a console: (1) builds the `MemoryMap`, bitmap and arrays through the **boot window** — the
frame-access trait's offset is `pa − 0x4000_0000 + 0xFFFF_FFFF_8000_0000`, so all it touches lies in the boot
GiB; (2) builds §6's final tables; (3) swaps `TTBR1_EL1` from the identity map in assembly — empty table,
`TLBI VMALLE1`, final table, as break-before-make requires *(Linux `idmap_cpu_replace_ttbr1`)* — and moves
the trait and bitmap to the physmap offset; (4) **repoints every kernel MMIO user** (UART; GIC if up);
(5) sets `TCR_EL1.EPD0 = 1` (the first AddressSpace install clears it) and `WXN`, then `TLBI VMALLE1`, `DSB
ISH`, `ISB`. **The boot tables are never freed**: freeing `.bss` would give image frames a second owner.

The reporter gains a DFSC and fault-level decode (it prints `FAR` already); x86_64's `#PF` prints `CR2` and
its error code. **x86_64**, once the UEFI stub exists, is already paging: Rust builds §6's PML4 with a
temporary identity entry for a trampoline page and sets `IA32_EFER.NXE` (bit 11), `CR0.WP` (bit 16),
`CR4.PGE` (bit 7) and `CR4.PCIDE`/`SMEP`/`SMAP` (bits 17, 20, 21) where CPUID permits; assembly loads `CR3`,
jumps high and drops the identity entry — the same "Rust runs only high" rule. *Rejected:* a separate loader
*(seL4's elfloader)*; Rust building tables with the MMU off, sound only without absolute addresses.

## 14. Obligations

| Obligation | This RFC | Status after implementation |
|-----------|----------|-----------------------------|
| O-3 revocability | destruction walks the Region's mappings, invalidates, then bumps; a page-table entry is a cached resolution (§7, §10) | **destruction case extended to memory**; selective revocation and donation are RFC-0003a's |
| O-5 argument validation | no user virtual address dereferenced; carried ones are numbers; memory by capability through the physmap; snapshots (§11) | **discharged for the memory surface**; RFC-0007 inherits §11's sentence |
| O-6 kernel memory safety | new `unsafe` only in `mm/**` (window access, volatile copy, the store's static) and `arch/**` (tables, TLB, boot) | **stays Built** — every new block listed per PR |
| O-7 no unprivileged exhaustion | arrays reused by generation; every post-boot unit charged to a Pool named by capability, `map`'s too; charge-first; `map` ≤ 512 leaves; zeroing in 512-page steps; run search bounded by RAM (§4, §5, §8) | **discharged for kernel allocation**; a transferred object stays charged to its payer until destroyed — **open** with RFC-0003a (§17.2); CPU time is RFC-0006's |
| O-8 address-space disjointness | per-process tables; Regions only by capability; frames and tables zeroed before use; invalidate before free (§4, §7, §12) | **discharged** |
| O-9 no ambient side channel | no shared mapping a capability did not establish (§10) | **contributes** |
| O-13 W^X | four layers, per Region across all spaces, counts falling only on completed invalidation (§9) | **discharged at mapping**; the load-path clause is Phase-3 policy over layer 3 |
| O-2, O-4 | `perms` ⊆ rights; `protect` narrows; spending needs a Pool capability; no physical-address map | **maintained** |
| O-18 confined DMA | single-extent Regions; Device Regions minted at boot | **groundwork only** — Phase 2 |

## 15. Graves checked (§3)

- **Policy in the kernel.** No address choice, demand paging, copy-on-write, overcommit or OOM response.
- **Baroque capability hierarchies.** No Untyped tree; Pools are flat, with one funder index for closure.
- **Multi-copy IPC.** The slow path copies once, frame to frame; bulk data is shared, never copied (§10).
- **Bolted-on multicore.** Broadcast `TLBI` from the first line; shootdown, rollover, per-core tags designed now (§12).
- **The catch-all right.** `EXECUTE` is narrow and refused off-type; no "map any physical address" right.
- **Compiled-in unused device paths (VENOM).** The kernel maps only the devices it drives itself.

## 16. Costs — what this makes harder

- **Capacities are ceilings** (a rebuild); **Regions are eager and bounded** (no lazy or sparse ones).
- **`map` needs the AddressSpace's own Pool**: a loader mapping into a child holds the child's Pool.
- **47-bit halves on AArch64** give up half of what `T0SZ = 16` allows; **the 64 KiB floor is fixed**.
- **User addresses are numbers only** (§11): every RFC-0007 syscall pays in ergonomics, for a bug class gone.
- **The physmap is a large target** (§9) — a software invariant, as are W^X on x86_64 and O-5 without PAN.
- **Every boot runs RWX** from MMU-on to Rust step (5), before any process exists; a stray early write goes
  uncaught. **Cache maintenance is untestable under QEMU**; **the load address is a QEMU `virt` fact**.
- **Reclaim across a transfer needs cooperation**: the payer stays charged until destruction (RFC-0003a).

## 17. Open questions

1. **Page-table frames on unmap.** Freed only with the AddressSpace; eager freeing needs §12's walk-cache rule.
2. **Donation and reclaim across a transfer** — `reissue` with a charge move, offered to RFC-0003a (§10).
3. **Which right authorises explicit destruction** — `REVOKE` would widen as `reissue` would; RFC-0003a.
4. **Hardware backing for software rules:** software PAN on Armv8.0; unmapping executable user frames from the physmap.
5. **Preemptible teardown**: the O(mappings) walk made restartable at RFC-0006 (proposed)'s preemption points.
6. **The firmware assumption** (§4) and O-7's new text (§5), via `docs/CHANGELOG.md`.
7. **"Minimal assembly"** (Constitution §11.1): are runtime one-instruction `TLBI`/`DC`/`IC`/`TTBR` intrinsics within it (§9)?
8. **One programme order** across RFC-0005/0006/0007 — §18 proposes one for the maintainer to fix.

## 18. Implementation increments

One reviewable PR each; **[demo]** marks what the demo needs. Pure logic lands in a new `memory/` crate as
`capability/` does (`no_std` in the kernel, `std` under test, both targets in CI, no `unsafe`). Each new `arch`
function gets a documented x86_64 stub — an empty `MemoryMap`, on which memory initialisation prints `mm: no
memory map (UEFI stub pending)` and halts — so the second Tier-1 build compiles the whole path.

1. **[demo] Relink at 0x4020_0000; prove the DTB.** MMU off. `--expect "DTB 0xd00dfeed at 0x40000000"`.
2. **[demo] Addresses, `MemoryMap`, FDT reader.** Host tests on a `dumpdtb` fixture, hostile blobs and later an
   OVMF dump; workspace `members`, `.github/workflows`, `CLAUDE.md` § Layout. `--expect "mm: 512 MiB RAM at 0x40000000"`.
3. **[demo] Frame bitmap.** Churn tests against a shadow model: no frame twice, none reserved, no image frame free.
4. **[demo] `EXECUTE` in `capability/`** with RFC-0003's amendment: bit 5, `ALL`, `Debug`, tests to `0..64`.
5. **[demo] `MapPerms` and both encoders.** Exhaustive: never W+X; every user entry `PXN`, `AF`, not-global.
6. **[demo] Table engine**, both formats: overlap, bounds, unpaid maps nothing, tables zeroed, root untouched by kernel maps.
7. **[demo] Slot store and one Pool.** Conservation; reuse at generation + 1; stale references count nothing; mint refuses off-type `EXECUTE`.
8. **`pool_split`/`move`/`merge` and closure forwarding**, under random churn. Host-only; not demo.
9. **[demo] MMU on (§13, stub)**, `SCTLR_EL1`, `VBAR_EL1` high, DFSC decode: `--expect "mm: running at 0xffffffff80"`.
10. **[demo] Final kernel map (§13, Rust)**, the store behind its static: physmap with the image read-only,
    MMIO window with every kernel MMIO user repointed, core 0's stack behind a guard; `EPD0`, `WXN`. One
    `--expect` per self-test: `provoke-ro-text` and `provoke-ro-alias` write `.text` through the image and
    the physmap (`data abort, same EL`); `provoke-wxn` branches into `.data` (`instruction abort, same EL`).
11. **[demo] Region and Mapping records** with the W^X counts. Host-only.
12. **[demo] The tag allocator**: rollover, per-core reservation, return on destruction. Host-only.
13. **[demo] AddressSpace and boot-minted Regions**; `TTBR0_EL1` with ASID; `EPD0` cleared. `AT S1E0R` under
    two ASIDs finds a shared Region at one frame, private ones apart: `mm: shared region agrees`.
14. **[demo] Pool, Region, AddressSpace methods via RFC-0007's `call`** (its increment 6): `[el0] map ok`.
15. **`unmap` and `protect`** with invalidation and count decrements (§9, §12). Not demo.
16. **User-memory module (§11)**: a message beyond the register budget. Not demo.
17. **Destruction (§10)**, after RFC-0007 increment 11's fault message: a destroyed sharer's fault reaches its handler.
18. **Stack overflow**, with RFC-0006's overflow stack: `provoke-stack-overflow` reaches the reporter.
19. **PAN, SMAP, SMEP** where reported, with a `provoke-user-deref` self-test under `-cpu max`.
20. **x86_64** tables, `CR3` and PCID, after the UEFI stub; **Device Regions** in Phase 2.

**Programme order proposed** (§17.8): 1–8 beside RFC-0006 1–2 and RFC-0007 1–5; RFC-0007's MMU-off EL0
self-test (its 3) before 9, which retires it; 9–10 before RFC-0006 3 (or 10 repoints the GIC too) and 4,
which needs the stack window; 11–13 before RFC-0006 5 and RFC-0007 6–7; 14 with RFC-0007 6.

**Interims the demo carries, with their cost:**

- **One root Pool** (increment 8 is not a syscall yet): server and client charges are indistinguishable.
- **No destruction or `unmap`**: a killed process's frames and slots stay charged; O-3's memory case is
  designed, not built. **No IPC-buffer module**: messages beyond the register budget cannot be sent.
- **Hard-coded UART and GIC addresses**, against QEMU's warning. **Until increment 10 the kernel runs RWX.**
- **1 GiB boot blocks** cover 512 MiB with no RAM behind them, which speculation may touch on real hardware:
  there, 2 MiB blocks sized from the DTB. **Zeroing in one step** until RFC-0006's preemption points.
- **A guarded-stack overflow is a recursive abort** until increment 18.
- **Not an interim:** the root's image reaches it through boot-minted and kernel-filled Regions and
  increment 13's `map`, so §8's rights check runs even at boot.

## 19. What this unblocks

Increments 1–14 give RFC-0006 (proposed) address spaces, a guarded per-core stack and a Pool to charge threads
to; RFC-0007 (proposed) the objects its Process, root task and boot info are made of, and §11's sentence on
user addresses as a constraint inherited, not discovered. The root receives all memory as one boot-minted
Pool — RFC-0003 §14.4's bootstrap without an ambient grantor. Increments 1–8 leave the MMU off: all can land now.
