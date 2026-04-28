pub mod writer;

/// Print to the kernel console. Mirrors `std::print!`.
#[macro_export]
macro_rules! print {
    ($($args:tt)*) => {{
        use core::fmt::Write as _;
        let _ = write!($crate::console::writer::console_writer(), $($args)*);
    }};
}

/// Print to the kernel console with a trailing CRLF.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($args:tt)*) => ($crate::print!("{}\r\n", format_args!($($args)*)));
}
