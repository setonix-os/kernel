// SPDX-License-Identifier: GPL-3.0-or-later

//! Capability rights — the attenuable authority a capability carries.

use core::fmt;

/// The set of operations a capability permits on its object.
///
/// A small, closed bitmask. Rights only ever *diminish* along a derivation
/// chain: [`Rights::diminish`] can drop bits, and no operation anywhere adds
/// one. That is RFC-0003 O-2 (non-widenability) made a property of the type
/// rather than a rule a reviewer must remember.
///
/// There is deliberately no constructor from arbitrary bits. A `Rights` is only
/// ever assembled from the named constants (with [`Rights::union`] at mint time)
/// and narrowed with [`Rights::diminish`] — never conjured from an integer a
/// caller supplies, which is what keeps an undefined bit from ever meaning
/// authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rights(u32);

impl Rights {
    /// No rights: a capability that permits nothing, though it still names its
    /// object.
    pub const NONE: Self = Self(0);
    /// May derive further capabilities from this one (see
    /// [`Capability::derive`](crate::Capability::derive)).
    pub const DUPLICATE: Self = Self(1 << 0);
    /// May transfer this capability to another process (in an IPC message).
    pub const TRANSFER: Self = Self(1 << 1);
    /// May read the object's state, or receive from it.
    pub const READ: Self = Self(1 << 2);
    /// May modify the object's state, or send to it.
    pub const WRITE: Self = Self(1 << 3);
    /// May revoke capabilities derived from this one.
    pub const REVOKE: Self = Self(1 << 4);

    /// Every defined right — the union of all named bits. Computed from the
    /// constants rather than written as a literal, so adding a right above
    /// extends this automatically and no undefined bit is ever included.
    pub const ALL: Self =
        Self(Self::DUPLICATE.0 | Self::TRANSFER.0 | Self::READ.0 | Self::WRITE.0 | Self::REVOKE.0);

    /// Whether `self` includes every right in `other`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Whether every right in `self` is also in `other`.
    #[must_use]
    pub const fn is_subset_of(self, other: Self) -> bool {
        other.contains(self)
    }

    /// The rights present in both `self` and `other`.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Combine two rights sets — for *assembling* an initial set at mint time,
    /// e.g. `Rights::READ.union(Rights::WRITE)`.
    ///
    /// This is a constructor convenience, not an operation on a held capability:
    /// the only way a *held* capability's rights change is [`diminish`](Self::diminish),
    /// which cannot widen. Union is never applied to widen an existing
    /// capability, so O-2 is not at risk from its existence.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Attenuate to `requested` — the O-2 operation. Succeeds only if `requested`
    /// is a subset of `self`, so a derivation can never gain a right its parent
    /// lacked. Returns [`None`] if `requested` asks for a right `self` does not
    /// hold.
    #[must_use]
    pub const fn diminish(self, requested: Self) -> Option<Self> {
        if self.contains(requested) {
            Some(requested)
        } else {
            None
        }
    }

    /// The raw bits, for packing into a stored capability or an IPC message.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for Rights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rights(")?;
        let mut first = true;
        for (name, bit) in [
            ("DUPLICATE", Self::DUPLICATE),
            ("TRANSFER", Self::TRANSFER),
            ("READ", Self::READ),
            ("WRITE", Self::WRITE),
            ("REVOKE", Self::REVOKE),
        ] {
            if self.contains(bit) {
                if !first {
                    write!(f, "|")?;
                }
                write!(f, "{name}")?;
                first = false;
            }
        }
        if first {
            write!(f, "NONE")?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::Rights;

    #[test]
    fn all_contains_every_named_right() {
        for bit in [
            Rights::DUPLICATE,
            Rights::TRANSFER,
            Rights::READ,
            Rights::WRITE,
            Rights::REVOKE,
        ] {
            assert!(Rights::ALL.contains(bit));
        }
    }

    #[test]
    fn none_contains_only_none() {
        assert!(Rights::ALL.contains(Rights::NONE));
        assert!(Rights::NONE.contains(Rights::NONE));
        assert!(!Rights::NONE.contains(Rights::READ));
    }

    #[test]
    fn subset_is_the_inverse_of_contains() {
        let rw = Rights::READ.union(Rights::WRITE);
        assert!(rw.is_subset_of(Rights::ALL));
        assert!(Rights::READ.is_subset_of(rw));
        assert!(!rw.is_subset_of(Rights::READ));
    }

    #[test]
    fn diminish_narrows_but_never_widens() {
        let rw = Rights::READ.union(Rights::WRITE);
        // Narrowing to a held subset succeeds and yields exactly that subset.
        assert_eq!(rw.diminish(Rights::READ), Some(Rights::READ));
        assert_eq!(rw.diminish(rw), Some(rw));
        assert_eq!(rw.diminish(Rights::NONE), Some(Rights::NONE));
        // Asking for a right not held fails closed — the O-2 guarantee.
        assert_eq!(rw.diminish(Rights::TRANSFER), None);
        assert_eq!(Rights::READ.diminish(Rights::WRITE), None);
        assert_eq!(Rights::NONE.diminish(Rights::READ), None);
    }

    #[test]
    fn diminish_result_is_always_a_subset_of_the_source() {
        // Exhaustively, for every source/request pair over the 5 defined bits,
        // a successful diminish returns a subset of the source. This is the
        // property O-2 rests on, checked rather than asserted.
        for s in 0u32..32 {
            for r in 0u32..32 {
                let source = rights_from_low_bits(s);
                let requested = rights_from_low_bits(r);
                if let Some(result) = source.diminish(requested) {
                    assert!(result.is_subset_of(source));
                }
            }
        }
    }

    #[test]
    fn intersection_and_union_are_dual() {
        let rw = Rights::READ.union(Rights::WRITE);
        let wt = Rights::WRITE.union(Rights::TRANSFER);
        assert_eq!(rw.intersection(wt), Rights::WRITE);
        assert!(rw.union(wt).contains(Rights::READ));
        assert!(rw.union(wt).contains(Rights::TRANSFER));
    }

    #[test]
    fn debug_lists_set_bits() {
        assert_eq!(format!("{:?}", Rights::NONE), "Rights(NONE)");
        assert_eq!(format!("{:?}", Rights::READ), "Rights(READ)");
        assert_eq!(
            format!("{:?}", Rights::READ.union(Rights::WRITE)),
            "Rights(READ|WRITE)"
        );
    }

    /// Build a `Rights` from the low 5 bits of `n`, for exhaustive testing only.
    fn rights_from_low_bits(n: u32) -> Rights {
        let mut r = Rights::NONE;
        for (i, bit) in [
            Rights::DUPLICATE,
            Rights::TRANSFER,
            Rights::READ,
            Rights::WRITE,
            Rights::REVOKE,
        ]
        .into_iter()
        .enumerate()
        {
            if n & (1 << i) != 0 {
                r = r.union(bit);
            }
        }
        r
    }
}
