//! Bare-minimum ELF64 parser.
//!
//! Just enough to validate the magic, machine, and type fields, then
//! walk the program-header table picking out the `PT_LOAD` segments
//! that actually need to be mapped into a process's address space.

/// `\x7fELF` — first four bytes of every ELF file.
pub const MAGIC: u32 = 0x464c_457f;
/// e_machine for RISC-V.
pub const MACHINE_RISCV: u16 = 0xf3;
/// e_type for an executable file.
pub const TYPE_EXEC: u16 = 2;

/// p_type == LOAD: this header's bytes are mapped into memory.
pub const PT_LOAD: u32 = 1;

pub const PROG_EXECUTE: u32 = 1;
pub const PROG_WRITE: u32 = 2;
pub const PROG_READ: u32 = 4;

/// ELF64 file header. Field order/sizes match the ELF spec exactly so
/// we can `*(buf as *const Header)` after a `block_read`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Header {
    pub magic: u32,
    pub bitsize: u8,
    pub endian: u8,
    pub ident_abi_version: u8,
    pub target_platform: u8,
    pub abi_version: u8,
    pub padding: [u8; 7],
    pub obj_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry_addr: usize,
    pub phoff: usize,
    pub shoff: usize,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

/// ELF64 program header — describes one segment of the file.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ProgramHeader {
    pub seg_type: u32,
    pub flags: u32,
    pub off: usize,
    pub vaddr: usize,
    pub paddr: usize,
    pub filesz: usize,
    pub memsz: usize,
    pub align: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ElfError {
    BadMagic,
    NotRiscv,
    NotExecutable,
    Truncated,
}

/// Validate `buf` and return a borrowed reference to its header.
///
/// `buf` must be the full file image — `phoff + phnum * phentsize`
/// has to be in bounds for the program-header walk to succeed.
pub fn parse(buf: &[u8]) -> Result<&Header, ElfError> {
    if buf.len() < core::mem::size_of::<Header>() {
        return Err(ElfError::Truncated);
    }
    let hdr = unsafe { &*(buf.as_ptr() as *const Header) };
    if hdr.magic != MAGIC {
        return Err(ElfError::BadMagic);
    }
    if hdr.machine != MACHINE_RISCV {
        return Err(ElfError::NotRiscv);
    }
    if hdr.obj_type != TYPE_EXEC {
        return Err(ElfError::NotExecutable);
    }
    let ph_end = hdr
        .phoff
        .checked_add(hdr.phnum as usize * hdr.phentsize as usize)
        .ok_or(ElfError::Truncated)?;
    if ph_end > buf.len() {
        return Err(ElfError::Truncated);
    }
    Ok(hdr)
}

/// Iterate the program-header table without copying.
pub fn program_headers<'a>(
    buf: &'a [u8],
    hdr: &Header,
) -> impl Iterator<Item = &'a ProgramHeader> + 'a {
    let phoff = hdr.phoff;
    let phnum = hdr.phnum as usize;
    (0..phnum).map(move |i| {
        // Safe because `parse` already bounds-checked phoff..ph_end.
        unsafe {
            &*(buf.as_ptr().add(phoff + i * core::mem::size_of::<ProgramHeader>())
                as *const ProgramHeader)
        }
    })
}
