use core::arch::global_asm;

pub mod asm;
pub mod paging;

// Pull in the RISC-V assembly entry point. `_start` lives in boot.S and `mret`s into `kmain`.
global_asm!(include_str!("asm/boot.S"));
