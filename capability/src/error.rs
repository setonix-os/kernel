// SPDX-License-Identifier: GPL-3.0-or-later

//! Errors from capability-table operations.

/// Why a capability-table operation failed.
///
/// Every variant fails closed: the operation grants no authority and changes no
/// state a caller could mistake for success. There is no "partial" outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapabilityError {
    /// The handle's index lies outside the table.
    OutOfBounds,
    /// The slot the handle names is empty.
    Empty,
    /// The handle or capability is stale: the slot is occupied at a different
    /// generation than the handle's, or is retired (a reused or dead handle,
    /// RFC-0003 O-1) — or the capability's minted generation no longer matches
    /// its object's, because the object was destroyed (the destruction half of
    /// O-3). A closed handle whose slot is still empty reports
    /// [`Empty`](Self::Empty) instead: the state check answers first.
    StaleGeneration,
    /// The requested rights are not a subset of the source capability's: a
    /// widening was attempted, which the type refuses (RFC-0003 O-2).
    RightsNotSubset,
    /// The source capability lacks `DUPLICATE`, so nothing may be derived from it.
    NotDuplicable,
    /// The table has no free slot to mint into.
    TableFull,
    /// An object's generation counter is exhausted, so the object must be
    /// retired: kept inert, its identity never reused, nothing minted to it
    /// again (the fail-closed boundary — see the generation contract on
    /// [`ObjectRef`](crate::ObjectRef)). Reserved vocabulary for the kernel's
    /// object-destruction path: no *table* operation returns it. Slot-side
    /// exhaustion is handled silently by retiring the slot —
    /// [`remove`](crate::CapabilityTable::remove) still succeeds, resolve
    /// reports [`StaleGeneration`](Self::StaleGeneration) and insert reports
    /// [`TableFull`](Self::TableFull).
    GenerationExhausted,
}
