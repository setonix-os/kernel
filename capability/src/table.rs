// SPDX-License-Identifier: GPL-3.0-or-later

//! The flat per-process capability table — RFC-0003 §4, Option B.

use crate::{Capability, CapabilityError, Generation, Handle, ObjectRef, Rights};
use core::fmt;
use core::mem;

/// One slot of the table: its generation, and what currently occupies it.
struct Slot<O> {
    /// For an occupied slot: the generation its handle was minted at. For a
    /// free slot: the generation the *next* mint will use — bumped at close
    /// time, so a closed handle dies at the instant of closing, not merely
    /// when its slot is reused.
    generation: Generation,
    state: SlotState<O>,
}

/// What a slot currently holds.
enum SlotState<O> {
    /// Empty. A recycled slot additionally links the free list; a never-used
    /// slot carries `None` and is reached through the high-water mark instead.
    Free { next_free: Option<u32> },
    /// Holds a live capability.
    Occupied(Capability<O>),
    /// The slot's generation space is exhausted: never offered again, so no
    /// stale handle can ever match a wrapped counter (the fail-closed
    /// boundary — see [`Generation::next`]).
    Retired,
}

/// A process's capability table: the flat array mapping [`Handle`]s to the
/// [`Capability`]s the process holds (RFC-0003 §4, Option B).
///
/// One table per process, `N` slots, no allocation and no tree: resolution is
/// an array index plus two generation checks, O(1) by construction. The
/// capacity is a compile-time bound because the kernel has no allocator, and
/// because pillar 2 prefers bounded structures a reviewer can hold in their
/// head; a full table fails closed with [`CapabilityError::TableFull`].
///
/// Slot reuse is guarded by a per-slot [`Generation`], bumped when a slot is
/// vacated — so a closed handle is dead from the moment of closing, and a
/// reused slot never honours the previous occupant's handle (the ABA defence,
/// O-1). A slot whose generation cannot advance is retired outright: unusable
/// forever, at the cost of one slot of capacity.
///
/// The division of labour is deliberately visible in the trait bounds:
/// [`insert`](Self::insert) and [`remove`](Self::remove) are pure slot
/// mechanics — the plumbing a transfer is built from — and know nothing about
/// objects; [`resolve`](Self::resolve) and [`derive`](Self::derive) are the
/// authority checks, and require [`ObjectRef`]. Rights enforcement on
/// *invocation* (does this capability permit `WRITE`?) belongs to the caller
/// that resolves, per RFC-0003 §9 — the table answers what a handle names,
/// not what a syscall may do with it.
///
/// # Example — the life of a capability
///
/// ```
/// use core::cell::Cell;
/// use std::rc::Rc;
/// use setonix_capability::{
///     Capability, CapabilityError, CapabilityTable, Generation, ObjectRef, Rights,
/// };
///
/// // A stand-in kernel object: a counted reference to a shared generation.
/// #[derive(Clone)]
/// struct Endpoint(Rc<Cell<Generation>>);
///
/// impl ObjectRef for Endpoint {
///     fn current_generation(&self) -> Generation {
///         self.0.get()
///     }
/// }
///
/// let generation = Rc::new(Cell::new(Generation::FIRST));
/// let mut table: CapabilityTable<Endpoint, 16> = CapabilityTable::new();
///
/// // Mint at the object's current generation; the holder gets back a handle.
/// let parent = table
///     .insert(Capability::mint(Endpoint(Rc::clone(&generation)), Rights::ALL))
///     .expect("the table is empty");
///
/// // Derivation attenuates: the child holds a subset, never more (O-2).
/// let child = table
///     .derive(parent, Rights::DUPLICATE.union(Rights::READ))
///     .expect("a subset of ALL");
/// assert_eq!(
///     table.derive(child, Rights::WRITE),
///     Err(CapabilityError::RightsNotSubset),
/// );
///
/// // Destroying the object bumps its generation: every capability to it,
/// // parent and child alike, is inert at its next resolve (O-3).
/// generation.set(Generation::FIRST.next().expect("generation 2 exists"));
/// assert_eq!(table.resolve(parent).err(), Some(CapabilityError::StaleGeneration));
/// assert_eq!(table.resolve(child).err(), Some(CapabilityError::StaleGeneration));
/// ```
pub struct CapabilityTable<O, const N: usize> {
    slots: [Slot<O>; N],
    /// Head of the free list threaded through recycled slots.
    free_head: Option<u32>,
    /// The high-water mark: the first never-used slot. Slots at or above it
    /// are pristine and reached here, not through the free list.
    next_unused: usize,
    /// The number of occupied slots.
    live: usize,
}

impl<O, const N: usize> CapabilityTable<O, N> {
    /// The number of slots, fixed at compile time.
    pub const CAPACITY: usize = N;

    /// An empty table: every slot pristine at [`Generation::FIRST`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [const {
                Slot {
                    generation: Generation::FIRST,
                    state: SlotState::Free { next_free: None },
                }
            }; N],
            free_head: None,
            next_unused: 0,
            live: 0,
        }
    }

    /// The number of capabilities currently held.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether the table holds no capabilities at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Store `capability`, minting the handle that names it.
    ///
    /// Prefers a recycled slot (most recently freed first) and opens a
    /// never-used one otherwise.
    ///
    /// # Errors
    ///
    /// On a full table the capability is handed **back** beside
    /// [`CapabilityError::TableFull`] rather than dropped: destroying
    /// in-flight authority because the receiver had no room would turn a
    /// resource limit into silent revocation. The caller — ultimately the IPC
    /// path — decides what a failed delivery means.
    pub fn insert(
        &mut self,
        capability: Capability<O>,
    ) -> Result<Handle, (CapabilityError, Capability<O>)> {
        let index = if let Some(recycled) = self.take_recycled() {
            recycled
        } else if self.next_unused < N {
            match u32::try_from(self.next_unused) {
                Ok(index) => {
                    self.next_unused += 1;
                    index
                }
                // A handle is deliberately small: slot numbers beyond
                // `u32::MAX` are unmintable, so a table that large exhausts
                // at the handle-width boundary. Fail closed as capacity.
                Err(_) => return Err((CapabilityError::TableFull, capability)),
            }
        } else {
            return Err((CapabilityError::TableFull, capability));
        };

        let Ok(slot_index) = usize::try_from(index) else {
            // Unreachable on any supported platform (`u32` fits in `usize`);
            // fail closed rather than panic.
            return Err((CapabilityError::TableFull, capability));
        };
        match self.slots.get_mut(slot_index) {
            Some(slot) if matches!(slot.state, SlotState::Free { .. }) => {
                slot.state = SlotState::Occupied(capability);
                // Occupancy is bounded by N, so this cannot overflow.
                self.live += 1;
                Ok(Handle::new(index, slot.generation))
            }
            // Unreachable: both index sources yield an in-bounds, vacant
            // slot. Refuse to overwrite regardless — silently dropping a live
            // occupant would be silent revocation — and fail closed.
            _ => Err((CapabilityError::TableFull, capability)),
        }
    }

    /// Take the capability `handle` names out of the table, by value.
    ///
    /// This is close and transfer-out in one operation: the caller receives
    /// the owned capability and the slot is vacated, its generation bumped so
    /// the handle — and any copy of it userspace kept — is dead from this
    /// instant, not merely from the slot's next reuse. Dropping the returned
    /// value is a close; moving it into another table is a transfer.
    ///
    /// Deliberately object-blind: a capability whose object is long destroyed
    /// can still be removed, because cleanup must always be possible. Whether
    /// the removal *means* anything — a transfer wants `TRANSFER` rights and
    /// a live object — is the invocation path's question, answered against
    /// [`resolve`](Self::resolve) before anything moves.
    ///
    /// # Errors
    ///
    /// - [`CapabilityError::OutOfBounds`] — the index lies outside the table.
    /// - [`CapabilityError::Empty`] — the slot holds nothing.
    /// - [`CapabilityError::StaleGeneration`] — the slot is occupied at a
    ///   different generation than the handle's, or is retired.
    pub fn remove(&mut self, handle: Handle) -> Result<Capability<O>, CapabilityError> {
        let slot_index =
            usize::try_from(handle.index()).map_err(|_| CapabilityError::OutOfBounds)?;
        let slot = self
            .slots
            .get_mut(slot_index)
            .ok_or(CapabilityError::OutOfBounds)?;

        // Take the state out to gain ownership of what it holds. Every path
        // below installs the slot's true successor state; `Retired` stands in
        // meanwhile so that no path can leave the slot claiming a capability
        // it no longer holds.
        let state = mem::replace(&mut slot.state, SlotState::Retired);
        match state {
            SlotState::Occupied(capability) if slot.generation == handle.generation() => {
                // If the generation cannot advance the slot stays `Retired` —
                // never freed, never reused — rather than wrapping to a value
                // a stale handle could match.
                if let Some(next) = slot.generation.next() {
                    slot.generation = next;
                    slot.state = SlotState::Free {
                        next_free: self.free_head,
                    };
                    self.free_head = Some(handle.index());
                }
                // An occupied slot implies at least one live capability, so
                // this cannot underflow.
                self.live -= 1;
                Ok(capability)
            }
            other => {
                let error = match &other {
                    SlotState::Free { .. } => CapabilityError::Empty,
                    // Occupied at some other generation, or retired: either
                    // way the handle is stale.
                    SlotState::Occupied(_) | SlotState::Retired => CapabilityError::StaleGeneration,
                };
                slot.state = other;
                Err(error)
            }
        }
    }

    /// Pop the most recently freed slot off the free list, if any.
    fn take_recycled(&mut self) -> Option<u32> {
        let index = self.free_head?;
        let slot_index = usize::try_from(index).ok()?;
        let slot = self.slots.get(slot_index)?;
        if let SlotState::Free { next_free } = &slot.state {
            self.free_head = *next_free;
            Some(index)
        } else {
            // A free-list entry that is not free would be a logic error in
            // this module; fail closed by offering no recycled slot rather
            // than panicking. The never-used region still serves inserts.
            None
        }
    }
}

impl<O: ObjectRef, const N: usize> CapabilityTable<O, N> {
    /// Resolve `handle` to the capability it names — the single gate through
    /// which a handle is exercised against its object, re-checked in full on
    /// every call. ([`remove`](Self::remove) relocates authority for transfer
    /// or close, after the same stale-handle check; but every *exercise* of
    /// that authority funnels through here.)
    ///
    /// Two of the RFC-0003 amendment's load-bearing invariants live on this
    /// method. A successful resolve yields a borrow, never a copy:
    /// capabilities are not cached outside the table, which is why a
    /// generation bump is a complete revocation. And the caller must hold
    /// that borrow across its whole check→act window: the borrow is what
    /// keeps revocation out until the authorised action completes, so acting
    /// after letting it go — say, by cloning the object reference out of the
    /// capability and dropping the borrow — would re-open the race with
    /// revocation that the generation check just closed. Today the borrow
    /// checker enforces this on a single thread; any future multi-core
    /// synchronisation story (RFC-0003 §14) must preserve exactly this
    /// property, as a correctness dependency of the generation scheme rather
    /// than a later addition.
    ///
    /// # Errors
    ///
    /// Checked in this order, each failing closed:
    ///
    /// - [`CapabilityError::OutOfBounds`] — the index lies outside the table.
    /// - [`CapabilityError::Empty`] — the slot holds nothing. (A freed slot's
    ///   generation was already bumped, so a closed handle also lands here
    ///   until the slot is reused; the state check simply answers first.)
    /// - [`CapabilityError::StaleGeneration`] — the slot is retired, or
    ///   occupied at a different generation than the handle's (a stale
    ///   handle, O-1); or the capability's minted generation no longer
    ///   matches the object's — the object was destroyed, and the capability
    ///   is inert (the destruction half of O-3).
    pub fn resolve(&self, handle: Handle) -> Result<&Capability<O>, CapabilityError> {
        let slot_index =
            usize::try_from(handle.index()).map_err(|_| CapabilityError::OutOfBounds)?;
        let slot = self
            .slots
            .get(slot_index)
            .ok_or(CapabilityError::OutOfBounds)?;
        match &slot.state {
            SlotState::Free { .. } => Err(CapabilityError::Empty),
            SlotState::Retired => Err(CapabilityError::StaleGeneration),
            SlotState::Occupied(capability) => {
                if slot.generation != handle.generation() {
                    return Err(CapabilityError::StaleGeneration);
                }
                if capability.generation() != capability.object().current_generation() {
                    return Err(CapabilityError::StaleGeneration);
                }
                Ok(capability)
            }
        }
    }

    /// Mint an attenuated sibling: resolve `parent`, run the derivation
    /// checks ([`Capability::derive`] — live, `DUPLICATE`, subset-only), and
    /// store the child in this same table.
    ///
    /// # Errors
    ///
    /// Everything [`resolve`](Self::resolve) and [`Capability::derive`] can
    /// return, plus [`CapabilityError::TableFull`] if there is no slot for
    /// the child. On any failure the table is unchanged and the parent
    /// untouched; a child that could not be stored is dropped, which loses
    /// nothing — it was never granted.
    pub fn derive(&mut self, parent: Handle, requested: Rights) -> Result<Handle, CapabilityError> {
        let child = self.resolve(parent)?.derive(requested)?;
        self.insert(child).map_err(|(error, _never_granted)| error)
    }
}

impl<O, const N: usize> Default for CapabilityTable<O, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O, const N: usize> fmt::Debug for CapabilityTable<O, N> {
    /// A summary, not a listing: what the table's capabilities reference is
    /// not this crate's to print.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityTable")
            .field("live", &self.live)
            .field("capacity", &N)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::test_support::TestObject;
    use crate::{Capability, CapabilityError, CapabilityTable, Generation, Handle, Rights};

    /// Mint a full-rights capability to a fresh test object.
    fn full(id: u32) -> Capability<TestObject> {
        Capability::mint(TestObject::new(id), Rights::ALL)
    }

    #[test]
    fn insert_then_resolve_returns_the_capability() {
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let handle = table.insert(full(1)).expect("the table is empty");
        let capability = table.resolve(handle).expect("a live handle resolves");
        assert_eq!(capability.rights(), Rights::ALL);
        assert_eq!(capability.object().id(), 1);
    }

    #[test]
    fn resolve_fails_closed_on_every_shape_of_bad_handle() {
        let mut table: CapabilityTable<TestObject, 2> = CapabilityTable::new();
        let real = table.insert(full(1)).expect("the table is empty");
        // Beyond the table: out of bounds.
        assert_eq!(
            table.resolve(Handle::new(2, Generation::FIRST)).err(),
            Some(CapabilityError::OutOfBounds)
        );
        assert_eq!(
            table
                .resolve(Handle::new(u32::MAX, Generation::FIRST))
                .err(),
            Some(CapabilityError::OutOfBounds)
        );
        // In bounds, never allocated: empty.
        assert_eq!(
            table.resolve(Handle::new(1, Generation::FIRST)).err(),
            Some(CapabilityError::Empty)
        );
        // The right slot at the wrong generation: stale.
        let wrong_generation = Handle::new(
            real.index(),
            real.generation().next().expect("a next generation exists"),
        );
        assert_eq!(
            table.resolve(wrong_generation).err(),
            Some(CapabilityError::StaleGeneration)
        );
    }

    #[test]
    fn only_the_exact_minted_handle_resolves() {
        // A small exhaustive sweep over forged (index, generation) pairs:
        // exactly one handle in the space resolves, and it is the minted one
        // (O-1 exercised as a search, not an anecdote).
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let real = table.insert(full(1)).expect("the table is empty");
        let mut generation = Generation::FIRST;
        for _ in 0..4 {
            for index in 0..6 {
                let forged = Handle::new(index, generation);
                if forged == real {
                    assert!(table.resolve(forged).is_ok());
                } else {
                    assert!(
                        table.resolve(forged).is_err(),
                        "forged handle {forged:?} must not resolve"
                    );
                    assert!(
                        table.remove(forged).is_err(),
                        "forged handle {forged:?} must not remove"
                    );
                }
            }
            generation = generation.next().expect("small generations exist");
        }
    }

    #[test]
    fn remove_returns_the_owned_capability_and_the_handle_dies_immediately() {
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let handle = table.insert(full(3)).expect("the table is empty");
        let capability = table.remove(handle).expect("a live handle removes");
        assert_eq!(capability.object().id(), 3);
        assert_eq!(table.len(), 0);
        // Dead at the instant of closing — before any reuse. The slot is
        // empty *and* its generation already bumped; the state check answers
        // first, hence `Empty` (the precedence is deliberate and documented
        // on `resolve`).
        assert_eq!(table.resolve(handle).err(), Some(CapabilityError::Empty));
        // A second remove finds nothing either.
        assert_eq!(table.remove(handle).err(), Some(CapabilityError::Empty));
    }

    #[test]
    fn a_reused_slot_never_honours_the_previous_occupants_handle() {
        // The ABA test: close, reuse, and the stale handle must fail rather
        // than alias the new occupant (RFC-0003 §8).
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let first = table.insert(full(1)).expect("the table is empty");
        drop(table.remove(first).expect("a live handle removes"));
        let second = table.insert(full(2)).expect("a recycled slot is free");
        // Same slot, new generation.
        assert_eq!(second.index(), first.index());
        assert_ne!(second.generation(), first.generation());
        // The stale handle fails closed; the new one names the new occupant.
        assert_eq!(
            table.resolve(first).err(),
            Some(CapabilityError::StaleGeneration)
        );
        assert_eq!(
            table
                .resolve(second)
                .expect("a live handle resolves")
                .object()
                .id(),
            2
        );
    }

    #[test]
    fn a_full_table_hands_the_capability_back_rather_than_destroying_it() {
        let mut table: CapabilityTable<TestObject, 2> = CapabilityTable::new();
        let _first = table.insert(full(1)).expect("slot 0 is free");
        let second = table.insert(full(2)).expect("slot 1 is free");
        let (error, returned) = table
            .insert(Capability::mint(TestObject::new(3), Rights::READ))
            .expect_err("the table is full");
        assert_eq!(error, CapabilityError::TableFull);
        // The capability comes back intact, not destroyed.
        assert_eq!(returned.object().id(), 3);
        assert_eq!(returned.rights(), Rights::READ);
        // Freeing a slot restores capacity, and the freed slot is the one
        // recycled.
        drop(table.remove(second).expect("a live handle removes"));
        let again = table.insert(returned).expect("a slot was recycled");
        assert_eq!(again.index(), second.index());
        assert_ne!(again.generation(), second.generation());
    }

    #[test]
    fn derive_mints_an_attenuated_sibling_in_the_same_table() {
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let parent = table.insert(full(5)).expect("the table is empty");
        let child = table
            .derive(parent, Rights::READ)
            .expect("READ is a subset of ALL");
        assert_ne!(child.index(), parent.index());
        let resolved = table.resolve(child).expect("the child is live");
        assert_eq!(resolved.rights(), Rights::READ);
        assert_eq!(resolved.object().id(), 5);
        // The parent is untouched by the derivation.
        assert_eq!(
            table.resolve(parent).expect("the parent is live").rights(),
            Rights::ALL
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn derive_refuses_widening_and_underprivileged_parents() {
        let mut table: CapabilityTable<TestObject, 8> = CapabilityTable::new();
        let limited = table
            .insert(Capability::mint(
                TestObject::new(1),
                Rights::DUPLICATE.union(Rights::READ),
            ))
            .expect("the table is empty");
        // Widening: the child asks for a right the parent lacks (O-2).
        assert_eq!(
            table.derive(limited, Rights::WRITE),
            Err(CapabilityError::RightsNotSubset)
        );
        assert_eq!(
            table.derive(limited, Rights::READ.union(Rights::WRITE)),
            Err(CapabilityError::RightsNotSubset)
        );
        // A leaf — no DUPLICATE — yields nothing, not even a strict subset.
        let leaf = table
            .derive(limited, Rights::READ)
            .expect("READ is a subset");
        assert_eq!(
            table.derive(leaf, Rights::READ),
            Err(CapabilityError::NotDuplicable)
        );
        assert_eq!(
            table.derive(leaf, Rights::NONE),
            Err(CapabilityError::NotDuplicable)
        );
        // The failures minted nothing.
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn derive_into_a_full_table_fails_and_the_parent_survives() {
        let mut table: CapabilityTable<TestObject, 1> = CapabilityTable::new();
        let parent = table.insert(full(1)).expect("the table is empty");
        assert_eq!(
            table.derive(parent, Rights::READ),
            Err(CapabilityError::TableFull)
        );
        assert_eq!(
            table
                .resolve(parent)
                .expect("the parent is still live")
                .rights(),
            Rights::ALL
        );
    }

    #[test]
    fn destroying_the_object_makes_every_capability_to_it_inert() {
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let object = TestObject::new(3);
        let parent = table
            .insert(Capability::mint(object.clone(), Rights::ALL))
            .expect("the table is empty");
        let child = table
            .derive(parent, Rights::READ)
            .expect("READ is a subset of ALL");
        object.destroy();
        // No list of holders required: parent and child alike fail at their
        // next resolve (the destruction half of O-3).
        assert_eq!(
            table.resolve(parent).err(),
            Some(CapabilityError::StaleGeneration)
        );
        assert_eq!(
            table.resolve(child).err(),
            Some(CapabilityError::StaleGeneration)
        );
        // Slot-level cleanup still works — a corpse can be removed...
        let corpse = table
            .remove(parent)
            .expect("slot bookkeeping outlives the object");
        assert_eq!(corpse.rights(), Rights::ALL);
        // ...but deriving from one fails closed.
        assert_eq!(
            table.derive(child, Rights::READ),
            Err(CapabilityError::StaleGeneration)
        );
    }

    #[test]
    fn transfer_moves_a_capability_between_tables() {
        let mut sender: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let mut receiver: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let outgoing = sender.insert(full(9)).expect("the sender table is empty");
        // The move (RFC-0003 §6): out of one table, into the other. At no
        // instant is the capability in both — `remove` returns it by value,
        // `insert` consumes it.
        let in_flight = sender.remove(outgoing).expect("a live handle removes");
        let delivered = receiver.insert(in_flight).expect("the receiver has room");
        assert_eq!(sender.len(), 0);
        assert_eq!(receiver.len(), 1);
        assert!(sender.resolve(outgoing).is_err());
        assert_eq!(
            receiver
                .resolve(delivered)
                .expect("the delivered handle resolves")
                .object()
                .id(),
            9
        );
    }

    #[test]
    fn generation_exhaustion_retires_the_slot_rather_than_wrapping() {
        let mut table: CapabilityTable<TestObject, 1> = CapabilityTable::new();
        // Age slot 0 to the last representable generation, as if it had been
        // recycled 2^64 - 1 times. Tests may reach into the private slot
        // array; nothing outside this module can.
        table.slots.get_mut(0).expect("slot 0 exists").generation = Generation::last();
        let handle = table.insert(full(7)).expect("the table is empty");
        assert_eq!(handle.generation(), Generation::last());
        assert!(table.resolve(handle).is_ok());
        // Closing cannot bump the generation, so the slot retires instead of
        // wrapping to a value some stale handle could match.
        drop(table.remove(handle).expect("a live handle removes"));
        assert_eq!(table.len(), 0);
        // The handle is dead...
        assert_eq!(
            table.resolve(handle).err(),
            Some(CapabilityError::StaleGeneration)
        );
        // ...and the slot is never offered again: with its only slot retired,
        // the table is permanently full. Capacity is the price of failing
        // closed.
        assert!(matches!(
            table.insert(full(8)),
            Err((CapabilityError::TableFull, _))
        ));
    }

    #[test]
    fn bookkeeping_reports_occupancy() {
        let mut table: CapabilityTable<TestObject, 3> = CapabilityTable::default();
        assert!(table.is_empty());
        assert_eq!(CapabilityTable::<TestObject, 3>::CAPACITY, 3);
        let handle = table.insert(full(1)).expect("the table is empty");
        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());
        drop(table.remove(handle).expect("a live handle removes"));
        assert!(table.is_empty());
        // Debug summarises without printing objects.
        let text = format!("{table:?}");
        assert!(text.contains("live"));
        assert!(!text.contains("TestObject"));
    }

    #[test]
    fn remove_honours_only_the_current_occupants_handle() {
        // The ABA case at the remove gate (RFC-0003 §8): a dangling handle to
        // a reused slot must not steal or destroy the slot's current
        // occupant.
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let first = table.insert(full(1)).expect("the table is empty");
        drop(table.remove(first).expect("a live handle removes"));
        let second = table.insert(full(2)).expect("the slot recycles");
        assert_eq!(second.index(), first.index());
        // The stale handle removes nothing...
        assert_eq!(
            table.remove(first).err(),
            Some(CapabilityError::StaleGeneration)
        );
        // ...and disturbed nothing: the occupant is still there, still
        // resolves, and still answers to its own handle.
        assert_eq!(table.len(), 1);
        assert_eq!(
            table
                .resolve(second)
                .expect("the occupant survives the failed remove")
                .object()
                .id(),
            2
        );
        let occupant = table.remove(second).expect("the real handle still removes");
        assert_eq!(occupant.object().id(), 2);
    }

    #[test]
    fn remove_fails_closed_on_out_of_bounds_handles() {
        let mut table: CapabilityTable<TestObject, 2> = CapabilityTable::new();
        let _live = table.insert(full(1)).expect("the table is empty");
        // The first index past the table, and the far end of handle space.
        assert_eq!(
            table.remove(Handle::new(2, Generation::FIRST)).err(),
            Some(CapabilityError::OutOfBounds)
        );
        assert_eq!(
            table.remove(Handle::new(u32::MAX, Generation::FIRST)).err(),
            Some(CapabilityError::OutOfBounds)
        );
        // The failures removed nothing.
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn a_retired_slot_coexists_with_usable_slots() {
        // Retirement in company: the N=1 exhaustion test proves the fail-closed
        // boundary, this one proves the price is one slot, not the table. The
        // ordering matters — a slot is freed normally *before* the retirement,
        // so the free list is shown to survive it.
        let mut table: CapabilityTable<TestObject, 3> = CapabilityTable::new();
        // Age slot 1 to the boundary before it is ever offered.
        table.slots.get_mut(1).expect("slot 1 exists").generation = Generation::last();
        let first = table.insert(full(0)).expect("slot 0 is free");
        let aged = table.insert(full(1)).expect("slot 1 is free");
        let neighbour = table.insert(full(2)).expect("slot 2 is free");
        // A normally freed slot joins the free list first...
        drop(table.remove(first).expect("a live handle removes"));
        // ...then closing the aged slot retires it: no bump is possible.
        drop(table.remove(aged).expect("a live handle removes"));
        // Removing through the retired slot's handle is stale, not empty —
        // the `Retired` arm of remove's error match.
        assert_eq!(
            table.remove(aged).err(),
            Some(CapabilityError::StaleGeneration)
        );
        // The neighbour is untouched by the retirement.
        assert_eq!(
            table
                .resolve(neighbour)
                .expect("the neighbour is live")
                .object()
                .id(),
            2
        );
        // The free list survived the retirement: slot 0 is still reachable.
        let reused = table.insert(full(3)).expect("slot 0 was recycled");
        assert_eq!(reused.index(), first.index());
        // And the table is permanently one slot smaller: full at N - 1.
        assert!(matches!(
            table.insert(full(4)),
            Err((CapabilityError::TableFull, _))
        ));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn recycled_slots_are_reused_most_recently_freed_first() {
        // The free list is LIFO (documented on `insert`): the most recently
        // freed slot is handed out first, and the chain is followed to its
        // end before a pristine slot is opened.
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let a = table.insert(full(1)).expect("slot 0 is free");
        let _b = table.insert(full(2)).expect("slot 1 is free");
        let c = table.insert(full(3)).expect("slot 2 is free");
        drop(table.remove(a).expect("a live handle removes"));
        drop(table.remove(c).expect("a live handle removes"));
        // The free list is now c -> a -> None: c was freed last, so it is
        // reused first, then a, and only then the pristine slot 3.
        let first = table.insert(full(4)).expect("two slots are recycled");
        assert_eq!(first.index(), c.index());
        let second = table.insert(full(5)).expect("one slot is recycled");
        assert_eq!(second.index(), a.index());
        let third = table.insert(full(6)).expect("slot 3 is pristine");
        assert_eq!(third.index(), 3);
    }

    #[test]
    fn a_removed_parent_leaves_its_derived_sibling_untouched() {
        // Flat table, no parent link: closing the parent is not revocation —
        // the child is a sibling, not a dependant. Selective revocation is
        // RFC-0003a's question, and this test pins that remove does not
        // pre-empt it.
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let parent = table.insert(full(1)).expect("the table is empty");
        let child = table
            .derive(parent, Rights::DUPLICATE.union(Rights::READ))
            .expect("a subset of ALL");
        drop(table.remove(parent).expect("a live handle removes"));
        let resolved = table.resolve(child).expect("the child outlives the parent");
        assert_eq!(resolved.rights(), Rights::DUPLICATE.union(Rights::READ));
        assert_eq!(resolved.object().id(), 1);
        // Reusing the parent's slot changes nothing for the child, and the
        // old parent handle stays dead.
        let newcomer = table.insert(full(2)).expect("the parent's slot recycled");
        assert_eq!(newcomer.index(), parent.index());
        assert!(table.resolve(parent).is_err());
        assert_eq!(
            table
                .resolve(child)
                .expect("the child is still live")
                .object()
                .id(),
            1
        );
        // The chain deepens monotonically (RFC-0003 §5): a DUPLICATE-carrying
        // child yields a grandchild, narrowed at each level.
        let grandchild = table
            .derive(child, Rights::READ)
            .expect("READ is a subset of DUPLICATE|READ");
        assert_eq!(
            table
                .resolve(grandchild)
                .expect("the grandchild is live")
                .rights(),
            Rights::READ
        );
    }

    #[test]
    fn a_corrupted_free_list_head_fails_closed_rather_than_panicking() {
        // The defensive arm in `take_recycled`: a free-list head pointing at
        // a non-free slot must offer nothing — not panic, not hand out the
        // occupied slot. Unreachable through the public API; tests may
        // corrupt the private state directly, as the retirement tests age
        // slots directly.
        let mut table: CapabilityTable<TestObject, 4> = CapabilityTable::new();
        let first = table.insert(full(1)).expect("the table is empty");
        table.free_head = Some(first.index());
        let second = table
            .insert(full(2))
            .expect("the pristine region still serves inserts");
        assert_ne!(second.index(), first.index());
        assert_eq!(
            table
                .resolve(first)
                .expect("the occupant is untouched")
                .object()
                .id(),
            1
        );
        assert_eq!(
            table
                .resolve(second)
                .expect("the newcomer resolves")
                .object()
                .id(),
            2
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn insert_refuses_to_overwrite_an_occupied_slot() {
        // The defensive arm in `insert`: even with the high-water mark
        // rewound onto an occupied slot (unreachable without a logic error),
        // insert hands the capability back rather than silently destroying
        // the occupant — silent revocation being the failure mode the error
        // shape exists to prevent.
        let mut table: CapabilityTable<TestObject, 1> = CapabilityTable::new();
        let first = table.insert(full(1)).expect("the table is empty");
        table.next_unused = 0;
        let (error, returned) = table.insert(full(2)).expect_err("the slot is occupied");
        assert_eq!(error, CapabilityError::TableFull);
        assert_eq!(returned.object().id(), 2);
        assert_eq!(
            table
                .resolve(first)
                .expect("the occupant survives")
                .object()
                .id(),
            1
        );
        assert_eq!(table.len(), 1);
    }

    /// A deterministic xorshift64 generator — no dependencies, fixed seed,
    /// reproducible failures.
    struct XorShift(u64);

    impl XorShift {
        fn draw(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// A uniform-enough index below `upper`, which must be non-zero.
    fn pick(rng: &mut XorShift, upper: usize) -> usize {
        let bound = u64::try_from(upper).expect("usize fits in u64 on supported platforms");
        usize::try_from(rng.draw() % bound).expect("a value below `upper` fits in usize")
    }

    /// A rights set built from five random bits — every combination of the
    /// defined rights is reachable, so derivation requests span the space.
    fn random_rights(rng: &mut XorShift) -> Rights {
        let bits = rng.draw();
        let mut rights = Rights::NONE;
        for (position, right) in [
            Rights::DUPLICATE,
            Rights::TRANSFER,
            Rights::READ,
            Rights::WRITE,
            Rights::REVOKE,
        ]
        .into_iter()
        .enumerate()
        {
            if bits & (1_u64 << position) != 0 {
                rights = rights.union(right);
            }
        }
        rights
    }

    /// The churn table's capacity — small enough that the slots run full and
    /// recycle constantly under thousands of operations.
    const CHURN_CAPACITY: usize = 16;

    /// The churn property test's shadow model: the table under test plus the
    /// model's view of it, one method per random operation — extracted from
    /// the test so that each step stays short enough to review on its own.
    struct Churn {
        table: CapabilityTable<TestObject, CHURN_CAPACITY>,
        rng: XorShift,
        /// Live entries: handle, object id, a clone of the object reference
        /// (so the model can destroy it), and the rights minted or derived.
        live: Vec<(Handle, u32, TestObject, Rights)>,
        /// Stale entries: still occupying a slot, but their object destroyed.
        stale: Vec<(Handle, Rights)>,
        /// Handles whose slots were vacated: they must never work again.
        dead: Vec<Handle>,
        next_id: u32,
    }

    impl Churn {
        fn new() -> Self {
            Self {
                table: CapabilityTable::new(),
                rng: XorShift(0x5e70_11f0_57ab_1e5d),
                live: Vec::new(),
                stale: Vec::new(),
                dead: Vec::new(),
                next_id: 0,
            }
        }

        /// Insert a fresh full-rights object.
        fn insert_fresh(&mut self) {
            let object = TestObject::new(self.next_id);
            match self
                .table
                .insert(Capability::mint(object.clone(), Rights::ALL))
            {
                Ok(handle) => {
                    self.live.push((handle, self.next_id, object, Rights::ALL));
                    self.next_id += 1;
                }
                Err((CapabilityError::TableFull, _)) => {
                    assert_eq!(
                        self.live.len() + self.stale.len(),
                        CHURN_CAPACITY,
                        "full means exactly N occupied"
                    );
                }
                Err((error, _)) => panic!("unexpected insert failure: {error:?}"),
            }
        }

        /// Derive a random rights request from a random live parent, checking
        /// the verdict against the recorded parent rights.
        fn derive_random(&mut self) {
            if self.live.is_empty() {
                return;
            }
            let (parent, id, object, parent_rights) = self
                .live
                .get(pick(&mut self.rng, self.live.len()))
                .expect("the index is bounded")
                .clone();
            let requested = random_rights(&mut self.rng);
            match self.table.derive(parent, requested) {
                Ok(child) => {
                    assert!(parent_rights.contains(Rights::DUPLICATE));
                    assert!(requested.is_subset_of(parent_rights));
                    assert_eq!(
                        self.table
                            .resolve(child)
                            .expect("the child is live")
                            .rights(),
                        requested
                    );
                    self.live.push((child, id, object, requested));
                }
                Err(CapabilityError::NotDuplicable) => {
                    assert!(!parent_rights.contains(Rights::DUPLICATE));
                }
                Err(CapabilityError::RightsNotSubset) => {
                    assert!(parent_rights.contains(Rights::DUPLICATE));
                    assert!(!requested.is_subset_of(parent_rights));
                }
                Err(CapabilityError::TableFull) => {
                    assert!(parent_rights.contains(Rights::DUPLICATE));
                    assert!(requested.is_subset_of(parent_rights));
                    assert_eq!(self.live.len() + self.stale.len(), CHURN_CAPACITY);
                }
                Err(error) => panic!("unexpected derive failure: {error:?}"),
            }
        }

        /// Remove a random live capability.
        fn remove_live(&mut self) {
            if self.live.is_empty() {
                return;
            }
            let victim = pick(&mut self.rng, self.live.len());
            let (handle, id, _object, rights) = self.live.swap_remove(victim);
            let capability = self.table.remove(handle).expect("a live handle removes");
            assert_eq!(capability.object().id(), id);
            assert_eq!(capability.rights(), rights);
            self.dead.push(handle);
        }

        /// Destroy a random live object: every capability sharing it — the
        /// entry itself and any derived from it — turns stale in place, still
        /// occupying its slot (the destruction half of O-3 needs no list of
        /// holders).
        fn destroy_object(&mut self) {
            if self.live.is_empty() {
                return;
            }
            let victim_id = self
                .live
                .get(pick(&mut self.rng, self.live.len()))
                .expect("the index is bounded")
                .1;
            let mut index = 0;
            let mut destroyed = false;
            while index < self.live.len() {
                if self.live.get(index).map(|entry| entry.1) == Some(victim_id) {
                    let (handle, _id, object, rights) = self.live.swap_remove(index);
                    if !destroyed {
                        object.destroy();
                        destroyed = true;
                    }
                    self.stale.push((handle, rights));
                } else {
                    index += 1;
                }
            }
            assert!(destroyed, "the victim was in the live set");
        }

        /// Cleanup: a stale entry is inert but must still remove
        /// (object-blind), freeing its slot; a dead handle must not remove
        /// anything, and its failed remove must disturb no one.
        fn clean_up(&mut self) {
            if !self.stale.is_empty() {
                let victim = pick(&mut self.rng, self.stale.len());
                let (handle, rights) = self.stale.swap_remove(victim);
                assert_eq!(
                    self.table.resolve(handle).err(),
                    Some(CapabilityError::StaleGeneration)
                );
                let corpse = self
                    .table
                    .remove(handle)
                    .expect("object-blind cleanup always works");
                assert_eq!(corpse.rights(), rights);
                self.dead.push(handle);
            }
            if !self.dead.is_empty() {
                let handle = *self
                    .dead
                    .get(pick(&mut self.rng, self.dead.len()))
                    .expect("the index is bounded");
                assert!(
                    self.table.remove(handle).is_err(),
                    "a dead handle must not remove anything"
                );
                if let Some((survivor, id, _object, _rights)) = self.live.first() {
                    assert_eq!(
                        self.table
                            .resolve(*survivor)
                            .expect("the failed remove disturbed nobody")
                            .object()
                            .id(),
                        *id
                    );
                }
            }
        }

        /// Resolve one of each: live, stale, dead.
        fn resolve_each_kind(&mut self) {
            if !self.live.is_empty() {
                let (handle, id, _object, rights) = self
                    .live
                    .get(pick(&mut self.rng, self.live.len()))
                    .expect("the index is bounded")
                    .clone();
                let capability = self.table.resolve(handle).expect("a live handle resolves");
                assert_eq!(capability.object().id(), id);
                assert_eq!(capability.rights(), rights);
            }
            if !self.stale.is_empty() {
                let (handle, _rights) = *self
                    .stale
                    .get(pick(&mut self.rng, self.stale.len()))
                    .expect("the index is bounded");
                assert_eq!(
                    self.table.resolve(handle).err(),
                    Some(CapabilityError::StaleGeneration),
                    "a destroyed object's capability must be inert"
                );
            }
            if !self.dead.is_empty() {
                let handle = *self
                    .dead
                    .get(pick(&mut self.rng, self.dead.len()))
                    .expect("the index is bounded");
                assert!(
                    self.table.resolve(handle).is_err(),
                    "a dead handle must never resolve"
                );
            }
        }
    }

    #[test]
    fn churn_never_resolves_a_dead_handle() {
        // Property test against a shadow model: thousands of random inserts,
        // subset-random derivations, object destructions, removals and
        // resolves, checking as each operation is drawn that live handles
        // resolve to the right object with the right rights, that handles to
        // destroyed objects are inert but still removable (object-blind
        // cleanup), and that closed handles never resolve and never remove —
        // and after every step that occupancy agrees with the model. All
        // three generation-checked behaviours —
        // slot reuse (O-1), derivation (O-2) and destruction (O-3) — at
        // scale rather than as anecdotes.
        let mut churn = Churn::new();
        for _ in 0..8192 {
            match churn.rng.draw() % 8 {
                // Weighted towards growth so the table regularly runs full.
                0..=2 => churn.insert_fresh(),
                3 => churn.derive_random(),
                4 => churn.remove_live(),
                5 => churn.destroy_object(),
                6 => churn.clean_up(),
                _ => churn.resolve_each_kind(),
            }
            assert_eq!(churn.table.len(), churn.live.len() + churn.stale.len());
        }

        // And at the end: every closed handle is still dead, every survivor
        // of a destroyed object still inert.
        for handle in churn.dead {
            assert!(churn.table.resolve(handle).is_err());
        }
        for (handle, _rights) in churn.stale {
            assert_eq!(
                churn.table.resolve(handle).err(),
                Some(CapabilityError::StaleGeneration)
            );
        }
    }
}
