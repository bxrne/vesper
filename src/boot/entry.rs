use crate::arch;
use crate::drivers::uart::{UART_BASE, Uart};
use crate::linker;
use crate::mm;
use crate::{print, println};

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
pub extern "C" fn kinit() -> ! {
    let uart = Uart::new(UART_BASE);
    uart.init();

    println!();
    println!("vesper booted successfully!");

    mm::alloc::page_frame::init();

    let root_page = mm::alloc::page_frame::zallocate(1).expect("OOM allocating root page table");
    let root_table_addr = root_page.as_ptr() as usize;
    let root = unsafe { &mut *(root_table_addr as *mut mm::paging::sv39::types::Table) };

    let rx = mm::paging::sv39::types::PteFlags::READ
        | mm::paging::sv39::types::PteFlags::EXECUTE
        | mm::paging::sv39::types::PteFlags::ACCESSED;
    let ro = mm::paging::sv39::types::PteFlags::READ | mm::paging::sv39::types::PteFlags::ACCESSED;
    let rw = mm::paging::sv39::types::PteFlags::READ
        | mm::paging::sv39::types::PteFlags::WRITE
        | mm::paging::sv39::types::PteFlags::ACCESSED
        | mm::paging::sv39::types::PteFlags::DIRTY;

    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::text_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::text_end(), mm::alloc::page_frame::PAGE_SIZE),
        rx,
    );
    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::rodata_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::rodata_end(), mm::alloc::page_frame::PAGE_SIZE),
        ro,
    );
    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::data_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::data_end(), mm::alloc::page_frame::PAGE_SIZE),
        rw,
    );
    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::bss_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::bss_end(), mm::alloc::page_frame::PAGE_SIZE),
        rw,
    );
    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::stack_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::stack_end(), mm::alloc::page_frame::PAGE_SIZE),
        rw,
    );
    mm::paging::sv39::map::id_map_range(
        root,
        align_down(linker::heap_start(), mm::alloc::page_frame::PAGE_SIZE),
        align_up(linker::memory_end(), mm::alloc::page_frame::PAGE_SIZE),
        rw,
    );

    // UART MMIO (identity map a single page).
    let uart_base = align_down(UART_BASE, mm::alloc::page_frame::PAGE_SIZE);
    mm::paging::sv39::map::id_map_range(
        root,
        uart_base,
        uart_base + mm::alloc::page_frame::PAGE_SIZE,
        rw,
    );

    let satp = arch::make_satp_sv39(root_table_addr);
    arch::enter_sv39(satp, skmain)
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
