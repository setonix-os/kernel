// SPDX-License-Identifier: GPL-3.0-or-later

//! x86_64 support — **placeholder**.
//!
//! The bring-up order in `CLAUDE.md` §6 is QEMU aarch64 `virt` first, then QEMU
//! x86_64 `q35`. This module exists so that the second Tier-1 target keeps
//! compiling and keeps failing to compile when the hardware-abstraction boundary
//! is violated, which is the whole reason for building both from day one. It
//! does not boot.
//!
//! What is missing, in the order it will be needed:
//!
//! - A UEFI entry path. `q35` has no equivalent of `-kernel` for a bare ELF, so
//!   this needs a proper PE/COFF stub or a bootloader shim plus an ESP image.
//! - A console. The 16550 UART at `0x3f8` is the counterpart of the PL011, and
//!   is where the console will land; the UEFI GOP framebuffer comes later.
//! - GDT, IDT and a stack, none of which AArch64 needs before its first print.
//!
//! Designated `unsafe` module: privileged instructions.
#![allow(unsafe_code)]

/// The ELF entry point.
///
/// **This image cannot boot.** There is no UEFI stub, no multiboot header, no
/// descriptor tables and no stack of our own, so nothing will ever transfer
/// control here in practice.
///
/// It nevertheless calls straight through to the kernel proper, and that is the
/// point: it makes the architecture-independent kernel genuinely compile and link
/// against this hardware-abstraction implementation. An entry point that merely
/// halted would leave every line above [`crate::arch`] as dead code on this
/// target, and the second Tier-1 build would prove nothing at all — which is the
/// opposite of why `CLAUDE.md` §6 insists both architectures are first-class from
/// day one. The compiler caught exactly that mistake in the first draft.
///
/// # Safety
///
/// Never call this from Rust. It is the image's entry symbol.
#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    crate::kernel_main()
}

/// Writes a string to the architecture's console.
///
/// No-op until there is a 16550 driver. Returning silently rather than panicking
/// is intentional: the architecture-independent kernel above is entitled to call
/// this, and a panic here would make the placeholder look like a bug in a caller
/// that is behaving correctly.
// Not `const`, although on this architecture it could be: the hardware-
// abstraction boundary requires both implementations to present the same
// signature, and the AArch64 one performs memory-mapped I/O and never can be.
// Marking only this one `const` would make the wrapper in `super` const-able on
// one target and not the other — a leak of the very kind the boundary exists to
// prevent, and one that would disappear again the moment a real console lands
// here.
#[allow(clippy::missing_const_for_fn)]
pub(super) fn console_write_str(_s: &str) {}

/// Parks this core in a low-power halt state, permanently.
pub(super) fn halt() -> ! {
    loop {
        // SAFETY: `hlt` has no operands and no memory effects. It halts until an
        // interrupt arrives; since none are enabled, and since the loop reissues
        // it regardless, the core stops here for good. Requires ring 0, which is
        // where the image starts.
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
