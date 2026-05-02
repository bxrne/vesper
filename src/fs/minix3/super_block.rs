//! Minix 3 superblock — block #1 of the filesystem.

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SuperBlock {
    pub ninodes: u32,
    pub pad0: u16,
    pub imap_blocks: u16,
    pub zmap_blocks: u16,
    pub first_data_zone: u16,
    pub log_zone_size: u16,
    pub pad1: u16,
    pub max_size: u32,
    pub zones: u32,
    pub magic: u16,
    pub pad2: u16,
    pub block_size: u16,
    pub disk_version: u8,
}
