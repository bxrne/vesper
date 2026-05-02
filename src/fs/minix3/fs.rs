//! `Fs` — borrowed handle to a freshly-read superblock plus the
//! per-block scratch space we'll reuse for inode/data reads.

use crate::drivers::virtio;

use super::blocks;
use super::consts::{BLOCK_SIZE, INDIRECT_PTRS_PER_BLOCK, MAGIC, NUM_DIRECT_ZONES};
use super::error::FsError;
use super::inode::Inode;
use super::super_block::SuperBlock;

pub struct Fs {
    dev: usize,
    pub sb: SuperBlock,
}

impl Fs {
    /// Probe `dev` for a Minix 3 filesystem. Returns a usable handle
    /// only if the magic matches — otherwise the disk is treated as
    /// non-Minix and the caller can fall back to whatever it likes.
    pub fn mount(dev: usize) -> Result<Self, FsError> {
        let buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;
        // Superblock lives at byte offset 1024 (block #1). The block
        // driver only does 512-byte sectors, so we still ask for a
        // full 1 KiB even though SuperBlock itself is only ~32 bytes.
        if !virtio::device::block_read(dev, buf.as_mut_ptr(), BLOCK_SIZE, BLOCK_SIZE as u64) {
            return Err(FsError::BlockReadFailed);
        }
        let sb = unsafe { *(buf.as_ptr() as *const SuperBlock) };
        if sb.magic != MAGIC {
            return Err(FsError::BadMagic);
        }
        Ok(Self { dev, sb })
    }

    /// Byte offset of the start of the inode table on disk.
    fn inode_table_offset(&self) -> u64 {
        // boot(1) + super(1) + imap + zmap, all in 1 KiB blocks.
        let blocks_before = 2u64 + self.sb.imap_blocks as u64 + self.sb.zmap_blocks as u64;
        blocks_before * BLOCK_SIZE as u64
    }

    /// Read inode `num` (1-based, as stored in directory entries).
    pub fn read_inode(&self, num: u32) -> Result<Inode, FsError> {
        if num == 0 || num > self.sb.ninodes {
            return Err(FsError::InodeOutOfRange);
        }
        // Inode `num` sits at byte offset `(num - 1) * sizeof(Inode)`
        // into the inode table; we have to round down to a block
        // boundary because the driver only reads whole blocks.
        let inode_off =
            self.inode_table_offset() + (num as u64 - 1) * core::mem::size_of::<Inode>() as u64;
        let block_off = inode_off & !(BLOCK_SIZE as u64 - 1);
        let within = (inode_off - block_off) as usize;

        let buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;
        if !virtio::device::block_read(self.dev, buf.as_mut_ptr(), BLOCK_SIZE, block_off) {
            return Err(FsError::BlockReadFailed);
        }
        let ptr = unsafe { buf.as_ptr().add(within) as *const Inode };
        Ok(unsafe { *ptr })
    }

    /// Copy up to `out.len()` bytes of file `inode`'s contents starting
    /// at `offset`. Returns the number of bytes actually copied.
    ///
    /// Walks direct → single → double → triple indirect zones in order,
    /// stopping as soon as `out` is full or we run past `inode.size`.
    pub fn read_file(&self, inode: &Inode, offset: u32, out: &mut [u8]) -> Result<usize, FsError> {
        if offset >= inode.size {
            return Ok(0);
        }
        let want = core::cmp::min(out.len() as u32, inode.size - offset) as usize;
        if want == 0 {
            return Ok(0);
        }

        let mut written = 0usize;
        let mut cur_offset = offset as usize;
        let end = offset as usize + want;

        let block_buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;
        let ind_buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;
        let dind_buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;
        let tind_buf = blocks::Buffer::new(BLOCK_SIZE as usize).ok_or(FsError::OutOfMemory)?;

        let mut copy_zone = |zone: u32,
                             logical_byte_start: usize,
                             cur_offset: &mut usize,
                             written: &mut usize|
         -> Result<bool, FsError> {
            // A zero zone pointer means "this block has been freed";
            // tutorial calls these "skip" entries.
            if zone == 0 {
                *cur_offset = logical_byte_start + BLOCK_SIZE as usize;
                return Ok(*cur_offset < end);
            }
            let block_start = logical_byte_start;
            let block_end = block_start + BLOCK_SIZE as usize;
            // Skip zones entirely before the requested window.
            if block_end <= *cur_offset {
                return Ok(true);
            }
            if !virtio::device::block_read(
                self.dev,
                block_buf.as_mut_ptr(),
                BLOCK_SIZE,
                zone as u64 * BLOCK_SIZE as u64,
            ) {
                return Err(FsError::BlockReadFailed);
            }
            let in_block = *cur_offset - block_start;
            let chunk = core::cmp::min(BLOCK_SIZE as usize - in_block, end - *cur_offset);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    block_buf.as_ptr().add(in_block),
                    out.as_mut_ptr().add(*written),
                    chunk,
                );
            }
            *written += chunk;
            *cur_offset += chunk;
            Ok(*cur_offset < end)
        };

        // Direct zones cover the first NUM_DIRECT_ZONES * BLOCK_SIZE
        // bytes of the file with no indirection.
        let mut logical = 0usize;
        for zi in 0..NUM_DIRECT_ZONES {
            if cur_offset >= end {
                return Ok(written);
            }
            if !copy_zone(inode.zones[zi], logical, &mut cur_offset, &mut written)? {
                return Ok(written);
            }
            logical += BLOCK_SIZE as usize;
        }

        // Single indirect: zones[7] points to a block of u32 zone numbers.
        if cur_offset < end && inode.zones[NUM_DIRECT_ZONES] != 0 {
            self.read_indirect_zone(inode.zones[NUM_DIRECT_ZONES], &ind_buf)?;
            let ptrs = ind_buf.as_ptr() as *const u32;
            for i in 0..INDIRECT_PTRS_PER_BLOCK {
                if cur_offset >= end {
                    return Ok(written);
                }
                let z = unsafe { ptrs.add(i).read() };
                if !copy_zone(z, logical, &mut cur_offset, &mut written)? {
                    return Ok(written);
                }
                logical += BLOCK_SIZE as usize;
            }
        } else {
            // Skip the address range covered by the (missing) indirect block.
            logical += INDIRECT_PTRS_PER_BLOCK * BLOCK_SIZE as usize;
        }

        // Double indirect: zones[8] -> [block of u32] -> [block of u32] -> data.
        if cur_offset < end && inode.zones[NUM_DIRECT_ZONES + 1] != 0 {
            self.read_indirect_zone(inode.zones[NUM_DIRECT_ZONES + 1], &dind_buf)?;
            let dptrs = dind_buf.as_ptr() as *const u32;
            'dind: for i in 0..INDIRECT_PTRS_PER_BLOCK {
                let dz = unsafe { dptrs.add(i).read() };
                if dz == 0 {
                    logical += INDIRECT_PTRS_PER_BLOCK * BLOCK_SIZE as usize;
                    if cur_offset >= end {
                        break 'dind;
                    }
                    continue;
                }
                self.read_indirect_zone(dz, &ind_buf)?;
                let ptrs = ind_buf.as_ptr() as *const u32;
                for j in 0..INDIRECT_PTRS_PER_BLOCK {
                    if cur_offset >= end {
                        return Ok(written);
                    }
                    let z = unsafe { ptrs.add(j).read() };
                    if !copy_zone(z, logical, &mut cur_offset, &mut written)? {
                        return Ok(written);
                    }
                    logical += BLOCK_SIZE as usize;
                }
            }
        }

        // Triple indirect: zones[9] -> [u32] -> [u32] -> [u32] -> data.
        if cur_offset < end && inode.zones[NUM_DIRECT_ZONES + 2] != 0 {
            self.read_indirect_zone(inode.zones[NUM_DIRECT_ZONES + 2], &tind_buf)?;
            let tptrs = tind_buf.as_ptr() as *const u32;
            'tind: for i in 0..INDIRECT_PTRS_PER_BLOCK {
                let tz = unsafe { tptrs.add(i).read() };
                if tz == 0 {
                    logical +=
                        INDIRECT_PTRS_PER_BLOCK * INDIRECT_PTRS_PER_BLOCK * BLOCK_SIZE as usize;
                    if cur_offset >= end {
                        break 'tind;
                    }
                    continue;
                }
                self.read_indirect_zone(tz, &dind_buf)?;
                let dptrs = dind_buf.as_ptr() as *const u32;
                for j in 0..INDIRECT_PTRS_PER_BLOCK {
                    let dz = unsafe { dptrs.add(j).read() };
                    if dz == 0 {
                        logical += INDIRECT_PTRS_PER_BLOCK * BLOCK_SIZE as usize;
                        continue;
                    }
                    self.read_indirect_zone(dz, &ind_buf)?;
                    let ptrs = ind_buf.as_ptr() as *const u32;
                    for k in 0..INDIRECT_PTRS_PER_BLOCK {
                        if cur_offset >= end {
                            return Ok(written);
                        }
                        let z = unsafe { ptrs.add(k).read() };
                        if !copy_zone(z, logical, &mut cur_offset, &mut written)? {
                            return Ok(written);
                        }
                        logical += BLOCK_SIZE as usize;
                    }
                }
            }
        }

        Ok(written)
    }

    fn read_indirect_zone(&self, zone: u32, into: &blocks::Buffer) -> Result<(), FsError> {
        if !virtio::device::block_read(
            self.dev,
            into.as_mut_ptr(),
            BLOCK_SIZE,
            zone as u64 * BLOCK_SIZE as u64,
        ) {
            return Err(FsError::BlockReadFailed);
        }
        Ok(())
    }
}
