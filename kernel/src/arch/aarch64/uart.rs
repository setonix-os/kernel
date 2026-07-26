// SPDX-License-Identifier: GPL-3.0-or-later

//! PL011 UART — the console on QEMU's AArch64 `virt` machine.
//!
//! Deliberately the smallest thing that can carry a character: no initialisation,
//! no interrupt path, no receive path. QEMU's firmware leaves the PL011 already
//! configured, and a console that only has to survive long enough to report a
//! fault is worth more early on than a complete driver. It will move to userspace
//! once there is a userspace to move it to — the Borrow Ledger puts drivers there.
//!
//! Designated `unsafe` module: all hardware access is memory-mapped I/O.
#![allow(unsafe_code)]

use core::ptr;

/// Base of UART0 on the `virt` machine. Fixed by the machine model, not
/// discovered — device-tree parsing arrives with the driver framework.
const UART0_BASE: usize = 0x0900_0000;

/// Data register: a write transmits one byte.
const UART_DR: usize = 0x00;

/// Flag register.
const UART_FR: usize = 0x18;

/// `UART_FR` bit 5: transmit FIFO full.
const FR_TXFF: u32 = 1 << 5;

/// Transmits one byte, blocking until the FIFO has room for it.
fn write_byte(byte: u8) {
    // `with_exposed_provenance` rather than an `as` cast: casting an integer to
    // a pointer is exactly the operation the strict-provenance API exists to
    // make explicit, and it keeps the intent legible to anyone auditing MMIO.
    let flags: *const u32 = ptr::with_exposed_provenance(UART0_BASE + UART_FR);
    let data: *mut u32 = ptr::with_exposed_provenance_mut(UART0_BASE + UART_DR);

    // SAFETY: both addresses are within the PL011's MMIO window, which the
    // `virt` machine maps for the lifetime of the system and which no other
    // code in the kernel touches. Both registers are 32 bits wide and naturally
    // aligned at these offsets. The accesses are volatile, so the compiler may
    // neither elide nor reorder them, which is the property that makes a
    // device-register poll terminate.
    unsafe {
        while ptr::read_volatile(flags) & FR_TXFF != 0 {
            core::hint::spin_loop();
        }
        ptr::write_volatile(data, u32::from(byte));
    }
}

/// Transmits a string, translating `\n` into `\r\n`.
///
/// A serial terminal does not move the carriage on a bare line feed, so without
/// this every line after the first would start where the previous one ended.
pub(super) fn write_str(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}
