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

mod process;
mod syscall;
mod table;

pub use process::{Process, ProcessState};
pub use syscall::do_syscall;
pub use table::{SpawnError, schedule, spawn_kernel};
