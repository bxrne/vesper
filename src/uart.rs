//! NS16550A UART driver.
//!
//! The QEMU `virt` machine exposes a 16550-compatible UART at MMIO base
//! `0x1000_0000`. Register offsets (8-bit, with `DLAB = 0`):
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
//! For now we drive the UART by polling — `kmain` is the only consumer
//! and no other harts run, so we don't need a lock.
use core::fmt;
use core::ptr::{read_volatile, write_volatile};

/// Base address of the NS16550A UART on the QEMU `virt` machine.
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

/// A handle to a 16550-compatible UART.
///
/// The struct is zero-sized in spirit — it just remembers the MMIO base
/// — so it's cheap to construct on demand inside `print!`.
pub struct Uart {
    base: *mut u8,
}

impl Uart {
    /// Construct a UART handle for the given MMIO base address.
    pub const fn new(base: usize) -> Self {
        Self { base: base as *mut u8 }
    }

    /// Configure the UART: 8N1, FIFO enabled, RX interrupt enabled.
    pub fn init(&self) {
        unsafe {
            self.write(LCR, LCR_8BITS);
            self.write(FCR, FCR_ENABLE);
            self.write(IER, IER_RX_ENABLE);
        }
    }

    /// Block until the transmit holding register is empty, then write a byte.
    pub fn put(&self, byte: u8) {
        unsafe {
            while self.read(LSR) & LSR_THR_EMPTY == 0 {}
            self.write(RBR_THR, byte);
        }
    }

    /// Non-blocking receive. Returns `None` if no byte is ready.
    pub fn get(&self) -> Option<u8> {
        unsafe {
            if self.read(LSR) & LSR_DATA_READY == 0 {
                None
            } else {
                Some(self.read(RBR_THR))
            }
        }
    }

    /// Block until a byte arrives, then return it.
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

/// Print to the UART. Mirrors `std::print!`.
#[macro_export]
macro_rules! print {
    ($($args:tt)*) => {{
        use core::fmt::Write as _;
        let _ = write!(
            $crate::uart::Uart::new($crate::uart::UART_BASE),
            $($args)*
        );
    }};
}

/// Print to the UART with a trailing CRLF.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($args:tt)*) => ($crate::print!("{}\r\n", format_args!($($args)*)));
}
