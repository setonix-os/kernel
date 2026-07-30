// SPDX-License-Identifier: GPL-3.0-or-later

//! The Setonix capability table — RFC-0003.
//!
//! Unforgeable, rights-attenuating references to kernel objects. This crate
//! holds the *mechanism*: architecture-independent, `no_std`, allocation-free
//! logic, unit-tested on the host. It lives outside the kernel crate precisely
//! so it *can* be host-tested — a bare-metal target has no test harness — and
//! so the security spine can be exercised exhaustively away from the hardware.
//!
//! This first increment is the value types the table is built from. The owned,
//! no-`Clone` capability and the flat per-process table follow, one reviewable
//! step at a time (§5.3).
//!
//! What the design guarantees, and where it lives:
//!
//! - **Unforgeability (O-1).** Userspace holds a [`Handle`] — a table index plus
//!   a [`Generation`] — never the capability itself, which is kernel memory. A
//!   stale handle fails closed against the generation rather than aliasing a
//!   reused slot.
//! - **Non-widenability (O-2).** [`Rights`] only ever *diminish*: no operation
//!   anywhere adds a right, and [`Rights::diminish`] is the sole way to change a
//!   held capability's rights.
//!
//! Per the RFC-0003 prior-art amendment, the compile-time guarantees here cover
//! the kernel's *internal* handling; the userspace-observable cross-process
//! transfer is a runtime table operation the generation scheme secures — the
//! borrow checker cannot span protection domains, so this crate does not pretend
//! it does.

#![cfg_attr(not(test), no_std)]

mod error;
mod generation;
mod handle;
mod rights;

pub use error::CapabilityError;
pub use generation::Generation;
pub use handle::Handle;
pub use rights::Rights;
