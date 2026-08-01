// SPDX-License-Identifier: GPL-3.0-or-later

//! The Setonix capability table — RFC-0003.
//!
//! Unforgeable, rights-attenuating references to kernel objects. This crate
//! holds the *mechanism*: architecture-independent, `no_std`, allocation-free
//! logic, unit-tested on the host. It lives outside the kernel crate precisely
//! so it *can* be host-tested — a bare-metal target has no test harness — and
//! so the security spine can be exercised exhaustively away from the hardware.
//!
//! What the design guarantees, and where it lives:
//!
//! - **Unforgeability (O-1).** Userspace holds a [`Handle`] — a table index plus
//!   a [`Generation`] — never the capability itself, which is kernel memory.
//!   [`CapabilityTable::resolve`] is the single gate through which a handle is
//!   exercised against its object, re-checked in full on every call; a stale
//!   handle fails closed against the generation rather than aliasing a reused
//!   slot. ([`CapabilityTable::remove`] relocates authority for transfer or
//!   close, after the same stale-handle check — but every exercise of that
//!   authority still funnels through `resolve`, whose borrow the caller holds
//!   across the whole check→act window.)
//! - **Non-widenability (O-2).** [`Rights`] only ever *diminish*: derivation
//!   ([`Capability::derive`], [`CapabilityTable::derive`]) is subset-only, and
//!   no operation anywhere adds a right.
//! - **Revocability, the destruction half (O-3).** Destroying an object bumps
//!   its generation ([`ObjectRef::current_generation`]); every capability
//!   minted before that instant fails closed at its next resolve, with no list
//!   of holders required. Selective revocation is RFC-0003a's question, still
//!   open — until it lands, this is the only revocation there is.
//! - **Transfer is a move (§6).** A [`Capability`] is owned and neither `Clone`
//!   nor `Copy`: it leaves one table by [`CapabilityTable::remove`] and enters
//!   another by [`CapabilityTable::insert`] as a Rust move, never live in two
//!   places at once.
//!
//! Per the RFC-0003 prior-art amendment, the compile-time guarantees here cover
//! the kernel's *internal* handling; the userspace-observable cross-process
//! transfer is a runtime table operation the generation scheme secures — the
//! borrow checker cannot span protection domains, so this crate does not pretend
//! it does.
//!
//! Still to come, each its own reviewable increment (§5.3): wiring into the
//! kernel's syscall surface once there are kernel objects to reference, and
//! selective revocation once RFC-0003a decides it.

#![cfg_attr(not(test), no_std)]

mod capability;
mod error;
mod generation;
mod handle;
mod object;
mod rights;
mod table;

#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) mod test_support;

pub use capability::Capability;
pub use error::CapabilityError;
pub use generation::Generation;
pub use handle::Handle;
pub use object::ObjectRef;
pub use rights::Rights;
pub use table::CapabilityTable;
