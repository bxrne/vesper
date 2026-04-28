//! Naive byte-per-page allocator.

use core::ptr::{NonNull, addr_of};
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering};

unsafe extern "C" {
    static _heap_start: usize;
    static _heap_size: usize;
}

pub const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_ORDER; // 4 KiB

/// Address of the first page-aligned data page. Set by `init`.
static ALLOC_START: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn alloc_start() -> usize {
    ALLOC_START.load(Ordering::Relaxed)
}

/// One byte of allocator metadata per data page.
#[repr(transparent)]
struct PageDesc(u8);

impl PageDesc {
    pub const TAKEN: u8 = 1 << 0;
    pub const LAST: u8 = 1 << 1;

    #[inline]
    pub fn is_taken(&self) -> bool {
        self.0 & Self::TAKEN != 0
    }
    #[inline]
    pub fn is_last(&self) -> bool {
        self.0 & Self::LAST != 0
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    #[inline]
    pub fn mark_taken(&mut self) {
        self.0 |= Self::TAKEN;
    }
    #[inline]
    pub fn mark_last(&mut self) {
        self.0 |= Self::LAST;
    }
}

// `_heap_start` and `_heap_size` are addresses baked in by the linker
// script, we have to take their address, not their value.
#[inline]
fn heap_start() -> usize {
    addr_of!(_heap_start) as usize
}
#[inline]
fn heap_size() -> usize {
    addr_of!(_heap_size) as usize
}

/// Borrow the descriptor table as a slice. Each entry is one byte and
/// describes the data page at `alloc_start() + i * PAGE_SIZE`.
///
/// Callers must not hand out overlapping `&mut` slices to the table.
unsafe fn descriptors<'a>() -> &'a mut [PageDesc] {
    let n = heap_size() / PAGE_SIZE;
    unsafe { slice::from_raw_parts_mut(heap_start() as *mut PageDesc, n) }
}

/// Initialise the allocator. Must be called once, before any
/// allocate / deallocate, while we still own the heap.
pub fn init() {
    let table = unsafe { descriptors() };
    for p in table.iter_mut() {
        p.clear();
    }
    // Reserve room for the descriptor table at the front of the heap,
    // then round up so the data pages are themselves page-aligned.
    ALLOC_START.store(
        (heap_start() + table.len()).next_multiple_of(PAGE_SIZE),
        Ordering::Relaxed,
    );
}

/// Allocate a contiguous run of `pages` 4 KiB pages.
///
/// Returns `None` on OOM.
pub fn allocate(pages: usize) -> Option<NonNull<u8>> {
    assert!(pages > 0);
    let table = unsafe { descriptors() };

    // First-fit: find the first window where every descriptor is free.
    let start_idx = table
        .windows(pages)
        .position(|w| w.iter().all(|p| !p.is_taken()))?;

    let run = &mut table[start_idx..start_idx + pages];
    for p in run.iter_mut() {
        p.mark_taken();
    }
    run[pages - 1].mark_last();

    let ptr = (alloc_start() + start_idx * PAGE_SIZE) as *mut u8;
    NonNull::new(ptr)
}

/// Allocate and zero a contiguous run of pages.
pub fn zallocate(pages: usize) -> Option<NonNull<u8>> {
    let p = allocate(pages)?;
    // Word-sized stores -> 8× fewer instructions than zeroing byte-by-byte.
    let words = (PAGE_SIZE * pages) / 8;
    unsafe { slice::from_raw_parts_mut(p.as_ptr().cast::<u64>(), words).fill(0) };
    Some(p)
}

/// Free a previously-allocated run.
///
/// `ptr` must be a pointer returned by `allocate` or `zallocate` that
/// has not already been freed.
///
/// # Safety
///
/// The caller must ensure `ptr` was returned by `allocate`/`zallocate`, is the
/// start of a currently-allocated run, and has not been freed already.
pub unsafe fn deallocate(ptr: NonNull<u8>) {
    let addr = ptr.as_ptr() as usize;
    let start = alloc_start();
    assert!(addr >= start && addr < heap_start() + heap_size());

    let table = unsafe { descriptors() };
    let mut i = (addr - start) / PAGE_SIZE;

    while table[i].is_taken() && !table[i].is_last() {
        table[i].clear();
        i += 1;
    }
    assert!(
        table[i].is_taken() && table[i].is_last(),
        "deallocate: not the start of a run (possible double free)"
    );
    table[i].clear();
}

// === Sv39 page table structures ===

#[repr(C)]
pub struct Table {
    pub entries: [Entry; 512],
}

impl Table {
    #[inline]
    pub const fn len() -> usize {
        512
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Entry {
    entry: i64,
}

impl Entry {
    #[inline]
    pub fn get_entry(&self) -> i64 {
        self.entry
    }

    #[inline]
    pub fn set_entry(&mut self, entry: i64) {
        self.entry = entry;
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        (self.get_entry() & PteFlags::Valid.bits()) != 0
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    // A leaf has one or more RWX bits set.
    #[inline]
    pub fn is_leaf(&self) -> bool {
        (self.get_entry() & PTE_RWX_MASK) != 0
    }

    #[inline]
    pub fn is_branch(&self) -> bool {
        !self.is_leaf()
    }
}

#[repr(i64)]
pub enum PteFlags {
    Valid = 1 << 0,
    Read = 1 << 1,
    Write = 1 << 2,
    Execute = 1 << 3,
    User = 1 << 4,
    Global = 1 << 5,
    Accessed = 1 << 6,
    Dirty = 1 << 7,
}

impl PteFlags {
    #[inline]
    pub const fn bits(self) -> i64 {
        self as i64
    }
}

pub const PTE_RWX_MASK: i64 = 0xe;
