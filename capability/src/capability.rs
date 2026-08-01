// SPDX-License-Identifier: GPL-3.0-or-later

//! The capability itself — an owned, unclonable grant of authority.

use crate::{CapabilityError, Generation, ObjectRef, Rights};
use core::fmt;

/// A capability: a kernel-held reference to a kernel object, together with the
/// rights this reference carries and the object generation it was minted at
/// (RFC-0003 §3).
///
/// The type is **owned and deliberately neither `Clone` nor `Copy`** (§6). A
/// capability is in exactly one table, or in flight in exactly one message —
/// moving it out of one table and into another is a Rust move, and the borrow
/// checker forbids the value existing in two places. The one way to get a
/// second capability to an object is [`derive`](Self::derive): an explicit,
/// rights-checked operation, never an implicit copy.
///
/// Per the RFC-0003 prior-art amendment, that compile-time guarantee covers
/// the kernel's *internal* handling. The userspace-observable cross-process
/// transfer is a runtime table operation; the generation check is what secures
/// it.
///
/// ```compile_fail,E0277
/// // A capability cannot be cloned — duplication is `derive`. The object
/// // type implements `ObjectRef` so this guards the instantiation the
/// // kernel will actually use: any `Clone` impl on `Capability` bounded by
/// // traits an object reference satisfies (`Clone`, `ObjectRef`) would make
/// // this compile and the test fail.
/// use setonix_capability::{Capability, Generation, ObjectRef};
///
/// #[derive(Clone)]
/// struct Object;
/// impl ObjectRef for Object {
///     fn current_generation(&self) -> Generation {
///         Generation::FIRST
///     }
/// }
///
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<Capability<Object>>();
/// ```
#[must_use = "dropping a capability closes it — move it into a table, or drop it deliberately"]
pub struct Capability<O> {
    object: O,
    rights: Rights,
    generation: Generation,
}

impl<O: ObjectRef> Capability<O> {
    /// Mint a capability to `object`, carrying `rights`, at the object's
    /// **current** generation.
    ///
    /// Reading the generation from the object rather than taking it as a
    /// parameter means a mint can never be back-dated: there is no way to
    /// construct a capability at a generation the object no longer occupies.
    /// The complementary discipline is the kernel's — mint only at object
    /// creation, or from a grant path that has established the object is
    /// live, because a mint always grants authority to the object's *current*
    /// incarnation. Everything downstream of the first grant should go
    /// through [`derive`](Self::derive), which refuses a stale parent.
    pub fn mint(object: O, rights: Rights) -> Self {
        let generation = object.current_generation();
        Self {
            object,
            rights,
            generation,
        }
    }

    /// Derive an attenuated child capability — the only duplication there is.
    ///
    /// The child references the same object (cloning the counted reference,
    /// which duplicates no authority), carries exactly `requested`, and is
    /// minted at the same generation as its live parent.
    ///
    /// # Errors
    ///
    /// Checked in this order, each failing closed with `self` untouched:
    ///
    /// - [`CapabilityError::StaleGeneration`] — `self` no longer matches its
    ///   object's generation. A stale capability is inert for *every*
    ///   operation, derivation included, so this is checked before anything
    ///   else.
    /// - [`CapabilityError::NotDuplicable`] — `self` lacks
    ///   [`Rights::DUPLICATE`]: a leaf, from which nothing may be derived
    ///   (RFC-0003 §5).
    /// - [`CapabilityError::RightsNotSubset`] — `requested` asks for a right
    ///   `self` does not hold, refused by [`Rights::diminish`]. O-2: no
    ///   derivation chain ever widens.
    pub fn derive(&self, requested: Rights) -> Result<Self, CapabilityError> {
        if self.object.current_generation() != self.generation {
            return Err(CapabilityError::StaleGeneration);
        }
        if !self.rights.contains(Rights::DUPLICATE) {
            return Err(CapabilityError::NotDuplicable);
        }
        let rights = self
            .rights
            .diminish(requested)
            .ok_or(CapabilityError::RightsNotSubset)?;
        Ok(Self {
            object: self.object.clone(),
            rights,
            generation: self.generation,
        })
    }
}

impl<O> Capability<O> {
    /// The rights this capability carries.
    #[must_use]
    pub const fn rights(&self) -> Rights {
        self.rights
    }

    /// The object generation this capability was minted at. Compared against
    /// the object's current generation on every resolve; destroying the
    /// object bumps its generation and leaves this value behind, which is
    /// what makes the capability inert (the destruction half of O-3).
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The referenced object.
    #[must_use]
    pub const fn object(&self) -> &O {
        &self.object
    }
}

impl<O> fmt::Debug for Capability<O> {
    /// The object reference is deliberately omitted: what a kernel object
    /// looks like inside is not this crate's to print, and diagnostics must
    /// not become a side channel. Omitting it also spares `O` a `Debug`
    /// bound.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Capability")
            .field("rights", &self.rights)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::test_support::TestObject;
    use crate::{Capability, CapabilityError, Generation, Rights};

    #[test]
    fn mint_records_rights_and_the_objects_current_generation() {
        let object = TestObject::new(1);
        // Advance the object to a second incarnation first, to prove the mint
        // reads the current generation rather than assuming the first.
        object.destroy();
        let generation_two = Generation::FIRST.next().expect("generation 2 exists");
        let capability = Capability::mint(object, Rights::READ);
        assert_eq!(capability.rights(), Rights::READ);
        assert_eq!(capability.generation(), generation_two);
        assert_eq!(capability.object().id(), 1);
    }

    #[test]
    fn derive_attenuates_and_references_the_same_object() {
        let parent = Capability::mint(TestObject::new(7), Rights::ALL);
        let child = parent
            .derive(Rights::READ)
            .expect("READ is a subset of ALL");
        assert_eq!(child.rights(), Rights::READ);
        assert_eq!(child.generation(), parent.generation());
        assert_eq!(child.object().id(), 7);
        // The parent is untouched by the derivation.
        assert_eq!(parent.rights(), Rights::ALL);
    }

    #[test]
    fn derive_refuses_widening() {
        let parent = Capability::mint(TestObject::new(1), Rights::DUPLICATE.union(Rights::READ));
        // A right the parent lacks, alone or mixed with rights it holds —
        // either way the request is not a subset (O-2).
        assert_eq!(
            parent.derive(Rights::WRITE).err(),
            Some(CapabilityError::RightsNotSubset)
        );
        assert_eq!(
            parent.derive(Rights::READ.union(Rights::WRITE)).err(),
            Some(CapabilityError::RightsNotSubset)
        );
    }

    #[test]
    fn derive_from_a_leaf_fails_even_for_a_strict_subset() {
        // No DUPLICATE: a leaf. Nothing may be derived from it, not even
        // nothing at all.
        let leaf = Capability::mint(TestObject::new(1), Rights::READ);
        assert_eq!(
            leaf.derive(Rights::READ).err(),
            Some(CapabilityError::NotDuplicable)
        );
        assert_eq!(
            leaf.derive(Rights::NONE).err(),
            Some(CapabilityError::NotDuplicable)
        );
        // A right the leaf also lacks still reports NotDuplicable — the
        // checks run in the documented order, and this is the one input that
        // tells the two rights checks apart.
        assert_eq!(
            leaf.derive(Rights::WRITE).err(),
            Some(CapabilityError::NotDuplicable)
        );
    }

    #[test]
    fn derive_from_a_stale_capability_fails_before_any_rights_check() {
        let object = TestObject::new(1);
        let parent = Capability::mint(object.clone(), Rights::ALL);
        object.destroy();
        // Staleness wins over every other verdict: a dead capability is inert,
        // not merely under-privileged.
        assert_eq!(
            parent.derive(Rights::READ).err(),
            Some(CapabilityError::StaleGeneration)
        );
        // Even a request that would also fail the rights checks reports the
        // staleness, deliberately: the checks run in the documented order.
        let stale_leaf = Capability::mint(object.clone(), Rights::NONE);
        object.destroy();
        assert_eq!(
            stale_leaf.derive(Rights::WRITE).err(),
            Some(CapabilityError::StaleGeneration)
        );
    }

    #[test]
    fn debug_omits_the_object() {
        let capability = Capability::mint(TestObject::new(1), Rights::READ);
        let text = format!("{capability:?}");
        assert!(text.contains("Rights(READ)"));
        assert!(
            !text.contains("TestObject"),
            "the object must not leak through Debug: {text}"
        );
    }
}
