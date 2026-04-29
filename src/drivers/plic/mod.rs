//! Platform-Level Interrupt Controller (PLIC) driver for the QEMU
//! `virt` machine.
//!
//! All external device interrupts (UART, virtio, PCIe, ...) funnel
//! through the PLIC and arrive at the hart as a single
//! `Machine External` cause. The handler then asks the PLIC which
//! source actually fired (claim) and acknowledges it (complete).
//!
//! The QEMU virt PLIC exposes a separate "context" per (hart, privilege)
//! pair. Traps are taken in M-mode here, so the M-mode context for
//! hart 0 is hard-coded.

use core::ptr::{read_volatile, write_volatile};

const PLIC_PRIORITY: usize = 0x0c00_0000;
const PLIC_ENABLE_HART0_M: usize = 0x0c00_2000;
const PLIC_THRESHOLD_HART0_M: usize = 0x0c20_0000;
// On the QEMU virt PLIC the same word serves as the claim register on
// read and the complete register on write.
const PLIC_CLAIM_HART0_M: usize = 0x0c20_0004;

/// Source ID of the NS16550A UART on the QEMU virt machine
/// (`qemu/include/hw/riscv/virt.h`).
pub const UART0_IRQ: u32 = 10;

/// Allow source `id` to interrupt this context. Source 0 is hard-wired
/// to "no interrupt" so the bit position equals the source id.
pub fn enable(id: u32) {
    let enables = PLIC_ENABLE_HART0_M as *mut u32;
    unsafe {
        let prev = read_volatile(enables);
        write_volatile(enables, prev | (1 << id));
    }
}

/// Set the priority for source `id`. Valid values are 0–7; 0 disables
/// the source regardless of the enable bit.
pub fn set_priority(id: u32, prio: u8) {
    let prio_reg = PLIC_PRIORITY as *mut u32;
    unsafe { write_volatile(prio_reg.add(id as usize), u32::from(prio) & 7) }
}

/// Mask any interrupt whose priority is `<= tsh`. A threshold of 0 lets
/// every enabled source through.
pub fn set_threshold(tsh: u8) {
    let tsh_reg = PLIC_THRESHOLD_HART0_M as *mut u32;
    unsafe { write_volatile(tsh_reg, u32::from(tsh) & 7) }
}

/// Claim the highest-priority pending interrupt. Returns `None` only on
/// a spurious external trap (PLIC reports source 0).
pub fn next() -> Option<u32> {
    let claim_reg = PLIC_CLAIM_HART0_M as *const u32;
    let claim = unsafe { read_volatile(claim_reg) };
    if claim == 0 { None } else { Some(claim) }
}

/// Tell the PLIC the given source has been serviced; without this the
/// PLIC keeps the source masked and no further interrupts arrive from it.
pub fn complete(id: u32) {
    let complete_reg = PLIC_CLAIM_HART0_M as *mut u32;
    unsafe { write_volatile(complete_reg, id) }
}
