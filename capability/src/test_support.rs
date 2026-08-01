// SPDX-License-Identifier: GPL-3.0-or-later

//! Test doubles shared by the unit tests. Compiled only under `cfg(test)`;
//! kernel builds never see this module.

use crate::{Generation, ObjectRef};
use std::cell::Cell;
use std::rc::Rc;

/// A counted reference to a pretend kernel object.
///
/// Mirrors what the kernel's object references will provide: cloning
/// duplicates the *reference*, never authority — that is `derive`'s job — and
/// the shared generation cell lets a test destroy the object out from under
/// its capabilities, which is exactly the situation the generation check
/// exists for.
#[derive(Clone, Debug)]
pub(crate) struct TestObject {
    /// Distinguishes objects in assertions.
    id: u32,
    /// The object's generation, shared across all clones of the reference.
    generation: Rc<Cell<Generation>>,
}

impl TestObject {
    pub(crate) fn new(id: u32) -> Self {
        Self {
            id,
            generation: Rc::new(Cell::new(Generation::FIRST)),
        }
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    /// Destroy the object: bump its generation, so that every capability
    /// minted before this moment fails closed at its next resolve.
    pub(crate) fn destroy(&self) {
        let next = self
            .generation
            .get()
            .next()
            .expect("test generations never exhaust");
        self.generation.set(next);
    }
}

impl ObjectRef for TestObject {
    fn current_generation(&self) -> Generation {
        self.generation.get()
    }
}
