#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

pub mod page;
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
    match info.location() {
        Some(loc) => println!("kernel panic at {}: {}", loc, info.message()),
        None => println!("kernel panic: {}", info.message()),
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

    echo_loop(&uart);
}

/// Polled UART echo loop with minimal ANSI escape handling.
fn echo_loop(uart: &Uart) -> ! {
    loop {
        let Some(byte) = uart.get() else { continue };
        match byte {
            // CR -> CRLF so the prompt looks right on a raw serial console.
            b'\r' => println!(),
            // backspace / DEL: erase the previous glyph.
            0x08 | 0x7f => print!("\x08 \x08"),
            // ANSI escape — for arrow keys the terminal sends ESC `[` X.
            // The follow-up bytes haven't necessarily landed yet, so block.
            0x1b => handle_escape(uart),
            b => print!("{}", b as char),
        }
    }
}

fn handle_escape(uart: &Uart) {
    if uart.get_blocking() != b'[' {
        return;
    }
    match uart.get_blocking() {
        b'A' => println!("up arrow!"),
        b'B' => println!("down arrow!"),
        b'C' => println!("right arrow!"),
        b'D' => println!("left arrow!"),
        _ => {}
    }
}
