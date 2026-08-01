// SPDX-License-Identifier: GPL-3.0-or-later

//! The hardware-abstraction boundary.
//!
//! This module is the only place in the kernel that knows which architecture it
//! is being built for. Nothing above it may name one, and no `cfg(target_arch)`
//! belongs outside this tree — the whole point of both Tier-1 architectures
//! being first-class from day one is that the boundary is forced honest by use
//! rather than by intention (CONSTITUTION.md §6).
//!
//! The interface below is deliberately tiny. It grows one function at a time,
//! each added only when a caller above genuinely needs it, because a
//! hardware-abstraction layer designed ahead of its callers ends up shaped like
//! whichever architecture was written first.

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
use aarch64 as imp;

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
use x86_64 as imp;

/// Writes a string to the architecture's console device.
///
/// Blocks until the last byte has been accepted by the hardware. There is no
/// buffering and no locking: until there is a scheduler there is nothing to
/// serialise against, and a lock here would be a lock the panic handler could
/// deadlock on.
pub(crate) fn console_write_str(s: &str) {
    imp::console_write_str(s);
}

/// Stops this core permanently, in the lowest-power state available.
///
/// Used at the end of `kernel_main` and by the panic handler. Never returns and
/// never resumes — a core that reaches this is out of the system.
pub(crate) fn halt() -> ! {
    imp::halt()
}

/// Deliberately takes a synchronous exception, so the boot self-test can prove
/// the exception reporter works end to end rather than trusting that it would.
///
/// Compiled only under the `provoke-exception` feature; nothing enables it by
/// default, and the ordinary boot path never calls it.
#[cfg(feature = "provoke-exception")]
pub(crate) fn provoke_exception() -> ! {
    imp::provoke_exception()
}
