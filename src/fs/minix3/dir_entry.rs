//! Directory entry — what lives inside the data blocks of a directory
//! inode. 4-byte inode number plus a 60-byte NUL-padded name.

use super::consts::MAX_NAME_LEN;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DirEntry {
    pub inode: u32,
    pub name: [u8; MAX_NAME_LEN],
}

impl DirEntry {
    /// Returns the entry's name as a `&str`, stripping the NUL padding.
    pub fn name_str(&self) -> &str {
        let len = self
            .name
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..len]).unwrap_or("<bad utf8>")
    }
}
