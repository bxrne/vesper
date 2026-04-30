//! Tiny RAII page-aligned scratch buffer for filesystem block I/O.
//!
//! The block driver insists on whole-sector reads, and most callers
//! here only need a transient 1 KiB scratch area per request. Wrapping
//! the page-frame allocation in a struct with `Drop` means we can hand
//! one to a fallible function (`?`-style) without leaking on early
//! return.

use core::ptr::NonNull;

use crate::mm::alloc::page_frame::{PAGE_SIZE, allocate, deallocate};

pub struct Buffer {
    ptr: NonNull<u8>,
    pages: usize,
}

impl Buffer {
    /// Allocate at least `bytes` of page-aligned scratch. Rounds up to
    /// whole pages because the underlying allocator is page-granular.
    pub fn new(bytes: usize) -> Option<Self> {
        let pages = bytes.div_ceil(PAGE_SIZE).max(1);
        let ptr = allocate(pages)?;
        Some(Self { ptr, pages })
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // SAFETY: `self.ptr` came from `allocate(self.pages)` and isn't
        // freed elsewhere (the type doesn't expose ownership transfer).
        let _ = self.pages;
        unsafe { deallocate(self.ptr) };
    }
}
