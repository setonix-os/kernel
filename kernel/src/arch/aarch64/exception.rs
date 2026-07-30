// SPDX-License-Identifier: GPL-3.0-or-later

//! The exception reporter — the instrument every later subsystem is debugged
//! with.
//!
//! Until there is a scheduler, every exception is terminal: the only job is to
//! say *what* happened, *where*, and *why*, precisely enough that no one ever
//! again spends an afternoon in a QEMU instruction trace working out that
//! `CPACR_EL1.FPEN` was zero. That bug — the kernel dying one instruction
//! before its first console character, on an FP/SIMD trap to a vector table
//! that did not exist — is the reason this module exists and the standard its
//! output is held to: the decoded line for exception class 0x07 names the
//! register that causes it.
//!
//! The reporter prints line by line, on the deliberately lock-free console,
//! for the same reason the panic handler does: when the machine is dying, half
//! a message on the wire is evidence, and a lock is a way to hang instead of
//! speak. It never returns — recovery is a policy decision, and there is
//! nothing yet to recover to.
//!
//! Designated `unsafe` module (`kernel/src/arch/**`): the entry point below
//! carries `#[unsafe(no_mangle)]` so the vector stub in `vectors.s` can reach
//! it by name.
#![allow(unsafe_code)]

use crate::console::kprintln;

/// Names the table entry an exception arrived through.
///
/// The vector index encodes group (bits 3:2) and kind (bits 1:0), matching the
/// architectural table layout in `vectors.s`.
const fn vector_name(index: u64) -> (&'static str, &'static str) {
    let group = match index >> 2 {
        0 => "current EL, SP_EL0",
        1 => "current EL, SP_ELx",
        2 => "lower EL, AArch64",
        _ => "lower EL, AArch32",
    };
    let kind = match index & 0b11 {
        0 => "synchronous",
        1 => "IRQ",
        2 => "FIQ",
        _ => "SError",
    };
    (kind, group)
}

/// Decodes `ESR_EL1.EC` (bits 31:26) into the sentence a person needs.
///
/// Only classes this kernel can plausibly meet are named; everything else
/// reports its raw value rather than guessing. The list grows as subsystems
/// do — a decoded class costs one line here and saves a datasheet lookup at
/// the worst possible moment there.
const fn exception_class_name(ec: u64) -> &'static str {
    match ec {
        0b00_0000 => "unknown reason",
        0b00_0001 => "trapped WFI/WFE",
        0b00_0111 => {
            "FP/SIMD access trapped by CPACR_EL1.FPEN — the kernel is soft-float; \
                      something emitted a vector instruction"
        }
        0b00_1110 => "illegal execution state",
        0b01_0101 => "SVC (AArch64 supervisor call)",
        0b01_1000 => "trapped MSR/MRS or system instruction",
        0b10_0000 => "instruction abort from lower EL (execute fault)",
        0b10_0001 => "instruction abort, same EL (execute fault — jumped somewhere unmapped?)",
        0b10_0010 => "PC alignment fault",
        0b10_0100 => "data abort from lower EL (read/write fault)",
        0b10_0101 => "data abort, same EL (read/write fault — FAR holds the address)",
        0b10_0110 => "SP alignment fault",
        0b10_1100 => "trapped floating-point exception (AArch64)",
        0b10_1111 => "SError interrupt",
        0b11_0000 => "breakpoint from lower EL",
        0b11_0001 => "breakpoint, same EL",
        0b11_0010 => "software step from lower EL",
        0b11_0011 => "software step, same EL",
        0b11_0100 => "watchpoint from lower EL",
        0b11_0101 => "watchpoint, same EL",
        0b11_1100 => "BRK instruction (AArch64)",
        _ => "unrecognised exception class — decode ESR against the Arm ARM",
    }
}

/// The exception reporter. Reached only from the vector stubs in `vectors.s`.
///
/// Prints the decoded exception, then halts this core for good. Written as
/// separate lines so a fault partway through the report still leaves the
/// earlier lines on the wire.
///
/// # Safety
///
/// Not for calling from Rust — `vectors.s` branches here with the syndrome
/// registers marshalled into the argument registers. The `no_mangle` exists
/// solely so the assembly can name it.
#[unsafe(no_mangle)]
extern "C" fn aarch64_exception(index: u64, esr: u64, elr: u64, far: u64, spsr: u64) -> ! {
    let (kind, group) = vector_name(index);
    let ec = (esr >> 26) & 0x3f;

    kprintln!();
    kprintln!("[EXCEPTION] {kind}, {group} (vector {index})");
    kprintln!("  EC   = {ec:#04x} — {}", exception_class_name(ec));
    kprintln!("  ESR  = {esr:#018x}");
    kprintln!("  ELR  = {elr:#018x}   (faulting PC)");
    kprintln!("  FAR  = {far:#018x}");
    kprintln!("  SPSR = {spsr:#018x}");
    kprintln!();
    kprintln!("Core halted.");

    super::halt()
}
