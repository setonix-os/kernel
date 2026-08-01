// SPDX-License-Identifier: GPL-3.0-or-later

//! Generation counters — the defence against handle reuse (the ABA problem).

/// A counter that distinguishes successive occupants of one table slot, and
/// successive incarnations of one object.
///
/// A [`Handle`](crate::Handle) records the generation its slot held when the
/// handle was minted; resolving the handle checks that generation still matches.
/// Closing a capability and reusing its slot, or destroying an object, bumps the
/// generation — so a stale handle fails closed rather than resolving to whatever
/// now occupies the slot. This is the mechanism behind RFC-0003 O-1
/// (unforgeability against reuse) and the destruction half of O-3 (revocation by
/// making outstanding capabilities inert).
///
/// 64 bits wide and **fail-closed on exhaustion** (RFC-0003 amendment): a slot
/// that somehow exhausts its generations is retired, never wrapped back to a
/// value a live handle might match. At 2^64 reuses of a single slot, exhaustion
/// is unreachable in practice; the type refuses to wrap regardless, so the
/// guarantee does not rest on that improbability.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a freshly created slot or object. Starts at 1, leaving
    /// 0 available as a "never allocated" sentinel for later use.
    pub const FIRST: Self = Self(1);

    /// The next generation, or [`None`] if this one is the last representable —
    /// the fail-closed boundary. A caller that receives [`None`] must retire the
    /// slot, never reuse it.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// The raw value, for packing into a handle or comparing on resolve.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
impl Generation {
    /// The last representable generation — test-only, so the fail-closed
    /// exhaustion boundary can be reached without 2^64 bumps. Kernel builds
    /// never see this constructor; outside tests a generation is only ever
    /// [`FIRST`](Self::FIRST) or the successor of an existing one.
    pub(crate) const fn last() -> Self {
        Self(u64::MAX)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::Generation;

    #[test]
    fn first_is_one_and_advances() {
        assert_eq!(Generation::FIRST.get(), 1);
        assert_eq!(Generation::FIRST.next(), Some(Generation(2)));
        assert_eq!(Generation(2).next(), Some(Generation(3)));
    }

    #[test]
    fn generations_are_strictly_increasing_and_distinct() {
        let mut g = Generation::FIRST;
        let mut previous = None;
        for _ in 0..1000 {
            if let Some(p) = previous {
                assert!(g > p);
            }
            previous = Some(g);
            g = g.next().expect("u64 does not exhaust in 1000 steps");
        }
    }

    #[test]
    fn exhaustion_fails_closed_rather_than_wrapping() {
        // The whole point: at the boundary, `next` returns None instead of
        // wrapping to a value a stale handle could match.
        assert_eq!(Generation(u64::MAX).next(), None);
        // And one before the boundary still advances.
        assert_eq!(Generation(u64::MAX - 1).next(), Some(Generation(u64::MAX)));
    }
}
