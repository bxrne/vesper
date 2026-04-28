#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

pub mod uart;

use uart::{UART_BASE, Uart};

// Pull in the assembly entry point. `_start` lives in boot.S, sets up
// CSRs, and `mret`s into `kmain`.
global_asm!(include_str!("asm/boot.S"));

/// Park the hart forever. Called from the panic handler and as the
/// fallback after `kmain` returns.
#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    loop {
        unsafe { asm!("wfi") }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("kernel panic: ");
    if let Some(loc) = info.location() {
        println!(
            "{}:{}:{}: {}",
            loc.file(),
            loc.line(),
            loc.column(),
            info.message()
        );
    } else {
        println!("{}", info.message());
    }
    abort();
}

/// Kernel entry point. Reached from `_start` (boot.S) via `mret`.
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    let uart = Uart::new(UART_BASE);
    uart.init();

    println!();
    println!("vesper booted successfully!");

    // Echo loop — type into the QEMU console and watch it come back.
    // Ctrl-A, x to exit QEMU.
    loop {
        let Some(byte) = uart.get() else { continue };
        match byte {
            // CR -> CRLF so the prompt looks right under -nographic
            b'\r' => println!(),
            // backspace / DEL: erase the previous glyph
            0x08 | 0x7f => print!("\x08 \x08"),
            // ANSI escape sequence — for arrows the terminal sends ESC `[` X.
            // The follow-up bytes haven't necessarily landed yet, so block.
            0x1b => {
                if uart.get_blocking() != b'[' {
                    continue;
                }
                match uart.get_blocking() {
                    b'A' => println!("up arrow!"),
                    b'B' => println!("down arrow!"),
                    b'C' => println!("right arrow!"),
                    b'D' => println!("left arrow!"),
                    _ => {}
                }
            }
            b => print!("{}", b as char),
        }
    }
}
