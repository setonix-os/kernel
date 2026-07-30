// SPDX-License-Identifier: GPL-3.0-or-later

//! Handles — the userspace-facing name for a capability.

use crate::Generation;

/// The value a process holds to name one of its own capabilities: an index into
/// its capability table, plus the [`Generation`] the slot held when the handle
/// was minted.
///
/// A handle carries no authority by itself and is meaningless outside the
/// process that holds it — it is a lookup key the kernel resolves against a table
/// only the kernel can write (RFC-0003 §3). Two things make it safe to hand to
/// userspace: the capability it names lives in kernel memory, so there is
/// nothing to forge; and the generation makes a stale handle fail closed instead
/// of aliasing a reused slot. Handle `7` in one process and handle `7` in another
/// name unrelated capabilities, or none.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Handle {
    index: u32,
    generation: Generation,
}

impl Handle {
    /// Constructs a handle naming slot `index` at `generation`. Minted by the
    /// table when it stores a capability; a value userspace holds but cannot
    /// turn into authority it was not granted, because every resolve re-checks
    /// the generation against the live slot.
    #[must_use]
    pub const fn new(index: u32, generation: Generation) -> Self {
        Self { index, generation }
    }

    /// The table slot this handle names.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The generation this handle was minted at.
    #[must_use]
    pub const fn generation(self) -> Generation {
        self.generation
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::Handle;
    use crate::Generation;

    #[test]
    fn round_trips_index_and_generation() {
        let h = Handle::new(7, Generation::FIRST);
        assert_eq!(h.index(), 7);
        assert_eq!(h.generation(), Generation::FIRST);
    }

    #[test]
    fn handles_differing_in_generation_are_distinct() {
        let g2 = Generation::FIRST.next().expect("second generation exists");
        assert_ne!(Handle::new(7, Generation::FIRST), Handle::new(7, g2));
        // Same slot, same generation: equal.
        assert_eq!(Handle::new(7, g2), Handle::new(7, g2));
        // Same generation, different slot: distinct.
        assert_ne!(Handle::new(7, g2), Handle::new(8, g2));
    }
}
