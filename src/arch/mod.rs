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
