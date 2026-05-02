//! Shared on-disk constants for the Minix 3 reader.

/// Minix 3 fixed block size.
pub const BLOCK_SIZE: u32 = 1024;
/// Magic value that identifies a Minix 3 v3 superblock.
pub const MAGIC: u16 = 0x4d5a;
/// Inode #1 is always the root directory.
pub const ROOT_INODE: u32 = 1;
/// Direct zone pointers stored in the inode itself.
pub const NUM_DIRECT_ZONES: usize = 7;
/// One u32-pointer per 4 bytes of an indirect block.
pub const INDIRECT_PTRS_PER_BLOCK: usize = (BLOCK_SIZE / 4) as usize;
/// Filename limit set by `DirEntry::name`.
pub const MAX_NAME_LEN: usize = 60;
/// Bytes per `DirEntry`.
pub const DIRENT_SIZE: usize = 64;
