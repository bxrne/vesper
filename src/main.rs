#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

pub mod mmu;
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

unsafe extern "C" {
    static _text_start: u8;
    static _text_end: u8;
    static _rodata_start: u8;
    static _rodata_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _bss_start: u8;
    static _bss_end: u8;
    static _stack_start: u8;
    static _stack_end: u8;
    static _heap_start: u8;
    static _memory_end: u8;
}

#[inline]
fn sym_addr(sym: &'static u8) -> usize {
    sym as *const u8 as usize
}

#[inline]
fn align_down(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr & !(align - 1)
}

#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr.next_multiple_of(align)
}

/// Kernel entry point. Reached from `_start` (boot.S) via `mret` in M-mode.
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    let uart = Uart::new(UART_BASE);
    uart.init();

    println!();
    println!("vesper booted successfully!");

    page::init();

    let root_page = page::zallocate(1).expect("OOM allocating root page table");
    let root_table_addr = root_page.as_ptr() as usize;
    let root = unsafe { &mut *(root_table_addr as *mut page::Table) };

    unsafe {
        let text_start = sym_addr(&_text_start);
        let text_end = sym_addr(&_text_end);
        let rodata_start = sym_addr(&_rodata_start);
        let rodata_end = sym_addr(&_rodata_end);
        let data_start = sym_addr(&_data_start);
        let data_end = sym_addr(&_data_end);
        let bss_start = sym_addr(&_bss_start);
        let bss_end = sym_addr(&_bss_end);
        let stack_start = sym_addr(&_stack_start);
        let stack_end = sym_addr(&_stack_end);
        let heap_start = sym_addr(&_heap_start);
        let memory_end = sym_addr(&_memory_end);

        let rx = page::PteFlags::Read.bits()
            | page::PteFlags::Execute.bits()
            | page::PteFlags::Accessed.bits();
        let ro = page::PteFlags::Read.bits() | page::PteFlags::Accessed.bits();
        let rw = page::PteFlags::Read.bits()
            | page::PteFlags::Write.bits()
            | page::PteFlags::Accessed.bits()
            | page::PteFlags::Dirty.bits();

        mmu::id_map_range(
            root,
            align_down(text_start, page::PAGE_SIZE),
            align_up(text_end, page::PAGE_SIZE),
            rx,
        );
        mmu::id_map_range(
            root,
            align_down(rodata_start, page::PAGE_SIZE),
            align_up(rodata_end, page::PAGE_SIZE),
            ro,
        );
        mmu::id_map_range(
            root,
            align_down(data_start, page::PAGE_SIZE),
            align_up(data_end, page::PAGE_SIZE),
            rw,
        );
        mmu::id_map_range(
            root,
            align_down(bss_start, page::PAGE_SIZE),
            align_up(bss_end, page::PAGE_SIZE),
            rw,
        );
        mmu::id_map_range(
            root,
            align_down(stack_start, page::PAGE_SIZE),
            align_up(stack_end, page::PAGE_SIZE),
            rw,
        );
        mmu::id_map_range(
            root,
            align_down(heap_start, page::PAGE_SIZE),
            align_up(memory_end, page::PAGE_SIZE),
            rw,
        );

        // UART MMIO (identity map a single page).
        let uart_base = align_down(UART_BASE, page::PAGE_SIZE);
        mmu::id_map_range(root, uart_base, uart_base + page::PAGE_SIZE, rw);
    }

    let satp = mmu::make_satp_sv39(root_table_addr);
    unsafe { mmu::enter_sv39(satp, skmain) };
}

/// Supervisor-mode entry point after paging is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn skmain() -> ! {
    let uart = Uart::new(UART_BASE);
    uart.init();
    println!();
    println!("paging enabled (Sv39), now in S-mode");
    echo_loop(&uart)
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
