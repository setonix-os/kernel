// SPDX-License-Identifier: GPL-3.0-or-later
//
// AArch64 exception vector table.
//
// The architecture dictates nearly everything here, which is why this file is
// short: VBAR_EL1 points at a 2 KiB-aligned table of sixteen entries, each 128
// bytes apart, in four groups of four. Groups, in order: exceptions taken from
// the current EL while on SP_EL0; from the current EL on SP_ELx; from a lower
// EL running AArch64; from a lower EL running AArch32. Within each group:
// synchronous, IRQ, FIQ, SError.
//
// Every entry does the same two things — load its own index into x0 and branch
// to the common stub — because at this stage of the kernel every exception has
// the same meaning: something unexpected happened, and the only job is to say
// so on the console with enough precision that nobody spends an afternoon in
// an instruction trace. The handler never returns, so registers are clobbered
// freely and no frame is saved. This table is an instrument, not a
// context-switch path: when IRQs become real (timer, scheduler), the relevant
// entries grow a full register save and a return path, and the clobbering
// stops being acceptable.
//
// Lower-EL entries are unreachable until userspace exists, and the SP_EL0
// group is unreachable while the kernel stays on SP_ELx — but the architecture
// requires the slots, and wiring them to the reporter costs nothing. Exception
// entry sets PSTATE.SP to 1, so the handler always starts on SP_ELx whichever
// group it arrived through; the common stub re-asserts SPSel before touching
// memory as defence in depth.
//
// Register discipline in the stub: x0 carries the vector index from the entry;
// x1-x4 receive the four syndrome registers; nothing is preserved, because
// aarch64_exception is declared `-> !`.

.section .text.vectors, "ax"
.balign 2048
.globl __vectors

__vectors:
    // --- Group 0: current EL, SP_EL0 ---------------------------------------
    .balign 0x80
    mov     x0, #0                  // synchronous
    b       .Lvector_common
    .balign 0x80
    mov     x0, #1                  // IRQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #2                  // FIQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #3                  // SError
    b       .Lvector_common

    // --- Group 1: current EL, SP_ELx ---------------------------------------
    // The group the kernel actually lives in today.
    .balign 0x80
    mov     x0, #4                  // synchronous
    b       .Lvector_common
    .balign 0x80
    mov     x0, #5                  // IRQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #6                  // FIQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #7                  // SError
    b       .Lvector_common

    // --- Group 2: lower EL, AArch64 ----------------------------------------
    // Unreachable until userspace exists.
    .balign 0x80
    mov     x0, #8                  // synchronous
    b       .Lvector_common
    .balign 0x80
    mov     x0, #9                  // IRQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #10                 // FIQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #11                 // SError
    b       .Lvector_common

    // --- Group 3: lower EL, AArch32 ----------------------------------------
    // Doubly unreachable: no userspace, and AArch32 is out of scope (§6).
    .balign 0x80
    mov     x0, #12                 // synchronous
    b       .Lvector_common
    .balign 0x80
    mov     x0, #13                 // IRQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #14                 // FIQ
    b       .Lvector_common
    .balign 0x80
    mov     x0, #15                 // SError
    b       .Lvector_common

.Lvector_common:
    // Re-assert SP_ELx selection. Exception entry already sets PSTATE.SP to 1
    // — the SP_EL0 group records the interrupted code's stack, not the
    // handler's — so this is defence in depth, not a required step.
    msr     spsel, #1

    // The four syndrome registers, in argument order for aarch64_exception:
    //   x1 = ESR_EL1  — what happened (exception class + syndrome)
    //   x2 = ELR_EL1  — where it happened (faulting PC)
    //   x3 = FAR_EL1  — which address, for aborts and alignment faults
    //   x4 = SPSR_EL1 — the interrupted state
    mrs     x1, esr_el1
    mrs     x2, elr_el1
    mrs     x3, far_el1
    mrs     x4, spsr_el1

    bl      aarch64_exception

    // aarch64_exception is `-> !`; reaching here means the handler itself is
    // broken. Park rather than fall through into whatever follows.
.Lvector_hang:
    wfe
    b       .Lvector_hang
