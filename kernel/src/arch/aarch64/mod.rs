// SPDX-License-Identifier: GPL-3.0-or-later

//! AArch64 support: boot entry, console, core control.
//!
//! This module is one of the two trees `CLAUDE.md` designates for `unsafe`. The
//! workspace denies `unsafe_code`; the opt-in below is what makes the set of
//! designated modules greppable, and every block within still carries its own
//! `// SAFETY:` justification.
#![allow(unsafe_code)]

mod uart;

// The boot stub is assembled by LLVM as part of this crate rather than by an
// external assembler, so the toolchain stays exactly as pinned — no host `as`
// creeps into the build.
core::arch::global_asm!(include_str!("boot.s"));

/// The Rust entry point, called by `_start` in `boot.s` once there is a stack
/// and `.bss` has been zeroed.
///
/// Architecture-specific one-time setup belongs here, before the kernel proper
/// is entered. At present there is none: the console needs no initialisation on
/// QEMU's `virt`, because the firmware has already configured the PL011.
///
/// # Safety
///
/// Called exactly once, by the boot stub, on the boot core only. Never call it
/// from Rust.
#[unsafe(no_mangle)]
extern "C" fn rust_entry() -> ! {
    crate::kernel_main()
}

/// Writes a string to the PL011 console.
pub(super) fn console_write_str(s: &str) {
    uart::write_str(s);
}

/// Parks this core in a low-power wait state, permanently.
pub(super) fn halt() -> ! {
    loop {
        // SAFETY: `wfe` has no operands and no memory effects. It either waits
        // for an event or returns immediately; both are correct here, because
        // the loop simply issues it again. It is architecturally valid at every
        // exception level.
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}
