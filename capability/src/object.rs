// SPDX-License-Identifier: GPL-3.0-or-later

//! The object side of the contract — what the table requires of a kernel
//! object reference.

use crate::Generation;

/// A counted reference to a kernel object, as the capability table sees one.
///
/// The table is deliberately generic over the reference type. It moves, clones
/// and drops references and asks exactly one question — *what generation is the
/// referenced object at now?* Everything else about kernel objects (what they
/// are, how they are allocated, how their reference counts work) belongs to the
/// kernel crate, which implements this trait for its object references once
/// those exist. Until then a test double implements it on the host, which is
/// how the whole scheme stays exhaustively testable away from the hardware.
///
/// `Clone` is a supertrait because a counted reference is duplicable *as a
/// reference*: cloning it copies a pointer and takes a reference count, and
/// duplicates no authority. Authority lives in
/// [`Capability`](crate::Capability), which is deliberately not `Clone` — its
/// only duplication is [`derive`](crate::Capability::derive), explicit and
/// rights-checked.
///
/// # The generation contract
///
/// An implementation must uphold what RFC-0003 §7–§8 rely on:
///
/// - The returned generation only ever advances: successive calls never yield
///   an earlier value, and a generation, once left, is never occupied again.
/// - Destroying the object advances its generation past every capability
///   minted for it, which is what makes those capabilities inert (the
///   destruction half of O-3):
///   [`CapabilityTable::resolve`](crate::CapabilityTable::resolve) checks the
///   minted generation against this one on every call.
/// - If the generation cannot advance — [`Generation::next`] returns [`None`],
///   the fail-closed boundary — the object must be retired: kept inert, its
///   identity never reused, no new capability ever minted to it. The error
///   vocabulary for that path is
///   [`CapabilityError::GenerationExhausted`](crate::CapabilityError::GenerationExhausted).
/// - A cloned reference confers no authority. Acting on the object is
///   authorised only by a live capability, resolved and *held* — as a borrow —
///   across the caller's whole check→act window
///   ([`CapabilityTable::resolve`](crate::CapabilityTable::resolve) states
///   why). Extracting the reference from a resolved capability and acting
///   after the borrow is gone would re-open the race with revocation the
///   generation check closes: `Clone` exists for table bookkeeping, not for
///   acting outside a resolve.
pub trait ObjectRef: Clone {
    /// The referenced object's current generation.
    fn current_generation(&self) -> Generation;
}
