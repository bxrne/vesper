//! Thin architecture façade. The rest of the kernel imports from here
//! so swapping in another ISA later only requires adding a new
//! `cfg(target_arch = ...)` module.

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64 as arch_impl;

#[inline]
pub fn wfi() {
    arch_impl::asm::wfi()
}

#[inline]
pub fn enter_sv39(satp_val: usize, s_entry: extern "C" fn() -> !) -> ! {
    unsafe { arch_impl::paging::enter_sv39(satp_val, s_entry) }
}

#[inline]
pub fn make_satp_sv39(root_table_addr: usize) -> usize {
    arch_impl::paging::make_satp_sv39(root_table_addr)
}

/// Install the M-mode trap vector and back it with a dedicated stack.
/// Call before [`enable_interrupts`] so the first interrupt finds a
/// valid `mscratch` / `mtvec`.
#[inline]
pub fn install_trap_handler(satp: usize) {
    unsafe { arch_impl::trap::install(satp) }
}

/// Unmask the machine timer and external interrupts. After this, the
/// CPU may take a trap at any time — make sure everything else
/// (page tables, PLIC, etc.) is already configured.
#[inline]
pub fn enable_interrupts() {
    arch_impl::trap::enable_interrupts()
}
