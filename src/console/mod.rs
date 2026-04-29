pub mod writer;

/// Print to the kernel console.
#[macro_export]
macro_rules! print {
    ($($args:tt)*) => {{
        use core::fmt::Write as _;
        let _ = write!($crate::console::writer::console_writer(), $($args)*);
    }};
}

/// Print to the kernel console with a trailing CRLF. CRLF (not just LF)
/// because the serial terminal is in raw mode and won't translate `\n`.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($args:tt)*) => ($crate::print!("{}\r\n", format_args!($($args)*)));
}
