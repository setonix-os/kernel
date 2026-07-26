// SPDX-License-Identifier: GPL-3.0-or-later
//
// AArch64 boot stub — the first Setonix instructions to execute.
//
// DRAFT: the Borrow Ledger marks the boot path "write ourselves, human-first".
// This is a sparring-partner draft, not a merge candidate: read it, verify every
// line against the architecture reference manual, and re-express it in your own
// hand before it lands (CLAUDE.md §5.1, §5.3).
//
// Entry state on QEMU 'virt':
//   - The image is loaded at 0x4000_0000 and entered at _start with the MMU off,
//     caches off, and interrupts masked.
//   - Execution begins at EL1 by default. This stub does not care which
//     exception level it is at, because it only touches memory-mapped I/O and
//     never programs a system register that is EL-specific. That stops being
//     true the moment the MMU or the exception vectors are set up, at which
//     point this stub must gain an explicit EL2-to-EL1 descent.
//   - Every core enters here simultaneously. Only the one with MPIDR_EL1.Aff0
//     == 0 continues; the rest are parked until there is a scheduler able to
//     receive them.
//
// Register discipline: x1 and x2 are scratch. Nothing is expected on entry and
// nothing is returned, because rust_entry is declared to never return.
//
// Symbol addresses use adrp/add rather than a literal pool, so that the stub is
// position-independent with respect to the linker's placement and assembles
// under LLVM's integrated assembler without relying on GNU-only syntax.

.section .text.boot,"ax"
.globl _start

_start:
    // Park every core but the first.
    mrs     x1, mpidr_el1
    and     x1, x1, #0xff
    cbnz    x1, .Lpark

    // Install the boot stack before touching memory that could need one.
    adrp    x1, __stack_top
    add     x1, x1, :lo12:__stack_top
    mov     sp, x1

    // Zero .bss. Rust assumes statics start zeroed; nothing else does this.
    adrp    x1, __bss_start
    add     x1, x1, :lo12:__bss_start
    adrp    x2, __bss_end
    add     x2, x2, :lo12:__bss_end
.Lzero_bss:
    cmp     x1, x2
    b.hs    .Lbss_zeroed
    str     xzr, [x1], #8
    b       .Lzero_bss
.Lbss_zeroed:

    // Into Rust. The architecture-specific entry shim calls the kernel proper.
    bl      rust_entry

    // rust_entry is `-> !`, so reaching here means something is badly wrong.
    // Halt rather than fall into whatever follows in memory.
.Lhang:
    wfe
    b       .Lhang

.Lpark:
    wfe
    b       .Lpark
