//! TrapFrame layout shared between Rust and the M-mode trap vector.
//!
//! Field ordering and offsets are load-bearing: the trap vector saves
//! and restores registers using hard-coded byte offsets into this
//! struct, so any change here must mirror the asm in `asm/trap.S`.

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
    pub regs: [usize; 32],   // offset   0
    pub fregs: [usize; 32],  // offset 256
    pub satp: usize,         // offset 512
    pub trap_stack: *mut u8, // offset 520
    pub hartid: usize,       // offset 528
}

impl TrapFrame {
    pub const fn empty() -> Self {
        Self {
            regs: [0; 32],
            fregs: [0; 32],
            satp: 0,
            trap_stack: core::ptr::null_mut(),
            hartid: 0,
        }
    }
}
