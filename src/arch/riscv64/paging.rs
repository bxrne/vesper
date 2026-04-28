use core::arch::asm;

#[inline]
pub fn make_satp_sv39(root_table_addr: usize) -> usize {
    let root_ppn = root_table_addr >> 12;
    (8usize << 60) | root_ppn
}

/// Enter supervisor mode with Sv39 paging enabled.
///
/// # Safety
///
/// - `satp_val` must point at a valid level-2 page table (physical address in PPN field).
/// - `s_entry` must be a valid supervisor-mode entry point.
pub unsafe fn enter_sv39(satp_val: usize, s_entry: extern "C" fn() -> !) -> ! {
    let s_entry_addr = s_entry as usize;
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma x0, x0",
            "csrw mepc, {mepc}",
            // MPP=01 (Supervisor), MPIE=1. Leave MIE=0 for now.
            "li   t0, (1 << 11) | (1 << 7)",
            "csrw mstatus, t0",
            "mret",
            satp = in(reg) satp_val,
            mepc = in(reg) s_entry_addr,
            options(noreturn)
        )
    }
}
