//! Error type returned by the Minix 3 reader.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FsError {
    BadMagic,
    BlockReadFailed,
    OutOfMemory,
    InodeOutOfRange,
    BufferTooSmall,
}
