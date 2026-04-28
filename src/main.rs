#![no_std]
#![no_main]

use core::arch::{asm, global_asm};

// Pull in the assembly entry point. `_start` lives in boot.S and is the
// real ELF entry; it sets up CSRs and jumps to `kmain` via `mret`.
global_asm!(include_str!("asm/boot.S"));

// RUST MACROS
#[macro_export]
macro_rules! print {
    ($($args:tt)+) => {{}};
}
#[macro_export]
macro_rules! println {
    () => ({
        print!("\r\n")
    });
    ($fmt:expr) => ({
        print!(concat!($fmt, "\r\n"))
    });
    ($fmt:expr, $($args:tt)+) => ({
        print!(concat!($fmt, "\r\n"), $($args)+)
    });
}

// LANGUAGE STRUCTURES / FUNCTIONS
#[unsafe(no_mangle)]
extern "C" fn eh_personality() {}

#[panic_handler]
#[allow(unused_variables)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    print!("Aborting: ");
    if let Some(p) = info.location() {
        println!("line {}, file {}: {}", p.line(), p.file(), info.message());
    } else {
        println!("no information available.");
    }
    abort();
}

#[unsafe(no_mangle)]
extern "C" fn abort() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kmain() {
    // Main should initialize all sub-systems and get
    // ready to start scheduling. The last thing this
    // should do is start the timer.
}
