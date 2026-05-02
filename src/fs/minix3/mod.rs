//! Minix 3 filesystem reader.
//!
//! On-disk layout (one 1 KiB block each unless noted):
//!
//! ```text
//!   block 0: boot block (unused by us)
//!   block 1: SuperBlock
//!   block 2..: inode bitmap (imap_blocks)
//!              zone  bitmap (zmap_blocks)
//!              inode table  (ceil(ninodes * sizeof(Inode) / BLOCK_SIZE))
//!              first_data_zone..zones: file/zone data
//! ```
//!
//! Reads are issued through the synchronous block driver (chapter 9),
//! so this module never needs the watcher/sleep machinery from the
//! tutorial — every `block_read` returns once the device IRQ fires.

pub mod blocks;
mod consts;
mod dir_entry;
mod error;
mod fs;
mod inode;
mod super_block;

pub use consts::{
    BLOCK_SIZE, DIRENT_SIZE, INDIRECT_PTRS_PER_BLOCK, MAGIC, MAX_NAME_LEN, NUM_DIRECT_ZONES,
    ROOT_INODE,
};
pub use dir_entry::DirEntry;
pub use error::FsError;
pub use fs::Fs;
pub use inode::{Inode, mode};
pub use super_block::SuperBlock;
