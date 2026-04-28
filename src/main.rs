#![no_std]
#![no_main]

pub mod arch;
pub mod boot;
pub mod console;
pub mod drivers;
pub mod linker;
pub mod mm;

pub use boot::abort::abort;
pub use boot::entry::{kmain, skmain};
