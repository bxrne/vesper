//! NS16550A UART driver for the QEMU `virt` machine.
//!
//! Register offsets (8-bit, with `DLAB = 0`):
//!
//! | offset | read       | write       |
//! |-------:|------------|-------------|
//! | 0      | RBR        | THR         |
//! | 1      | IER        | IER         |
//! | 2      | IIR        | FCR         |
//! | 3      | LCR        | LCR         |
//! | 4      | MCR        | MCR         |
//! | 5      | LSR        |             |
//!
//! Polled access only — there is no trap handler yet, and only one hart
//! is running, so locking would just be ceremony.

use core::fmt;
use core::ptr::{read_volatile, write_volatile};

/// MMIO base of the NS16550A on the QEMU `virt` machine.
pub const UART_BASE: usize = 0x1000_0000;

const RBR_THR: usize = 0;
const IER: usize = 1;
const FCR: usize = 2;
const LCR: usize = 3;
const LSR: usize = 5;

const LSR_DATA_READY: u8 = 1 << 0;
const LSR_THR_EMPTY: u8 = 1 << 5;

const LCR_8BITS: u8 = 0b11;
const FCR_ENABLE: u8 = 0b1;
const IER_RX_ENABLE: u8 = 0b1;

/// Handle to a 16550-compatible UART. Holds only the MMIO base, so
/// constructing one inside `print!` is essentially free.
pub struct Uart {
    base: *mut u8,
}

impl Uart {
    pub const fn new(base: usize) -> Self {
        Self {
            base: base as *mut u8,
        }
    }

    /// 8N1, FIFO on, RX interrupt enabled. RX-IRQ is harmless while
    /// MIE is off and saves a register write once interrupts are wired.
    pub fn init(&self) {
        unsafe {
            self.write(LCR, LCR_8BITS);
            self.write(FCR, FCR_ENABLE);
            self.write(IER, IER_RX_ENABLE);
        }
    }

    /// Block until the THR is empty before pushing a byte; otherwise
    /// fast back-to-back writes drop characters silently.
    pub fn put(&self, byte: u8) {
        unsafe {
            while self.read(LSR) & LSR_THR_EMPTY == 0 {}
            self.write(RBR_THR, byte);
        }
    }

    /// Non-blocking receive. `None` means the RX FIFO is empty right now.
    pub fn get(&self) -> Option<u8> {
        unsafe {
            if self.read(LSR) & LSR_DATA_READY == 0 {
                None
            } else {
                Some(self.read(RBR_THR))
            }
        }
    }

    pub fn get_blocking(&self) -> u8 {
        loop {
            if let Some(b) = self.get() {
                return b;
            }
        }
    }

    #[inline(always)]
    unsafe fn read(&self, offset: usize) -> u8 {
        unsafe { read_volatile(self.base.add(offset)) }
    }

    #[inline(always)]
    unsafe fn write(&self, offset: usize, value: u8) {
        unsafe { write_volatile(self.base.add(offset), value) }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.put(byte);
        }
        Ok(())
    }
}
