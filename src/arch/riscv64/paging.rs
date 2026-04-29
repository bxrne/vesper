use core::arch::asm;

/// Pack a level-2 root table address into a Sv39 `satp` value. Mode 8 in
/// the top nibble selects Sv39; the PPN field holds the table address
/// shifted right by the 4 KiB page order.
#[inline]
pub fn make_satp_sv39(root_table_addr: usize) -> usize {
    let root_ppn = root_table_addr >> 12;
    (8usize << 60) | root_ppn
}

/// Switch on Sv39 paging and drop into supervisor mode at `s_entry`.
///
/// The dance is: write `satp`, flush stale TLB entries, point `mepc` at
/// the supervisor entry, set `mstatus.MPP = S` (with `MPIE = 1` so the
/// previous interrupt-enable is restored on `mret`), then `mret`.
///
/// # Safety
///
/// - `satp_val` must encode a valid level-2 page table that contains an
///   identity mapping for the code that runs immediately after `mret`,
///   otherwise the very next instruction fetch faults.
/// - `s_entry` must be a `!`-returning function — there is no path back.
pub unsafe fn enter_sv39(satp_val: usize, s_entry: extern "C" fn() -> !) -> ! {
    let s_entry_addr = s_entry as usize;
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma x0, x0",
            "csrw mepc, {mepc}",
            // MPP=01 (Supervisor) | MPIE=1. MIE stays 0 — interrupts
            // remain off until a real trap handler is installed.
            "li   t0, (1 << 11) | (1 << 7)",
            "csrw mstatus, t0",
            "mret",
            satp = in(reg) satp_val,
            mepc = in(reg) s_entry_addr,
            options(noreturn)
        )
    }
}
