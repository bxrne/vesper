//! Index node — describes a single file's metadata and zone pointers.

/// Inode mode bits we care about (octal).
pub mod mode {
    pub const TYPE_MASK: u16 = 0o170000;
    pub const REGULAR: u16 = 0o100000;
    pub const DIRECTORY: u16 = 0o040000;
}

/// 13 zones: 7 direct, 1 single-indirect, 1 double-indirect, 1
/// triple-indirect, plus 3 reserved that we never follow.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Inode {
    pub mode: u16,
    pub nlinks: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub atime: u32,
    pub mtime: u32,
    pub ctime: u32,
    pub zones: [u32; 10],
}

impl Inode {
    #[inline]
    pub fn is_dir(&self) -> bool {
        (self.mode & mode::TYPE_MASK) == mode::DIRECTORY
    }

    #[inline]
    pub fn is_regular(&self) -> bool {
        (self.mode & mode::TYPE_MASK) == mode::REGULAR
    }
}
