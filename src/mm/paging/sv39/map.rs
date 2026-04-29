//! Sv39 page table walker, mapper, and inverse lookup.
//!
//! Sv39 virtual addresses are 39 bits split into three 9-bit VPNs plus
//! a 12-bit page offset. Each level of the walk indexes a 512-entry
//! table with the matching VPN slice, so a full walk is at most three
//! memory accesses.

use crate::mm::alloc::page_frame::{PAGE_SIZE, deallocate, zallocate};
use crate::mm::paging::sv39::types::{Entry, PteFlags, Table};
use core::ptr::NonNull;

/// Install a mapping `vaddr -> paddr` with the given permissions. The
/// leaf can sit at any level (0/4 KiB, 1/2 MiB, 2/1 GiB); `level`
/// selects which.
pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: PteFlags, level: usize) {
    // A leaf without R, W, or X is a "pointer" PTE in Sv39 — refusing
    // here avoids silently producing an unwalkable hierarchy.
    assert!(bits.bits() & PteFlags::RWX.bits() != 0);

    let vpn = [
        (vaddr >> 12) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
    ];

    // PPN[2] is wider than the others (26 bits) because Sv39 supports
    // up to 56-bit physical addresses.
    let ppn = [
        (paddr >> 12) & 0x1ff,
        (paddr >> 21) & 0x1ff,
        (paddr >> 30) & 0x3ff_ffff,
    ];

    let mut v = &mut root.entries[vpn[2]];
    for i in (level..2).rev() {
        if v.is_invalid() {
            // Allocate the next-level table on demand. Zeroing matters:
            // unused entries must read as invalid (V=0).
            let page = zallocate(1).expect("failed to allocate page for page table");
            let addr = page.as_ptr() as usize as i64;
            v.set_entry((addr >> 2) | PteFlags::VALID.bits());
        }
        // PPN in the PTE is shifted left by 10; the address of the next
        // table is therefore `(entry & !flags) << 2`.
        let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
        v = unsafe { entry.add(vpn[i]).as_mut().unwrap() };
    }

    // Layout of a leaf PTE (Sv39):
    //   [53:28] PPN[2]  [27:19] PPN[1]  [18:10] PPN[0]  [9:0] flags
    let entry = (ppn[2] << 28) as i64
        | (ppn[1] << 19) as i64
        | (ppn[0] << 10) as i64
        | bits.bits()
        | PteFlags::VALID.bits();
    v.set_entry(entry);
}

/// Walk the tree and free every non-leaf table. Leaves are left alone
/// because they describe physical frames the caller still owns.
pub fn unmap(root: &mut Table) {
    fn pte_to_addr(pte: i64) -> usize {
        ((pte & !0x3ff) << 2) as usize
    }

    unsafe fn deallocate_addr(addr: usize) {
        let ptr = NonNull::new(addr as *mut u8).expect("page table address was null");
        unsafe { deallocate(ptr) };
    }

    for lv2 in 0..Table::len() {
        let entry_lv2 = &mut root.entries[lv2];
        if !entry_lv2.is_valid() || !entry_lv2.is_branch() {
            continue;
        }

        let memaddr_lv1 = pte_to_addr(entry_lv2.get_entry());
        let table_lv1 = unsafe { &mut *(memaddr_lv1 as *mut Table) };

        for lv1 in 0..Table::len() {
            let entry_lv1 = &mut table_lv1.entries[lv1];
            if !entry_lv1.is_valid() || !entry_lv1.is_branch() {
                continue;
            }

            // Level 0 holds only leaves, so the branch at level 1
            // points at a frame whose contents are page-table data.
            let memaddr_lv0 = pte_to_addr(entry_lv1.get_entry());
            unsafe { deallocate_addr(memaddr_lv0) };
            entry_lv1.set_entry(0);
        }

        unsafe { deallocate_addr(memaddr_lv1) };
        entry_lv2.set_entry(0);
    }
}

/// Software MMU walk: resolve `vaddr` to its physical address, or
/// `None` on a missing/invalid mapping. Useful for sanity-checking
/// page-table construction without actually flipping `satp`.
pub fn v2p(root: &Table, vaddr: usize) -> Option<usize> {
    let vpn = [
        (vaddr >> 12) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
    ];

    let mut v = &root.entries[vpn[2]];
    for i in (0..=2).rev() {
        if v.is_invalid() {
            return None;
        }
        if v.is_leaf() {
            // Superpages: the offset bits grow by 9 per level.
            let off_mask = (1 << (12 + 9 * i)) - 1;
            let vaddr_pgoff = vaddr & off_mask;
            let addr = ((v.get_entry() << 2) as usize) & !off_mask;
            return Some(addr | vaddr_pgoff);
        }
        if i == 0 {
            // Branch PTE at level 0 is malformed (no level -1 to walk).
            break;
        }
        let entry = ((v.get_entry() & !0x3ff) << 2) as *const Entry;
        v = unsafe { entry.add(vpn[i - 1]).as_ref().unwrap() };
    }
    None
}

/// Identity-map every page in `[start, end)`. Both bounds must already
/// be page-aligned — the caller usually rounds linker symbols outward.
pub fn id_map_range(root: &mut Table, start: usize, end: usize, bits: PteFlags) {
    assert!(start.is_multiple_of(PAGE_SIZE));
    assert!(end.is_multiple_of(PAGE_SIZE));
    for addr in (start..end).step_by(PAGE_SIZE) {
        map(root, addr, addr, bits, 0);
    }
}
