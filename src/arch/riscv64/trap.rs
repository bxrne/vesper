//! M-mode trap install + Rust dispatcher.
//!
//! Traps are kept in **machine mode** even though the kernel runs in
//! S-mode: M-mode access bypasses paging, so the handler can touch the
//! TrapFrame, trap stack, UART, CLINT, and PLIC by physical address
//! without any mapping ceremony.

use core::arch::asm;

use crate::arch::riscv64::trap_frame::TrapFrame;
use crate::drivers::plic;
use crate::drivers::uart::{UART_BASE, Uart};
use crate::mm::alloc::page_frame;
use crate::{print, println};

unsafe extern "C" {
    fn m_trap_vector();
}

/// 16 KiB stack for the trap handler — generous enough that nested
/// `println!` formatting won't run off the end.
const TRAP_STACK_PAGES: usize = 4;

const MIE_MTIE: usize = 1 << 7; // machine timer
const MIE_MEIE: usize = 1 << 11; // machine external (PLIC)

const CLINT_MTIMECMP: *mut u64 = 0x0200_4000 as *mut u64;
const CLINT_MTIME: *const u64 = 0x0200_bff8 as *const u64;
/// QEMU's virt machine drives `mtime` at 10 MHz, so this many ticks
/// equals one wall-clock second.
const TIMER_TICK: u64 = 10_000_000;

// A single hart only — once SMP lands this becomes a per-hart array
// indexed by mhartid.
static mut TRAP_FRAME: TrapFrame = TrapFrame::empty();

/// Allocate a trap stack, point the static TrapFrame at it, and program
/// `mscratch` / `mtvec` so the next trap reaches `m_trap_vector`.
///
/// # Safety
///
/// Must run in M-mode after the page-frame allocator is initialised
/// and before machine interrupts are enabled.
pub unsafe fn install(satp: usize) {
    let trap_stack = page_frame::zallocate(TRAP_STACK_PAGES)
        .expect("OOM allocating trap stack")
        .as_ptr();
    // Stack grows down, so sp starts one byte past the high end.
    let stack_top = unsafe { trap_stack.add(TRAP_STACK_PAGES * page_frame::PAGE_SIZE) };

    let frame_ptr = &raw mut TRAP_FRAME;
    unsafe {
        (*frame_ptr).satp = satp;
        (*frame_ptr).trap_stack = stack_top;
        (*frame_ptr).hartid = 0;

        asm!("csrw mscratch, {}", in(reg) frame_ptr as usize);
        asm!("csrw mtvec,    {}", in(reg) m_trap_vector as *const () as usize);
    }
}

/// Enable machine timer + external interrupts. The first `mtimecmp` is
/// scheduled before unmasking MTIE so the handler doesn't fire
/// immediately on the reset value of zero.
pub fn enable_interrupts() {
    unsafe {
        CLINT_MTIMECMP.write_volatile(CLINT_MTIME.read_volatile() + TIMER_TICK);
        asm!("csrw mie, {}", in(reg) MIE_MEIE | MIE_MTIE);
    }
}

#[unsafe(no_mangle)]
extern "C" fn m_trap(
    epc: usize,
    tval: usize,
    cause: usize,
    hart: usize,
    _status: usize,
    _frame: &mut TrapFrame,
) -> usize {
    // The MSB of mcause distinguishes async (interrupt) from sync
    // (exception); the low 12 bits give the cause number.
    let is_async = (cause >> 63) & 1 == 1;
    let cause_num = cause & 0xfff;
    let mut return_pc = epc;

    if is_async {
        match cause_num {
            3 => println!("machine software interrupt cpu#{}", hart),
            7 => unsafe {
                // Rearm the next tick. No print — the timer fires at 1 Hz
                // and would otherwise drown out everything else.
                CLINT_MTIMECMP.write_volatile(CLINT_MTIME.read_volatile() + TIMER_TICK);
            },
            11 => handle_external(),
            _ => panic!("unhandled async trap cpu#{} cause {}", hart, cause_num),
        }
    } else {
        match cause_num {
            2 => panic!(
                "illegal instruction cpu#{} epc=0x{:x} tval=0x{:x}",
                hart, epc, tval
            ),
            8 => {
                println!("ecall from U-mode cpu#{} epc=0x{:x}", hart, epc);
                // Skip past the ecall so we don't immediately re-trap.
                return_pc += 4;
            }
            9 => {
                println!("ecall from S-mode cpu#{} epc=0x{:x}", hart, epc);
                return_pc += 4;
            }
            // M-mode ecalls are unexpected: nothing in this kernel
            // executes `ecall` while running in M-mode.
            11 => panic!("ecall from M-mode cpu#{} epc=0x{:x}", hart, epc),
            12 => {
                println!(
                    "instruction page fault cpu#{} epc=0x{:x} tval=0x{:x}",
                    hart, epc, tval
                );
                return_pc += 4;
            }
            13 => {
                println!(
                    "load page fault cpu#{} epc=0x{:x} tval=0x{:x}",
                    hart, epc, tval
                );
                return_pc += 4;
            }
            15 => {
                println!(
                    "store page fault cpu#{} epc=0x{:x} tval=0x{:x}",
                    hart, epc, tval
                );
                return_pc += 4;
            }
            _ => panic!("unhandled sync trap cpu#{} cause {}", hart, cause_num),
        }
    }

    return_pc
}

/// Dispatch a Machine External Interrupt: ask the PLIC which source
/// fired, service it, then mark the source as complete.
fn handle_external() {
    let Some(id) = plic::next() else {
        // The PLIC reports source 0 only on a spurious interrupt — no
        // ack is required because nothing was actually claimed.
        return;
    };
    match id {
        plic::UART0_IRQ => echo_uart(),
        _ => println!("non-UART external interrupt: {}", id),
    }
    plic::complete(id);
}

/// ANSI CSI sequences (`ESC [ X`) arrive one byte per interrupt, so the
/// handler tracks where it is in the sequence between calls. Single
/// hart, so a plain `static mut` is sufficient — no synchronisation.
#[derive(Copy, Clone)]
enum EscState {
    Idle,
    EscSeen,
    CsiOpen,
}

static mut ESC_STATE: EscState = EscState::Idle;

fn echo_uart() {
    let uart = Uart::new(UART_BASE);
    let Some(c) = uart.get() else { return };

    let state = unsafe { ESC_STATE };
    match state {
        EscState::Idle => match c {
            0x08 | 0x7f => print!("\x08 \x08"),
            b'\r' | b'\n' => println!(),
            // Start of a CSI sequence — swallow until it resolves.
            0x1b => unsafe { ESC_STATE = EscState::EscSeen },
            _ => print!("{}", c as char),
        },
        EscState::EscSeen => {
            unsafe {
                ESC_STATE = if c == b'[' {
                    EscState::CsiOpen
                } else {
                    // Not a CSI sequence — drop the bare ESC.
                    EscState::Idle
                };
            }
        }
        EscState::CsiOpen => {
            match c {
                b'A' => println!("up arrow!"),
                b'B' => println!("down arrow!"),
                b'C' => println!("right arrow!"),
                b'D' => println!("left arrow!"),
                _ => {}
            }
            unsafe { ESC_STATE = EscState::Idle };
        }
    }
}
