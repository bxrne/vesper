use core::ops::{BitOr, BitOrAssign};
use core::ptr::NonNull;

#[repr(C)]
pub struct Table {
    pub entries: [Entry; 512],
}

impl Table {
    #[inline]
    pub const fn len() -> usize {
        512
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> NonNull<Self> {
        NonNull::from(self)
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
        (self.get_entry() & PteFlags::VALID.bits()) != 0
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    // A leaf has one or more RWX bits set.
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

    /// Mask of the RWX bits (leaf indicator).
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
