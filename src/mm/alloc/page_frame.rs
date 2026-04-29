//! First-fit page-frame allocator.
//!
//! Layout: a one-byte descriptor per data page lives at the start of
//! the heap, followed by page-aligned data pages. Storing metadata
//! out-of-band (instead of in each free page) means a freshly-allocated
//! page is immediately usable without clearing a header.

use crate::linker;
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_ORDER;

/// Address of the first data page. Set by [`init`]; left at 0 to make
/// "allocator used before init" trip an obvious null deref.
static ALLOC_START: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn alloc_start() -> usize {
    ALLOC_START.load(Ordering::Relaxed)
}

#[repr(transparent)]
struct PageDesc(u8);

impl PageDesc {
    const TAKEN: u8 = 1 << 0;
    /// Marks the last page in an allocation so [`deallocate`] knows
    /// where the run ends without storing an explicit length.
    const LAST: u8 = 1 << 1;

    #[inline]
    fn is_taken(&self) -> bool {
        self.0 & Self::TAKEN != 0
    }
    #[inline]
    fn is_last(&self) -> bool {
        self.0 & Self::LAST != 0
    }
    #[inline]
    fn clear(&mut self) {
        self.0 = 0;
    }
    #[inline]
    fn mark_taken(&mut self) {
        self.0 |= Self::TAKEN;
    }
    #[inline]
    fn mark_last(&mut self) {
        self.0 |= Self::LAST;
    }
}

/// # Safety
///
/// The returned slice aliases the entire descriptor table. Callers must
/// not hold two `&mut` views at once; the allocator is single-threaded
/// for now, which makes this safe in practice.
unsafe fn descriptors<'a>() -> &'a mut [PageDesc] {
    let heap_start = linker::heap_start();
    let n = linker::heap_size() / PAGE_SIZE;
    unsafe { slice::from_raw_parts_mut(heap_start as *mut PageDesc, n) }
}

/// Initialise the allocator. Must run exactly once, before any
/// allocation, while no other code is using the heap region.
pub fn init() {
    let table = unsafe { descriptors() };
    for p in table.iter_mut() {
        p.clear();
    }
    // The descriptor table sits at the head of the heap, so the first
    // usable data page starts after it — rounded up to keep alignment.
    let heap_start = linker::heap_start();
    ALLOC_START.store(
        (heap_start + table.len()).next_multiple_of(PAGE_SIZE),
        Ordering::Relaxed,
    );
}

/// Allocate a contiguous run of `pages` 4 KiB pages, or `None` on OOM.
pub fn allocate(pages: usize) -> Option<NonNull<u8>> {
    assert!(pages > 0);
    let table = unsafe { descriptors() };

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

/// Allocate and zero a contiguous run. Page tables, in particular,
/// must start out zero so unused entries read as invalid.
pub fn zallocate(pages: usize) -> Option<NonNull<u8>> {
    let p = allocate(pages)?;
    // 64-bit stores zero 8× as much memory per instruction as bytes.
    let words = (PAGE_SIZE * pages) / 8;
    unsafe { slice::from_raw_parts_mut(p.as_ptr().cast::<u64>(), words).fill(0) };
    Some(p)
}

/// # Safety
///
/// `ptr` must be the start of a run currently held by the caller and
/// not already freed; double-free trips the `LAST` assertion below.
pub unsafe fn deallocate(ptr: NonNull<u8>) {
    let addr = ptr.as_ptr() as usize;
    let start = alloc_start();
    let heap_end = linker::heap_start() + linker::heap_size();
    assert!(addr >= start && addr < heap_end);

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
