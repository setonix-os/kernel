// SPDX-License-Identifier: GPL-3.0-or-later

//! The panic handler.
//!
//! A kernel panic is terminal by construction: the `*-unknown-none` targets abort
//! rather than unwind, so there is no path back. What matters is that the reason
//! reaches the console before the core stops, because on bare metal the console
//! is often the only witness.

use core::panic::PanicInfo;

use crate::console::kprintln;

/// Reports the panic on the console, then stops this core for good.
#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // Written as separate lines rather than one format string so that a fault
    // partway through still leaves the earlier part on the wire. When the
    // console itself is the suspect, half a message is evidence.
    kprintln!();
    kprintln!("[PANIC]");

    if let Some(location) = info.location() {
        kprintln!(
            "  at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }

    kprintln!("  {}", info.message());
    kprintln!();
    kprintln!("Core halted.");

    crate::arch::halt()
}
