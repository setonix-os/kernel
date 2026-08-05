// SPDX-License-Identifier: GPL-3.0-or-later

//! The kernel console.
//!
//! A write-only, unbuffered, unlocked path to whatever the architecture offers as
//! a serial device. Unlocked is a deliberate choice rather than an omission: the
//! panic handler writes through here, and a lock the panic handler can block on
//! is a lock that turns a diagnosable fault into a silent hang.

use core::fmt::{self, Write};

/// A `fmt::Write` sink over the architecture's console.
///
/// Zero-sized: there is no state to hold, because the device is a fixed address
/// and there is nothing to buffer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::arch::console_write_str(s);
        Ok(())
    }
}

/// Writes a string slice to the console.
pub(crate) fn write_str(s: &str) {
    crate::arch::console_write_str(s);
}

/// Writes formatted output to the console.
///
/// Formatting errors are discarded: the sink cannot fail, and there is nowhere to
/// report a failure to report something.
pub(crate) fn write_fmt(args: fmt::Arguments<'_>) {
    let _ = Console.write_fmt(args);
}

/// Writes to the console, `format!`-style, with a trailing newline.
///
/// There is deliberately no `kprint!` counterpart yet: nothing needs one, and an
/// unused macro is dead weight that CI would reject anyway. Add it when a caller
/// genuinely wants output without a line break.
macro_rules! kprintln {
    () => ($crate::console::write_str("\n"));
    ($($arg:tt)*) => ($crate::console::write_fmt(format_args!("{}\n", format_args!($($arg)*))));
}

pub(crate) use kprintln;

/// The greeting, per `CONSTITUTION.md` §8.
const GREETING: &str = "Kaya!";

/// The resident console critter, per `CONSTITUTION.md` §8.
const CRITTER: &str = r"  (\_/)
 ( •_•)
 / ></";

/// Announces the kernel on the console.
///
/// The first output of the system, and for now the only evidence that any of it
/// works. Kept as the boot smoke test's expected string precisely because it
/// exercises the whole chain — link script, boot stub, stack, `.bss`, the
/// hardware-abstraction boundary, and the console device — in one line.
pub(crate) fn greet() {
    kprintln!();
    kprintln!("{GREETING}");
    kprintln!("{CRITTER}");
    kprintln!();
}
