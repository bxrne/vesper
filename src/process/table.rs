//! Static process table, PID allocator, and the round-robin scheduler.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::arch::riscv64::trap;
use crate::arch::riscv64::trap_frame::TrapFrame;
use crate::mm::alloc::page_frame::{self, PAGE_SIZE};

use super::process::{Process, ProcessState};

/// Per-process kernel stack size. 8 KiB is plenty for the toy
/// workloads here and keeps the demo cheap.
const STACK_PAGES: usize = 2;
/// Hard cap on concurrent processes. Static array avoids needing a
/// general-purpose allocator at this stage.
const MAX_PROCESSES: usize = 16;

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
pub(super) fn exit_current() -> Option<u16> {
    let table = unsafe { TABLE.get() };
    let idx = table.current?;
    let pid = table.slots[idx].as_ref()?.pid;
    table.slots[idx] = None;
    table.current = None;
    Some(pid)
}
