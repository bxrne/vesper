use core::arch::global_asm;

pub mod asm;
pub mod paging;
pub mod trap;
pub mod trap_frame;

// `_start` (M-mode entry) lives in boot.S and `mret`s into `kinit`.
global_asm!(include_str!("asm/boot.S"));
// M-mode trap vector that saves context, calls `m_trap`, restores, mret.
global_asm!(include_str!("asm/trap.S"));
