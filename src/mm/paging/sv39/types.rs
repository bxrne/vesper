//! Sv39 page table primitives.

use core::ops::{BitOr, BitOrAssign};

/// 512-entry page table — exactly one 4 KiB page so a table aligns
/// cleanly to a frame boundary.
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

/// Sv39 page table entry. Stored as `i64` because the walker uses
/// arithmetic shifts when sign-extending PPN bits — staying signed
/// keeps the masks consistent.
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
        (self.get_entry() & PteFlags::VALID.bits()) != 0
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    /// In Sv39 a PTE with any of R/W/X set is a leaf; otherwise it
    /// points at the next-level table.
    #[inline]
    pub fn is_leaf(&self) -> bool {
        (self.get_entry() & PteFlags::RWX.bits()) != 0
    }

    #[inline]
    pub fn is_branch(&self) -> bool {
        !self.is_leaf()
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct PteFlags(i64);

impl PteFlags {
    pub const EMPTY: Self = Self(0);

    pub const VALID: Self = Self(1 << 0);
    pub const READ: Self = Self(1 << 1);
    pub const WRITE: Self = Self(1 << 2);
    pub const EXECUTE: Self = Self(1 << 3);
    pub const USER: Self = Self(1 << 4);
    pub const GLOBAL: Self = Self(1 << 5);
    pub const ACCESSED: Self = Self(1 << 6);
    pub const DIRTY: Self = Self(1 << 7);

    /// Combined R|W|X mask — testing a PTE against this is the
    /// canonical leaf check.
    pub const RWX: Self = Self(0xe);

    #[inline]
    pub const fn bits(self) -> i64 {
        self.0
    }
}

impl BitOr for PteFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PteFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
