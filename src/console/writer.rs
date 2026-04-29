use crate::drivers::uart::{UART_BASE, Uart};

/// Construct a fresh UART handle for the current `print!` invocation.
/// The handle is just a raw MMIO base, so this is essentially free and
/// avoids needing global mutable state.
#[inline]
pub fn console_writer() -> Uart {
    Uart::new(UART_BASE)
}
