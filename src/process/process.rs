//! Per-process state.

use core::ptr::NonNull;

use crate::arch::riscv64::trap_frame::TrapFrame;

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
