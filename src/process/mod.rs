//! Cooperative-preemptive kernel-thread scheduler.
//!
//! Each [`Process`] owns its own stack and TrapFrame and runs in S-mode
//! sharing the kernel's identity-mapped page table — i.e. these are
//! kernel threads, not user processes. User-mode and per-process page
//! tables come later when we can load ELFs.
//!
//! Context switches are driven by the M-mode timer interrupt:
//! [`schedule`] picks the next runnable process, the trap handler
//! reprograms `mscratch` to point at its frame, and the trap vector
//! restores its registers on the way out.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::arch::riscv64::trap;
use crate::arch::riscv64::trap_frame::TrapFrame;
use crate::mm::alloc::page_frame::{self, PAGE_SIZE};
use crate::println;

/// Per-process kernel stack size. 8 KiB is plenty for the toy
/// workloads here and keeps the demo cheap.
const STACK_PAGES: usize = 2;
/// Hard cap on concurrent processes. Static array avoids needing a
/// general-purpose allocator at this stage.
const MAX_PROCESSES: usize = 16;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Dead,
}

pub struct Process {
    pub frame: TrapFrame,
    pub stack: NonNull<u8>,
    /// Resume PC. Updated on every preemption so the next switch lands
    /// on the instruction after the one we trapped on.
    pub pc: usize,
    pub pid: u16,
    pub state: ProcessState,
}

struct Table {
    slots: [Option<Process>; MAX_PROCESSES],
    /// Index of the currently-running process in `slots`, if any.
    current: Option<usize>,
}

/// Single-hart `Sync` newtype around `UnsafeCell`. The kernel runs on
/// one hart and traps disable preemption while in M-mode, so a plain
/// `static` works as long as access goes through `unsafe`.
struct Shared<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Shared<T> {}

impl<T> Shared<T> {
    const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }
    /// # Safety
    ///
    /// Caller must guarantee no concurrent access — true on a single
    /// hart with M-mode interrupts disabling each other.
    #[allow(clippy::mut_from_ref)]
    unsafe fn get(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

static TABLE: Shared<Table> = Shared::new(Table {
    slots: [const { None }; MAX_PROCESSES],
    current: None,
});

/// PIDs are handed out monotonically; 0 is reserved as "no process".
static NEXT_PID: AtomicU16 = AtomicU16::new(1);

#[derive(Debug)]
pub enum SpawnError {
    /// All [`MAX_PROCESSES`] slots are occupied.
    TableFull,
    /// The page-frame allocator couldn't satisfy the stack request.
    OutOfMemory,
}

/// Spawn a new kernel thread that begins executing at `entry` on a
/// freshly-allocated stack. Returns the assigned PID.
pub fn spawn_kernel(entry: fn() -> !) -> Result<u16, SpawnError> {
    let stack = page_frame::zallocate(STACK_PAGES).ok_or(SpawnError::OutOfMemory)?;
    // Stack grows down, so sp starts past the top of the allocation.
    let stack_top = unsafe { stack.as_ptr().add(STACK_PAGES * PAGE_SIZE) } as usize;

    let mut frame = TrapFrame::empty();
    frame.regs[2] = stack_top; // sp (x2)
    // Every process shares the same M-mode trap stack; only one trap
    // is in flight at a time on this hart.
    frame.trap_stack = trap::trap_stack_top();
    frame.hartid = 0;

    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    let proc = Process {
        frame,
        stack,
        pc: entry as *const () as usize,
        pid,
        state: ProcessState::Running,
    };

    let table = unsafe { TABLE.get() };
    for slot in table.slots.iter_mut() {
        if slot.is_none() {
            *slot = Some(proc);
            return Ok(pid);
        }
    }
    Err(SpawnError::TableFull)
}

/// Round-robin pick of the next runnable process.
///
/// `epc` is stashed as the resume PC for whichever process was last
/// running so it picks back up exactly where it was preempted.
/// Returns `None` if the table is empty — in that case the caller
/// (the trap handler) leaves `mscratch` alone and resumes whatever
/// was on the CPU before.
pub fn schedule(epc: usize) -> Option<(*mut TrapFrame, usize)> {
    let table = unsafe { TABLE.get() };

    if let Some(idx) = table.current
        && let Some(proc) = table.slots[idx].as_mut()
        && proc.state == ProcessState::Running
    {
        proc.pc = epc;
    }

    let n = table.slots.len();
    let start = table.current.map_or(0, |i| (i + 1) % n);
    for offset in 0..n {
        let i = (start + offset) % n;
        if let Some(proc) = table.slots[i].as_mut()
            && proc.state == ProcessState::Running
        {
            table.current = Some(i);
            return Some((&mut proc.frame as *mut _, proc.pc));
        }
    }

    table.current = None;
    None
}

/// Mark the current process Dead and free its slot. The next call to
/// [`schedule`] will skip it.
fn exit_current() -> Option<u16> {
    let table = unsafe { TABLE.get() };
    let idx = table.current?;
    let pid = table.slots[idx].as_ref()?.pid;
    table.slots[idx] = None;
    table.current = None;
    Some(pid)
}

/// Handle a synchronous `ecall`. The convention follows the blog: a0
/// holds the syscall number, return value goes back in a0.
pub fn do_syscall(epc: usize, frame: &mut TrapFrame) -> usize {
    let num = frame.regs[10]; // a0
    match num {
        1 => println!("test syscall"),
        93 => {
            // Exit: drop the current process and pull the next one in
            // *immediately*, otherwise the trap vector would resume
            // execution at the now-orphaned PC.
            if let Some(pid) = exit_current() {
                println!("process {} exited", pid);
            }
            if let Some((next_frame, next_pc)) = schedule(epc) {
                unsafe { asm!("csrw mscratch, {}", in(reg) next_frame as usize) };
                return next_pc;
            }
            // No remaining processes — park here. mret would otherwise
            // jump to a bogus PC.
            loop {
                unsafe { asm!("wfi") };
            }
        }
        _ => println!("unknown syscall {}", num),
    }
    // ecall is always 4 bytes (no compressed form), so unconditionally
    // step past it to avoid re-trapping.
    epc + 4
}
