// SPDX-License-Identifier: GPL-3.0-or-later
//
// AArch64 boot stub — the first Setonix instructions to execute.
//
// Assembly is the one place the constitution allows outside Rust, and it is kept
// to the minimum: boot and context switching, nothing else. Every instruction
// here should be checked against the Arm Architecture Reference Manual by the
// reviewer before it merges — §5.3 delegates the writing of the kernel core, not
// the understanding of it, and this file is the least forgiving in the tree.
//
// Entry state on QEMU 'virt':
//   - The image is loaded at 0x4000_0000 and entered at _start with the MMU off,
//     caches off, and interrupts masked.
//   - Execution begins at EL1: QEMU's built-in loader enters a `-kernel` image
//     at EL1 unless the machine is created with virtualization=on, and the
//     xtask harness never passes that. This stub now DOES program EL1-specific
//     registers (VBAR_EL1 below; SPSel in the vector stub), so the EL1
//     assumption is load-bearing. Before real hardware or virtualization=on,
//     this stub owes an explicit CurrentEL check and EL2-to-EL1 descent —
//     at EL2 the writes below would be legal but useless, and a later fault
//     would vector through VBAR_EL2 = 0 into exactly the silent death the
//     table exists to prevent.
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

    // Install the exception vector table before entering Rust, so that even
    // the first Rust instruction faults diagnosably instead of vectoring to
    // address zero. VBAR_EL1 requires 2 KiB alignment; vectors.s guarantees it.
    // The isb orders the write before any instruction that could fault.
    adrp    x1, __vectors
    add     x1, x1, :lo12:__vectors
    msr     vbar_el1, x1
    isb

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
