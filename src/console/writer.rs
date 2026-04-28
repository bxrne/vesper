use crate::drivers::uart::{UART_BASE, Uart};

#[inline]
pub fn console_writer() -> Uart {
    Uart::new(UART_BASE)
}
