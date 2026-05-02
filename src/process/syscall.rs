//! `ecall` dispatcher.
//!
//! Syscall numbers track libgloss / newlib so userspace can target the
//! same ABI Stephen Marz's tutorial does:
//!
//! * 1  — debug ping (no-op, prints nothing).
//! * 63 — `read(dev, inode, buf, size, offset)`. Synchronous Minix 3
//!   file read; returns the byte count or 0 on failure.
//! * 93 — `exit`. Drops the current process and reschedules
//!   immediately so we don't `mret` to a freed PC.

use core::arch::asm;

use crate::arch::riscv64::trap_frame::TrapFrame;
use crate::println;

use super::table::{exit_current, schedule};

/// Handle a synchronous `ecall`. The convention follows the blog: a0
/// holds the syscall number, return value goes back in a0.
pub fn do_syscall(epc: usize, frame: &mut TrapFrame) -> usize {
    let num = frame.regs[10]; // a0
    match num {
        1 => {}
        63 => {
            // a1=dev, a2=inode, a3=buffer, a4=size, a5=offset
            let dev = frame.regs[11];
            let inode = frame.regs[12] as u32;
            let buffer = frame.regs[13] as *mut u8;
            let size = frame.regs[14] as u32;
            let offset = frame.regs[15] as u32;
            frame.regs[10] = match crate::fs::minix3::Fs::mount(dev) {
                Ok(fs) => match fs.read_inode(inode) {
                    Ok(node) => {
                        let slice =
                            unsafe { core::slice::from_raw_parts_mut(buffer, size as usize) };
                        fs.read_file(&node, offset, slice).unwrap_or(0)
                    }
                    Err(_) => 0,
                },
                Err(_) => 0,
            };
        }
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
