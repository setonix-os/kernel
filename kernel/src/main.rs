// SPDX-License-Identifier: GPL-3.0-or-later

//! The Setonix microkernel.
//!
//! Mechanism, never policy. This file is the architecture-independent front door
//! and is expected to stay short: everything that knows about hardware lives
//! below [`arch`], and everything that decides anything will live in userspace.
//!
//! There is no `unsafe` in this file, and there should never be. The workspace
//! denies `unsafe_code`; the only modules that lift the denial are the ones
//! `CLAUDE.md` designates, and the full list can be recovered with
//! `grep -rn "allow(unsafe_code)" kernel/src/`.

#![no_std]
// The entry symbol comes from the architecture's boot path, not from a `main`
// that a runtime would call — there is no runtime.
#![no_main]

mod arch;
mod console;
mod panic;

/// The kernel proper.
///
/// Entered from the architecture's boot path once there is a stack and `.bss` has
/// been zeroed. Never returns: when there is nothing left to do, the core halts.
///
/// Phase 1 will grow this into scheduler, IPC, capability table and MMU
/// initialisation. Today it proves the chain beneath it works, which is a
/// smaller claim but the one everything else rests on.
pub(crate) fn kernel_main() -> ! {
    console::greet();

    // The exception self-test: greet first (proving the ordinary path), then
    // fault on purpose and let CI grep the console for the decoded report.
    // Nothing enables this feature by default.
    #[cfg(feature = "provoke-exception")]
    arch::provoke_exception();

    #[cfg(not(feature = "provoke-exception"))]
    arch::halt()
}
