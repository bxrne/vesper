use crate::page::{Entry, PAGE_SIZE, PteFlags, Table, deallocate, zallocate};
use core::arch::asm;
use core::ptr::NonNull;

pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: i64, level: usize) {
    assert!(bits & 0xe != 0); // make sure R|W|E were provided

    // extract the VPN from the virtual address. The VPN is the index into the page tables at each level, so we need to split it into three 9-bit chunks.
    let vpn = [
        // 20-12
        (vaddr >> 12) & 0x1ff,
        // 29-21
        (vaddr >> 21) & 0x1ff,
        // 38-30
        (vaddr >> 30) & 0x1ff,
    ];

    // extract the physical address numbers
    let ppn = [
        // 20-12
        (paddr >> 12) & 0x1ff,
        // 29-21
        (paddr >> 21) & 0x1ff,
        // 55-30
        (paddr >> 30) & 0x3ffffff, // stores 26 bits instead of 9
    ];

    let mut v = &mut root.entries[vpn[2]];
    for i in (level..2).rev() {
        if v.is_invalid() {
            let page = zallocate(1);
            match page {
                Some(p) => {
                    let addr = p.as_ptr() as usize as i64;
                    v.set_entry((addr >> 2) | PteFlags::Valid.bits())
                }
                None => panic!("failed to allocate page for page table"),
            }
        }
        let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
        v = unsafe { entry.add(vpn[i]).as_mut().unwrap() };
    }
    // When we get here, we should be at VPN[0] and v should be pointing to
    // our entry.
    let entry = (ppn[2] << 28) as i64 |   // PPN[2] = [53:28]
			(ppn[1] << 19) as i64 |   // PPN[1] = [27:19]
			(ppn[0] << 10) as i64 |   // PPN[0] = [18:10]
			bits |                    // Specified bits, such as User, Read, Write, etc
			PteFlags::Valid.bits(); // Valid bit
    v.set_entry(entry);
}

pub fn unmap(root: &mut Table) {
    fn pte_to_addr(pte: i64) -> usize {
        ((pte & !0x3ff) << 2) as usize
    }

    unsafe fn deallocate_addr(addr: usize) {
        let ptr = NonNull::new(addr as *mut u8).expect("page table address was null");
        unsafe { deallocate(ptr) };
    }

    // Start with level 2.
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

            // Level 0 is a leaf-only table; free the page holding that table.
            let memaddr_lv0 = pte_to_addr(entry_lv1.get_entry());
            unsafe { deallocate_addr(memaddr_lv0) };
            entry_lv1.set_entry(0);
        }

        unsafe { deallocate_addr(memaddr_lv1) };
        entry_lv2.set_entry(0);
    }
}

pub fn v2p(root: &Table, vaddr: usize) -> Option<usize> {
    let vpn = [
        // 20-12
        (vaddr >> 12) & 0x1ff,
        // 29-21
        (vaddr >> 21) & 0x1ff,
        // 38-30
        (vaddr >> 30) & 0x1ff,
    ];

    let mut v = &root.entries[vpn[2]];
    for i in (0..=2).rev() {
        if v.is_invalid() {
            break; // page fault
        } else if v.is_leaf() {
            // leaves can be at any level
            let off_mask = (1 << (12 + 9 * i)) - 1; // mask for the offset bits at this level
            let vaddr_pgoff = vaddr & off_mask; // offset within the page
            let addr = ((v.get_entry() << 2) as usize) & !off_mask;
            return Some(addr | vaddr_pgoff);
        }
        let entry = ((v.get_entry() & !0x3ff) << 2) as *const Entry;
        if i == 0 {
            break;
        }
        v = unsafe { entry.add(vpn[i - 1]).as_ref().unwrap() };
    }
    None
}

pub fn id_map_range(root: &mut Table, start: usize, end: usize, bits: i64) {
    assert!(start.is_multiple_of(PAGE_SIZE));
    assert!(end.is_multiple_of(PAGE_SIZE));
    for addr in (start..end).step_by(PAGE_SIZE) {
        map(root, addr, addr, bits, 0);
    }
}

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
